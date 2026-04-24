//! Простий QMP-клієнт для QEMU monitor protocol (JSON over TCP).

use anyhow::{anyhow, Context, Result};
use crossbeam_channel::{Sender, unbounded};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

pub struct QmpClient {
    write_half: Arc<Mutex<TcpStream>>,
    read_half: Option<TcpStream>,
}

impl QmpClient {
    pub fn connect_with_retry(addr: &str, timeout: Duration) -> Result<Self> {
        let start = Instant::now();
        loop {
            match TcpStream::connect(addr) {
                Ok(s) => {
                    let s2 = s.try_clone().context("clone tcpstream")?;
                    let mut client = Self {
                        write_half: Arc::new(Mutex::new(s)),
                        read_half: Some(s2),
                    };
                    client.handshake()?;
                    return Ok(client);
                }
                Err(_) if start.elapsed() < timeout => {
                    thread::sleep(Duration::from_millis(200));
                }
                Err(e) => return Err(anyhow!("QMP connect failed: {e}")),
            }
        }
    }

    fn handshake(&mut self) -> Result<()> {
        // Перше — читаємо greeting через ВІДКЛАДЕНИЙ read_half (бо нам потрібен оригінал).
        // Спрощено: відкриємо тимчасовий BufReader на копії.
        let stream = self.read_half.as_mut().expect("read_half").try_clone()?;
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).context("read QMP greeting")?;
        crate::log_info(&format!("QMP greeting: {}", line.trim()));

        // Заповнюємо capabilities
        self.send_raw(r#"{"execute":"qmp_capabilities"}"#)?;
        let mut line2 = String::new();
        reader.read_line(&mut line2).context("read QMP cap reply")?;
        if !line2.contains("\"return\"") {
            return Err(anyhow!("QMP capabilities failed: {line2}"));
        }
        Ok(())
    }

    fn send_raw(&self, msg: &str) -> Result<()> {
        let mut s = self.write_half.lock().unwrap();
        writeln!(s, "{msg}").context("QMP write")?;
        s.flush().ok();
        Ok(())
    }

    /// Виконати `human-monitor-command`. Повертає текстову відповідь (без асинхронних подій).
    pub fn hmp(&self, cmd: &str) -> Result<()> {
        let json = serde_json::json!({
            "execute": "human-monitor-command",
            "arguments": { "command-line": cmd }
        });
        self.send_raw(&json.to_string())
    }

    pub fn hostfwd_add(&self, host_port: u16, guest_port: u16) -> Result<()> {
        self.hmp(&format!("hostfwd_add tcp:127.0.0.1:{host_port}-:{guest_port}"))
    }

    pub fn hostfwd_remove(&self, host_port: u16, guest_port: u16) -> Result<()> {
        self.hmp(&format!("hostfwd_remove tcp:127.0.0.1:{host_port}-:{guest_port}"))
    }

    pub fn command_sender(&self) -> CommandHandle {
        CommandHandle { write_half: self.write_half.clone() }
    }

    /// Спіннемо потік для читання QMP подій (асинхронні events).
    pub fn spawn_reader(&mut self) {
        if let Some(s) = self.read_half.take() {
            thread::spawn(move || {
                let reader = BufReader::new(s);
                for line in reader.lines() {
                    match line {
                        Ok(l) => {
                            let trimmed = l.trim();
                            if trimmed.contains("\"event\"") {
                                crate::log_info(&format!("QMP event: {trimmed}"));
                            }
                        }
                        Err(_) => break,
                    }
                }
                crate::log_warn("QMP reader exited");
            });
        }
    }
}

/// Клонабельний handle для виконання QMP команд з інших потоків.
#[derive(Clone)]
pub struct CommandHandle {
    write_half: Arc<Mutex<TcpStream>>,
}

impl CommandHandle {
    pub fn hostfwd_add(&self, host_port: u16, guest_port: u16) -> Result<()> {
        let cmd = format!("hostfwd_add tcp:127.0.0.1:{host_port}-:{guest_port}");
        let json = serde_json::json!({
            "execute": "human-monitor-command",
            "arguments": { "command-line": cmd }
        }).to_string();
        let mut s = self.write_half.lock().unwrap();
        writeln!(s, "{json}").context("QMP write")?;
        s.flush().ok();
        Ok(())
    }

    pub fn hostfwd_remove(&self, host_port: u16, guest_port: u16) -> Result<()> {
        let cmd = format!("hostfwd_remove tcp:127.0.0.1:{host_port}-:{guest_port}");
        let json = serde_json::json!({
            "execute": "human-monitor-command",
            "arguments": { "command-line": cmd }
        }).to_string();
        let mut s = self.write_half.lock().unwrap();
        writeln!(s, "{json}").context("QMP write")?;
        s.flush().ok();
        Ok(())
    }
}
