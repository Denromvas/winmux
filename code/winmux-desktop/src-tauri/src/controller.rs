//! Spawns and supervises winmux-controller.exe (bundled with the app).

use anyhow::Result;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

use crate::VmStatus;

pub struct ControllerHandle {
    child: Child,
    pub status: Arc<Mutex<VmStatus>>,
}

impl ControllerHandle {
    pub fn kill(&mut self) -> Result<()> {
        self.child.kill()?;
        Ok(())
    }
}

/// Парсить один рядок логу контролера і оновлює VmStatus.
/// Повертає true якщо статус змінився (треба emit подію).
pub fn parse_log_line(line: &str, status: &mut VmStatus) -> bool {
    if line.contains("QEMU launched: PID ") {
        if let Some(pid_str) = line.split("PID ").nth(1) {
            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                status.pid = Some(pid);
                status.state = "starting".into();
                return true;
            }
        }
    }
    if line.contains("guest ready:") {
        status.state = "running".into();
        // витягнемо kernel
        if let Some(idx) = line.find("kernel=") {
            let k = &line[idx + 7..];
            status.guest_kernel = Some(k.trim().to_string());
        }
        return true;
    }
    if line.contains("port_added:") {
        // приклад: "port_added: 8080/Tcp bind=0.0.0.0 comm=..."
        if let Some(rest) = line.split("port_added:").nth(1) {
            if let Some(port_str) = rest.trim().split('/').next() {
                if let Ok(p) = port_str.trim().parse::<u16>() {
                    if p != 22 && !status.ports.contains(&p) {
                        status.ports.push(p);
                        return true;
                    }
                }
            }
        }
    }
    if line.contains("port_removed:") {
        if let Some(rest) = line.split("port_removed:").nth(1) {
            if let Some(port_str) = rest.trim().split_whitespace().next() {
                if let Ok(p) = port_str.parse::<u16>() {
                    let before = status.ports.len();
                    status.ports.retain(|&x| x != p);
                    if status.ports.len() != before {
                        return true;
                    }
                }
            }
        }
    }
    if let Some(idx) = line.find("guest heartbeat: uptime ") {
        let rest = &line[idx + "guest heartbeat: uptime ".len()..];
        if let Some(num) = rest.trim().strip_suffix("s").or(Some(rest.trim())) {
            if let Ok(u) = num.parse::<u64>() {
                status.uptime_sec = Some(u);
                return true;
            }
        }
    }
    false
}

/// Locate the bundled winmux.exe (alongside the desktop binary).
fn locate_controller() -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let dir = exe.parent().ok_or_else(|| anyhow::anyhow!("no parent for current exe"))?;
    let candidate = dir.join("winmux.exe");
    if candidate.exists() {
        return Ok(candidate);
    }
    // dev fallback
    let dev = dir
        .join("..").join("..").join("..").join("..")
        .join("target").join("x86_64-pc-windows-gnu").join("release")
        .join("winmux-controller.exe");
    if dev.exists() {
        return Ok(dev);
    }
    anyhow::bail!("winmux.exe not found near {}", dir.display());
}

pub fn spawn<F, S>(_app: tauri::AppHandle, log_cb: F, status_cb: S) -> Result<ControllerHandle>
where
    F: Fn(String) + Send + Sync + 'static,
    S: Fn(VmStatus) + Send + Sync + 'static,
{
    let exe = locate_controller()?;
    let workdir = exe.parent().unwrap().to_path_buf();

    log_cb(format!("[desktop] launching {}", exe.display()));

    let mut cmd = Command::new(&exe);
    cmd.current_dir(&workdir)
       .arg("start").arg("--config").arg("winmux.toml")
       .stdout(Stdio::piped())
       .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn()?;
    let cb = Arc::new(log_cb);
    let scb = Arc::new(status_cb);
    let status = Arc::new(Mutex::new(VmStatus {
        state: "starting".into(),
        ..Default::default()
    }));
    scb(status.lock().unwrap().clone());

    if let Some(stdout) = child.stdout.take() {
        let cb1 = cb.clone();
        let scb1 = scb.clone();
        let status1 = status.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                {
                    let mut st = status1.lock().unwrap();
                    if parse_log_line(&line, &mut st) {
                        scb1(st.clone());
                    }
                }
                cb1(line);
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let cb2 = cb.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                cb2(format!("[stderr] {line}"));
            }
        });
    }
    Ok(ControllerHandle { child, status })
}
