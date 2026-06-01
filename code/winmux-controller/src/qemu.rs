use crate::config::Config;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

mod ssh_setup {
    use anyhow::{Context, Result};
    use std::path::PathBuf;
    use std::process::Command;

    /// Generate SSH ed25519 keypair in workdir/ssh/, return (private_key_path, public_key_str).
    pub fn ensure_keypair(workdir: &std::path::Path) -> Result<(PathBuf, String)> {
        let ssh_dir = workdir.join("ssh");
        std::fs::create_dir_all(&ssh_dir).context("mkdir ssh")?;
        let priv_key = ssh_dir.join("id_winmux_ed25519");
        let pub_key = ssh_dir.join("id_winmux_ed25519.pub");
        if !priv_key.exists() {
            // Try ssh-keygen.exe (built-in Win OpenSSH client)
            let status = Command::new("ssh-keygen")
                .args(["-t", "ed25519", "-N", "", "-q", "-f"])
                .arg(&priv_key)
                .status()
                .context("ssh-keygen — make sure Windows OpenSSH client is installed")?;
            if !status.success() { anyhow::bail!("ssh-keygen failed"); }
        }
        let pub_str = std::fs::read_to_string(&pub_key)?;
        Ok((priv_key, pub_str.trim().to_string()))
    }

    /// Detect if current user is in BUILTIN\Administrators group.
    /// Windows OpenSSH for admins ignores ~/.ssh/authorized_keys and reads only
    /// %PROGRAMDATA%\ssh\administrators_authorized_keys (default Match Group rule).
    fn is_admin() -> bool {
        // `net session` works only for admins. Cheap & reliable.
        Command::new("net")
            .args(["session"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Add pubkey to administrators_authorized_keys via takeown+icacls round-trip.
    /// File is owned by SYSTEM with no Admin write — even admins need to claim it.
    fn install_admin_pubkey(pub_key: &str) -> Result<()> {
        let f = r"C:\ProgramData\ssh\administrators_authorized_keys";
        // 1. Take ownership and grant ourselves Full
        let _ = Command::new("takeown").args(["/F", f]).output();
        let me = std::env::var("USERNAME").unwrap_or_default();
        let _ = Command::new("icacls").args([f, "/grant", &format!("{me}:F")]).output();

        // 2. Read current, append if missing
        let cur = std::fs::read_to_string(f).unwrap_or_default();
        let key_body = pub_key.split_whitespace().nth(1).unwrap_or("");
        if !key_body.is_empty() && !cur.contains(key_body) {
            let mut new = cur.trim_end().to_string();
            new.push_str("\r\n");
            new.push_str(pub_key.trim());
            new.push_str("\r\n");
            std::fs::write(f, &new).context("write administrators_authorized_keys")?;
            crate::log_info(&format!("added pubkey to {f}"));
        }

        // 3. Restore canonical perms (SYSTEM:F + BUILTIN\Administrators:F via SID — locale-safe)
        let _ = Command::new("icacls").args([
            f, "/inheritance:r",
            "/grant", "*S-1-5-18:F",       // NT AUTHORITY\SYSTEM
            "/grant", "*S-1-5-32-544:F",   // BUILTIN\Administrators
        ]).output();
        Ok(())
    }

    /// Add public key to user's SSH server authorized_keys.
    /// For non-admin: %USERPROFILE%\.ssh\authorized_keys
    /// For admin: %PROGRAMDATA%\ssh\administrators_authorized_keys (special Windows OpenSSH rule)
    pub fn install_pubkey(pub_key: &str) -> Result<String> {
        if is_admin() {
            install_admin_pubkey(pub_key)?;
        } else {
            let home = std::env::var("USERPROFILE").context("USERPROFILE not set")?;
            let ssh_dir = std::path::Path::new(&home).join(".ssh");
            std::fs::create_dir_all(&ssh_dir).ok();
            let auth_path = ssh_dir.join("authorized_keys");

            let mut current = std::fs::read_to_string(&auth_path).unwrap_or_default();
            if !current.contains(pub_key) {
                if !current.is_empty() && !current.ends_with('\n') { current.push('\n'); }
                current.push_str(pub_key);
                current.push('\n');
                std::fs::write(&auth_path, &current).context("write authorized_keys")?;
                crate::log_info(&format!("added pubkey to {}", auth_path.display()));
            }
        }
        Ok(std::env::var("USERNAME").unwrap_or_default())
    }
}

/// Detect Hyper-V root partition (full Hyper-V VMMS service running).
/// We DO NOT use Win32_ComputerSystem.HypervisorPresent — that flag is also
/// True when just HypervisorPlatform (WHPX, the user-mode API we want to use)
/// is enabled, which would cause us to skip WHPX in the very case where it
/// would have worked. Only vmms RUNNING means the OS owns the hypervisor as
/// a root partition and user-mode WHPX VMs would die.
fn has_full_hyperv() -> bool {
    let svc = Command::new("sc")
        .args(["query", "vmms"])  // Hyper-V VMM service
        .output();
    if let Ok(out) = svc {
        let s = String::from_utf8_lossy(&out.stdout);
        // "RUNNING" is the value we care about. "STOPPED" or service-not-found = OK.
        return s.contains("RUNNING");
    }
    false
}

pub struct Vm {
    child: Child,
}

impl Vm {
    pub fn launch(cfg: &Config) -> Result<Self> {
        let qemu = if cfg.qemu_binary.is_absolute() {
            cfg.qemu_binary.clone()
        } else {
            cfg.workdir.join(&cfg.qemu_binary)
        };

        // Auto-create overlay disk from base if it doesn't exist.
        // Convention: якщо disk закінчується на user.qcow2 — base поряд як base.qcow2 в тому ж rootfs/
        let disk_path: PathBuf = if cfg.disk.is_absolute() {
            cfg.disk.clone()
        } else {
            cfg.workdir.join(&cfg.disk)
        };
        if !disk_path.exists() {
            let parent = disk_path.parent()
                .ok_or_else(|| anyhow::anyhow!("disk has no parent dir"))?;
            let base = parent.join("base.qcow2");
            if base.exists() {
                // qemu-img treats the -b path as relative to the new file's dir.
                // Use absolute paths to avoid that confusion.
                let abs_disk = std::fs::canonicalize(&parent)
                    .map(|p| p.join(disk_path.file_name().unwrap()))
                    .unwrap_or_else(|_| disk_path.clone());
                let abs_base = std::fs::canonicalize(&base)
                    .unwrap_or_else(|_| base.clone());
                crate::log_info(&format!("creating overlay {} from {}",
                    abs_disk.display(), abs_base.display()));
                let qemu_img = qemu.parent()
                    .map(|p| p.join("qemu-img.exe"))
                    .unwrap_or_else(|| PathBuf::from("qemu-img.exe"));
                let status = Command::new(&qemu_img)
                    .args([
                        "create", "-f", "qcow2",
                        "-b", abs_base.to_str().unwrap(),
                        "-F", "qcow2",
                        abs_disk.to_str().unwrap(),
                        "20G",
                    ])
                    .status()
                    .with_context(|| format!("running {}", qemu_img.display()))?;
                if !status.success() {
                    anyhow::bail!("qemu-img create failed");
                }
            } else {
                anyhow::bail!("disk {} not found and no base.qcow2 alongside",
                    disk_path.display());
            }
        }

        // Auto-detect: при "auto" пробуємо WHPX лише якщо востаннє він не крашив у нас.
        // Стан зберігаємо у workdir/last-accel.txt: "whpx-ok" або "whpx-failed".
        let last_accel_path = cfg.workdir.join("last-accel.txt");
        let accel = match cfg.accel.as_str() {
            "auto" => {
                let last = std::fs::read_to_string(&last_accel_path)
                    .map(|s| s.trim().to_string()).unwrap_or_default();
                if last == "whpx-failed" {
                    crate::log_info("auto-accel: WHPX previously failed → using TCG");
                    "tcg".to_string()
                } else if has_full_hyperv() {
                    // Якщо повний Hyper-V (Microsoft-Hyper-V-All) увімкнений —
                    // root partition забирає гипервізор, а user-mode WHPX VM
                    // стартує і помирає за ~7 сек з code=1. Не пробуємо.
                    crate::log_info("auto-accel: Hyper-V Platform full mode detected → using TCG (WHPX would crash)");
                    "tcg".to_string()
                } else {
                    crate::log_info("auto-accel: trying WHPX (will fallback to TCG if it dies fast)");
                    "whpx,kernel-irqchip=off".to_string()
                }
            }
            other => other.to_string(),
        };
        // MTTCG: коли йдемо в TCG, дозволяємо vCPU крутитися в кількох потоках
        // (інакше SMP TCG однопотоковий → відчутна затримка) + більший TB-кеш,
        // менше ретрансляцій. Для гостя прозоро. WHPX цих опцій не приймає.
        let tune_tcg = |a: &str| -> String {
            if a.starts_with("tcg") && !a.contains("thread=") {
                format!("{a},thread=multi,tb-size=256")
            } else {
                a.to_string()
            }
        };
        let accel = tune_tcg(&accel);
        // Tag для запису після успіху/невдачі
        let _ = std::fs::write(&last_accel_path, "whpx-trying");

        let mut cmd = Command::new(&qemu);
        cmd.current_dir(&cfg.workdir)
            .arg("-accel").arg(&accel)
            .arg("-accel").arg("tcg,thread=multi,tb-size=256")  // fallback (MTTCG)
            .arg("-m").arg(&cfg.ram)
            .arg("-smp").arg(cfg.smp.to_string());

        // CPU model: для TCG треба max (інакше SSE2-only, що ламає Node V8 native binaries
        // як claude-code, sharp тощо з SIGILL "Illegal instruction").
        // Для WHPX -cpu max викликає VP exit code 4 — там краще без явного -cpu.
        if accel.starts_with("tcg") {
            cmd.arg("-cpu").arg("max");
        }

        // Direct kernel boot
        if let Some(kernel) = &cfg.kernel {
            cmd.arg("-kernel").arg(kernel)
               .arg("-append").arg(&cfg.kernel_append);
        }

        cmd.arg("-drive").arg(format!("file={},if=virtio,format=qcow2", cfg.disk.display()));

        // Network with auto-forward of SSH and agent ports
        // SSH: hostfwd для adminського SSH доступу
        // Agent: gust підключається до 10.0.2.2:agent_port (через SLIRP прокситься на host's agent_port)
        let netdev = format!(
            "user,id=n0,hostfwd=tcp:127.0.0.1:{}-:22",
            cfg.ssh_port
        );
        cmd.arg("-netdev").arg(netdev)
           .arg("-device").arg("virtio-net-pci,netdev=n0");

        // QMP
        cmd.arg("-qmp").arg(format!("tcp:127.0.0.1:{},server=on,wait=off", cfg.qmp_port));

        // Low-latency terminal channel: a virtio-serial port backed by a host
        // chardev socket. The desktop connects here and the guest agent runs a
        // PTY shell per channel — no SSH crypto, no SLIRP TCP/IP stack.
        // nodelay=on disables Nagle on the host loopback socket.
        cmd.arg("-device").arg("virtio-serial-pci,id=wmvioser0");
        cmd.arg("-chardev").arg(format!(
            "socket,id=wmterm,host=127.0.0.1,port={},server=on,wait=off,nodelay=on",
            cfg.term_port
        ));
        cmd.arg("-device").arg("virtserialport,chardev=wmterm,name=winmux.term");

        // --- Auto-mount Windows folders via SSH key ---
        // Generate (or reuse) SSH key, install public key into Windows OpenSSH authorized_keys,
        // then pass private key + Windows username + USERPROFILE path to guest via fw_cfg.
        // Guest reads from /sys/firmware/qemu_fw_cfg/by_name/opt/winmux/*/raw and mounts via sshfs.
        let mut auto_mount_ok = false;
        if let Ok((priv_path, pub_key)) = ssh_setup::ensure_keypair(&cfg.workdir) {
            if let Ok(username) = ssh_setup::install_pubkey(&pub_key) {
                if let Ok(priv_data) = std::fs::read_to_string(&priv_path) {
                    let userprofile = std::env::var("USERPROFILE").unwrap_or_default()
                        .replace('\\', "/");
                    cmd.arg("-fw_cfg")
                       .arg(format!("name=opt/winmux/user,string={}", username));
                    cmd.arg("-fw_cfg")
                       .arg(format!("name=opt/winmux/profile,string=/{}", userprofile));
                    // Private key (для sshfs до Windows-host)
                    let key_tmp = cfg.workdir.join("ssh").join("key.tmp");
                    if std::fs::write(&key_tmp, &priv_data).is_ok() {
                        cmd.arg("-fw_cfg")
                           .arg(format!("name=opt/winmux/keyfile,file={}",
                                key_tmp.to_string_lossy()));
                        // Public key — guest init додасть у /home/winmux/.ssh/authorized_keys
                        // для passwordless SSH від хоста до guest
                        let pub_tmp = cfg.workdir.join("ssh").join("pub.tmp");
                        let _ = std::fs::write(&pub_tmp, &pub_key);
                        cmd.arg("-fw_cfg")
                           .arg(format!("name=opt/winmux/pubkey,file={}",
                                pub_tmp.to_string_lossy()));
                        auto_mount_ok = true;
                        crate::log_info(&format!(
                            "auto-mount: user={} profile={} (passwordless guest SSH enabled)",
                            username, userprofile
                        ));
                    }
                }
            }
        }
        if !auto_mount_ok {
            crate::log_warn("auto-mount: failed to setup SSH key — guest will require manual winmux-mount");
        }

        // No display, serial → file
        cmd.arg("-display").arg("none")
           .arg("-serial").arg(format!("file:{}", cfg.serial_log.display()));

        if cfg.hidden {
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }
        }

        cmd.stdout(Stdio::null())
           .stderr(Stdio::piped());

        let child = cmd.spawn()
            .with_context(|| format!("spawn {}", qemu.display()))?;

        // Якщо ми пробували WHPX — слідкуємо протягом 5с; якщо помер — позначаємо як failed.
        if accel.starts_with("whpx") {
            let pid = child.id();
            let workdir = cfg.workdir.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(5));
                // Перевіряємо чи процес ще живий через WaitForSingleObject (Windows).
                // На Linux/dev: просто пробуємо kill з sig 0.
                #[cfg(windows)]
                let alive = {
                    use std::os::windows::io::AsRawHandle;
                    // Ми не маємо handle — спробуємо tasklist через стандартний API
                    // Простіше: через std::process::Command.
                    let out = std::process::Command::new("tasklist.exe")
                        .args(["/FI", &format!("PID eq {pid}")])
                        .output();
                    match out {
                        Ok(o) => String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()),
                        Err(_) => true,
                    }
                };
                #[cfg(not(windows))]
                let alive = unsafe { libc::kill(pid as i32, 0) == 0 };
                let mark = if alive { "whpx-ok" } else { "whpx-failed" };
                let _ = std::fs::write(workdir.join("last-accel.txt"), mark);
                if !alive {
                    crate::log_warn("auto-accel: WHPX died within 5s — next start will use TCG");
                } else {
                    crate::log_info("auto-accel: WHPX stable after 5s ✓");
                }
            });
        }

        Ok(Self { child })
    }

    pub fn pid(&self) -> u32 { self.child.id() }

    pub fn is_alive(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(Some(_)) => false,
            Ok(None) => true,
            Err(_) => false,
        }
    }

    pub fn kill(&mut self) -> Result<()> {
        self.child.kill().context("kill QEMU")
    }

    /// Спробувати graceful shutdown через QMP `system_powerdown` — поки що просто kill.
    /// Розширимо коли підключимо QMP power command.
    pub fn shutdown_graceful(&mut self) -> Result<()> {
        // TODO: send {"execute":"system_powerdown"} via QMP.
        self.kill()
    }
}
