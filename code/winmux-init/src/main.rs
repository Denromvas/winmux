//! winmux-init -- minimal PID 1 init for WinMux guest.
//!
//! Replaces systemd. Does only what we actually need:
//!   1. Mount /proc, /sys, /dev, /dev/pts, /tmp, /run
//!   2. Bring up network (eth0 + DHCP)
//!   3. Start sshd
//!   4. Spawn login on /dev/ttyS0 (serial console)
//!
//! Target: <2 seconds to shell prompt on TCG.

use nix::mount::{mount, MsFlags};
use nix::sys::signal::{signal, SigHandler, Signal};
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::{execv, fork, ForkResult, Pid};
use std::ffi::CString;
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Instant;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn log(msg: &str) {
    eprintln!("[winmux-init] {msg}");
    let _ = fs::write("/dev/kmsg", format!("winmux-init: {msg}\n"));
}

fn mount_fs(src: &str, target: &str, fstype: &str, flags: MsFlags) {
    if !Path::new(target).exists() {
        let _ = fs::create_dir_all(target);
    }
    match mount(Some(src), target, Some(fstype), flags, None::<&str>) {
        Ok(_) => log(&format!("mounted {fstype} at {target}")),
        Err(e) => log(&format!("FAIL mount {fstype} at {target}: {e}")),
    }
}

fn early_mounts() {
    mount_fs("proc", "/proc", "proc", MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC);
    mount_fs("sys", "/sys", "sysfs", MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC);
    mount_fs("dev", "/dev", "devtmpfs", MsFlags::MS_NOSUID);
    let _ = fs::create_dir_all("/dev/pts");
    mount_fs("devpts", "/dev/pts", "devpts", MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC);
    let _ = fs::create_dir_all("/dev/shm");
    mount_fs("tmpfs", "/dev/shm", "tmpfs", MsFlags::MS_NOSUID | MsFlags::MS_NODEV);
    mount_fs("tmpfs", "/run", "tmpfs", MsFlags::MS_NOSUID | MsFlags::MS_NODEV);
    mount_fs("tmpfs", "/tmp", "tmpfs", MsFlags::MS_NOSUID | MsFlags::MS_NODEV);
}

fn bring_up_network() {
    let _ = Command::new("/sbin/ip").args(["link", "set", "lo", "up"]).status();
    let _ = Command::new("/sbin/ip").args(["addr", "add", "127.0.0.1/8", "dev", "lo"]).status();

    let mut iface_up = None;
    for iface in ["eth0", "ens3"] {
        let r = Command::new("/sbin/ip").args(["link", "set", iface, "up"]).status();
        if let Ok(s) = r {
            if s.success() {
                log(&format!("brought up {iface}"));
                iface_up = Some(iface);
                break;
            }
        }
    }

    // Set hostname from /etc/hostname or sane default
    let hostname = fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "(none)")
        .unwrap_or_else(|| "winmux-guest".into());
    use std::ffi::CString;
    if let Ok(cs) = CString::new(hostname.as_str()) {
        unsafe {
            extern "C" {
                fn sethostname(name: *const i8, len: usize) -> i32;
            }
            sethostname(cs.as_ptr() as *const i8, hostname.len());
        }
    }
    let _ = fs::write("/etc/hostname", &hostname);
    let _ = fs::write("/etc/hosts", format!("127.0.0.1 {hostname} localhost\n::1 {hostname} localhost ip6-localhost ip6-loopback\n"));
    log(&format!("hostname set to {hostname}"));

    if let Some(iface) = iface_up {
        let mut got_dhcp = false;
        if Path::new("/sbin/dhclient").exists() {
            if Command::new("/sbin/dhclient")
                .args(["-1", iface])
                .stdout(Stdio::null()).stderr(Stdio::null())
                .status().map(|s| s.success()).unwrap_or(false) {
                log("dhclient done");
                got_dhcp = true;
            }
        } else if Path::new("/usr/bin/udhcpc").exists() || Path::new("/sbin/udhcpc").exists() {
            if Command::new("udhcpc")
                .args(["-i", iface, "-q", "-n"])
                .stdout(Stdio::null()).stderr(Stdio::null())
                .status().map(|s| s.success()).unwrap_or(false) {
                log("udhcpc done");
                got_dhcp = true;
            }
        }
        if !got_dhcp {
            // Fallback: static IP. QEMU SLIRP за замовчуванням видає 10.0.2.15/24, шлюз 10.0.2.2
            let _ = Command::new("/sbin/ip").args(["addr", "add", "10.0.2.15/24", "dev", iface]).status();
            let _ = Command::new("/sbin/ip").args(["route", "add", "default", "via", "10.0.2.2"]).status();
            log("static IP 10.0.2.15/24 fallback applied");
        }
    }
    // /etc/resolv.conf — ВИДАЛЯЄМО симлінк (Ubuntu 24 default → systemd-resolved stub),
    // потім записуємо звичайний файл.
    let _ = fs::remove_file("/etc/resolv.conf");
    let _ = fs::write("/etc/resolv.conf", "nameserver 1.1.1.1\nnameserver 8.8.8.8\n");
}

fn ensure_basic_dirs() {
    for dir in ["/var/run", "/var/log", "/var/lib", "/var/empty", "/run/sshd"] {
        let _ = fs::create_dir_all(dir);
    }
}

/// Read a string value from /sys/firmware/qemu_fw_cfg/by_name/<name>/raw if available.
fn fw_cfg_read(name: &str) -> Option<String> {
    let path = format!("/sys/firmware/qemu_fw_cfg/by_name/{name}/raw");
    fs::read_to_string(&path).ok().map(|s| s.trim().to_string())
}

fn fw_cfg_read_bytes(name: &str) -> Option<Vec<u8>> {
    let path = format!("/sys/firmware/qemu_fw_cfg/by_name/{name}/raw");
    fs::read(&path).ok()
}

fn auto_mount_workspace() {
    // Чекаємо що qemu_fw_cfg модуль завантажиться (kernel autoloads коли є devtree)
    let _ = Command::new("/sbin/modprobe").arg("qemu_fw_cfg").status();
    std::thread::sleep(std::time::Duration::from_millis(200));

    let user = match fw_cfg_read("opt/winmux/user") {
        Some(u) if !u.is_empty() => u,
        _ => { log("auto-mount: no opt/winmux/user — skipping"); return; }
    };
    let profile = fw_cfg_read("opt/winmux/profile").unwrap_or_default();
    let key_data = match fw_cfg_read_bytes("opt/winmux/keyfile") {
        Some(d) if !d.is_empty() => d,
        _ => { log("auto-mount: no opt/winmux/keyfile — skipping"); return; }
    };

    let ssh_dir = "/home/winmux/.ssh";
    let _ = fs::create_dir_all(ssh_dir);

    // Private key для sshfs до Windows-host
    let key_path = format!("{ssh_dir}/winmux_auto");
    if fs::write(&key_path, &key_data).is_err() {
        log("auto-mount: failed to write key");
        return;
    }
    let _ = Command::new("/bin/chmod").args(["600", &key_path]).status();

    // Public key → /home/winmux/.ssh/authorized_keys для passwordless SSH від хоста до guest.
    // Це дає Tauri-терміналу заходити без пароля.
    if let Some(pub_data) = fw_cfg_read_bytes("opt/winmux/pubkey") {
        if !pub_data.is_empty() {
            let auth_path = format!("{ssh_dir}/authorized_keys");
            let mut existing = fs::read_to_string(&auth_path).unwrap_or_default();
            let pub_str = String::from_utf8_lossy(&pub_data);
            if !existing.contains(pub_str.trim()) {
                if !existing.is_empty() && !existing.ends_with('\n') { existing.push('\n'); }
                existing.push_str(pub_str.trim());
                existing.push('\n');
                let _ = fs::write(&auth_path, existing);
                let _ = Command::new("/bin/chmod").args(["600", &auth_path]).status();
                log("auto-mount: passwordless SSH enabled (host → guest)");
            }
        }
    }

    let _ = Command::new("/bin/chmod").args(["700", ssh_dir]).status();
    let _ = Command::new("/bin/chown").args(["-R", "winmux:winmux", ssh_dir]).status();

    // Update /etc/motd from guest agent — щоб видно було що workspace готовий
    let _ = std::fs::write("/etc/update-motd.d/99-winmux", r#"#!/bin/sh
cat <<'BANNER'

╔══════════════════════════════════════════════════════════════════════╗
║   WinMux Linux                                                        ║
║                                                                        ║
║   📁 ~/win → ваш Windows USERPROFILE (auto-mount via SSH key)         ║
║   🤖 claude          — AI agent (need ANTHROPIC_API_KEY)              ║
║   ⚡ claude --dangerously-skip-permissions   — auto-mode              ║
║   📦 npm install -g <pkg>   — будь-що з npm                           ║
║                                                                        ║
║   Ports: будь-який LISTEN автоматично доступний на Windows localhost  ║
╚══════════════════════════════════════════════════════════════════════╝

BANNER
"#);
    let _ = Command::new("/bin/chmod").args(["+x", "/etc/update-motd.d/99-winmux"]).status();

    // Mount: target=/workspace, source=user@10.0.2.2:profile_path
    let _ = fs::create_dir_all("/workspace");
    let target = "/workspace";
    let source = format!("{user}@10.0.2.2:{profile}");

    let status = Command::new("/usr/bin/sshfs")
        .args([
            "-o", &format!("IdentityFile={key_path}"),
            "-o", "StrictHostKeyChecking=no",
            "-o", "UserKnownHostsFile=/dev/null",
            "-o", "reconnect,ServerAliveInterval=15,allow_other,uid=1000,gid=1000",
            &source, target,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    match status {
        Ok(s) if s.success() => log(&format!("auto-mount: ✓ {} → /workspace", source)),
        Ok(s) => log(&format!("auto-mount: sshfs failed (exit {})", s.code().unwrap_or(-1))),
        Err(e) => log(&format!("auto-mount: sshfs spawn err: {e}")),
    }

    // Зробимо symlink ~/win → /workspace для backwards compat
    let _ = fs::remove_dir_all("/home/winmux/win");
    let _ = std::os::unix::fs::symlink("/workspace", "/home/winmux/win");
}

fn ensure_ssh_host_keys() {
    // Якщо немає host keys — згенеруємо. Інакше sshd падає мовчки.
    if !Path::new("/etc/ssh/ssh_host_ed25519_key").exists() {
        log("generating SSH host keys...");
        let _ = Command::new("/usr/bin/ssh-keygen")
            .args(["-A"])
            .stdout(Stdio::null()).stderr(Stdio::null())
            .status();
        log("SSH host keys generated");
    }
}

fn start_sshd() {
    ensure_ssh_host_keys();
    log("starting sshd...");
    let r = Command::new("/usr/sbin/sshd")
        .arg("-D")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    match r {
        Ok(child) => log(&format!("sshd PID={}", child.id())),
        Err(e) => log(&format!("sshd FAIL: {e}")),
    }
}

fn start_winmux_agent() {
    let agent = "/sbin/winmux-agent";
    if !Path::new(agent).exists() {
        log("winmux-agent binary not found, skipping");
        return;
    }
    log("starting winmux-agent...");
    let r = Command::new(agent)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    match r {
        Ok(child) => log(&format!("winmux-agent PID={}", child.id())),
        Err(e) => log(&format!("winmux-agent FAIL: {e}")),
    }
}

fn install_zombie_reaper() {
    extern "C" fn handle_sigchld(_: i32) {
        loop {
            match waitpid(None, Some(WaitPidFlag::WNOHANG)) {
                Ok(WaitStatus::StillAlive) | Err(_) => break,
                _ => continue,
            }
        }
    }
    unsafe {
        let _ = signal(Signal::SIGCHLD, SigHandler::Handler(handle_sigchld));
    }
}

fn spawn_login_on_console() {
    match unsafe { fork() } {
        Ok(ForkResult::Parent { child }) => {
            log(&format!("login spawned PID={child}"));
        }
        Ok(ForkResult::Child) => {
            let agetty = "/sbin/agetty";
            if Path::new(agetty).exists() {
                let prog = CString::new(agetty).unwrap();
                let args = [
                    CString::new("agetty").unwrap(),
                    CString::new("--autologin").unwrap(),
                    CString::new("winmux").unwrap(),
                    CString::new("-8").unwrap(),
                    CString::new("-L").unwrap(),
                    CString::new("ttyS0").unwrap(),
                    CString::new("115200").unwrap(),
                    CString::new("vt100").unwrap(),
                ];
                let _ = execv(&prog, &args);
            }
            let _ = Command::new("/bin/bash").arg("-l").exec();
            std::process::exit(1);
        }
        Err(e) => log(&format!("fork FAIL: {e}")),
    }
}

fn main() {
    let start = Instant::now();
    log(&format!("WinMux init v{VERSION} starting (pid {})", std::process::id()));

    if std::process::id() != 1 {
        log("WARN: not PID 1; running in test mode");
    }

    early_mounts();
    log(&format!("mounts done at +{}ms", start.elapsed().as_millis()));

    ensure_basic_dirs();
    bring_up_network();
    log(&format!("network done at +{}ms", start.elapsed().as_millis()));

    install_zombie_reaper();

    start_sshd();
    log(&format!("sshd done at +{}ms", start.elapsed().as_millis()));

    start_winmux_agent();
    log(&format!("agent done at +{}ms", start.elapsed().as_millis()));

    // Auto-mount Windows folder via SSH key from fw_cfg (background thread бо може блокувати)
    std::thread::spawn(move || {
        auto_mount_workspace();
    });

    spawn_login_on_console();
    log(&format!("READY at +{}ms", start.elapsed().as_millis()));

    loop {
        match waitpid(Pid::from_raw(-1), None) {
            Ok(WaitStatus::Exited(pid, code)) => {
                log(&format!("child {pid} exited code={code}"));
            }
            Ok(WaitStatus::Signaled(pid, sig, _)) => {
                log(&format!("child {pid} killed by signal {sig:?}"));
            }
            Ok(_) => {}
            Err(e) => {
                if format!("{e}").contains("ECHILD") {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                } else {
                    log(&format!("waitpid err: {e}"));
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }
            }
        }
    }
}
