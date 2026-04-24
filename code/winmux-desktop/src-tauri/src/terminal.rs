//! Real PTY terminal: spawn ssh.exe via ConPTY, pipe both ways into xterm.js.

use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;

pub struct TerminalHandle {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pty_master: Box<dyn portable_pty::MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

impl TerminalHandle {
    pub fn write(&mut self, data: &str) -> Result<()> {
        let mut w = self.writer.lock().unwrap();
        w.write_all(data.as_bytes())?;
        w.flush().ok();
        Ok(())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.pty_master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }

    pub fn kill(&mut self) -> Result<()> {
        let _ = self.child.kill();
        Ok(())
    }
}

/// Spawn ssh client connected to guest, pipe output via callback, return write-handle.
/// `on_data` is invoked for each chunk read from PTY (UTF-8 best-effort).
pub fn spawn_ssh<F>(host_port: u16, on_data: F) -> Result<TerminalHandle>
where
    F: Fn(String) + Send + Sync + 'static,
{
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 30,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("openpty")?;

    // Командний рядок ssh — через Windows OpenSSH client (вбудований у Win10 1803+).
    // Спершу пробуємо знайти SSH key (той самий, що для auto-mount) — для passwordless login.
    let key_path = std::env::current_exe().ok()
        .and_then(|exe| exe.parent().map(|p| p.join("ssh").join("id_winmux_ed25519")))
        .filter(|p| p.exists());

    let mut cmd = CommandBuilder::new("ssh.exe");
    cmd.args([
        "-p", &host_port.to_string(),
        "-o", "StrictHostKeyChecking=no",
        "-o", "UserKnownHostsFile=NUL",
        "-o", "BatchMode=no",
        "-tt",
    ]);
    if let Some(kp) = &key_path {
        cmd.args([
            "-i", kp.to_str().unwrap_or(""),
            "-o", "IdentitiesOnly=yes",
            "-o", "PreferredAuthentications=publickey,password",
        ]);
    } else {
        cmd.args(["-o", "PreferredAuthentications=password,keyboard-interactive"]);
    }
    cmd.arg("winmux@127.0.0.1");
    cmd.env("TERM", "xterm-256color");

    let child = pair.slave.spawn_command(cmd).context("spawn ssh")?;
    drop(pair.slave);  // не потрібен далі

    let mut reader = pair.master.try_clone_reader().context("clone pty reader")?;
    let writer = pair.master.take_writer().context("take pty writer")?;
    let writer = Arc::new(Mutex::new(writer));

    // Reader thread → on_data callback
    let cb = Arc::new(on_data);
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let chunk = String::from_utf8_lossy(&buf[..n]).to_string();
                    cb(chunk);
                }
                Err(_) => break,
            }
        }
    });

    Ok(TerminalHandle {
        writer,
        pty_master: pair.master,
        child,
    })
}
