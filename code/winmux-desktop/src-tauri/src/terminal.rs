//! Real PTY terminal: spawn ssh.exe via ConPTY, pipe both ways into xterm.js.

use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;

/// SSH-over-SLIRP backend (fallback transport).
pub struct SshHandle {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pty_master: Box<dyn portable_pty::MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

impl SshHandle {
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

/// A terminal tab is either the fast virtio-serial mux channel or the SSH
/// fallback. lib.rs picks the backend once per VM (see term_mux::probe).
pub enum TerminalHandle {
    Ssh(SshHandle),
    Mux {
        client: std::sync::Arc<crate::term_mux::MuxClient>,
        channel: u32,
    },
}

impl TerminalHandle {
    pub fn write(&mut self, data: &str) -> Result<()> {
        match self {
            TerminalHandle::Ssh(h) => h.write(data),
            TerminalHandle::Mux { client, channel } => {
                client.write(*channel, data);
                Ok(())
            }
        }
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        match self {
            TerminalHandle::Ssh(h) => h.resize(cols, rows),
            TerminalHandle::Mux { client, channel } => {
                client.resize(*channel, cols, rows);
                Ok(())
            }
        }
    }

    pub fn kill(&mut self) -> Result<()> {
        match self {
            TerminalHandle::Ssh(h) => h.kill(),
            TerminalHandle::Mux { client, channel } => {
                client.close_channel(*channel);
                Ok(())
            }
        }
    }
}

/// Spawn ssh client connected to guest, pipe output via callback, return write-handle.
/// `on_data` is invoked for each chunk read from PTY (UTF-8 best-effort).
pub fn spawn_ssh<F>(host_port: u16, on_data: F) -> Result<SshHandle>
where
    F: Fn(String) + Send + Sync + 'static,
{
    let pty_system = native_pty_system();
    // Wide but SHORT initial matrix. Wide cols (200) so early wide output / TUI
    // boxes don't wrap before the first resize_term. But rows MUST stay small (24):
    // if initial rows > the real window height, bash prints the banner+prompt on a
    // tall canvas, then xterm resizes down and shows the BOTTOM slice — leaving the
    // prompt stuck at the bottom with empty space above. 24 keeps the prompt near
    // the top; resize_term from xterm.fit() expands it within a frame.
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 200,
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
        // Low-latency interactive tuning: no compression (saves CPU per
        // keystroke), low-delay QoS, and a fast AES-NI AEAD cipher first
        // (cheaper to encrypt each echo char than the chacha20 default,
        // especially under TCG where every cycle is emulated).
        "-o", "Compression=no",
        "-o", "IPQoS=lowdelay",
        "-c", "aes128-gcm@openssh.com,chacha20-poly1305@openssh.com,aes128-ctr",
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

    Ok(SshHandle {
        writer,
        pty_master: pair.master,
        child,
    })
}
