//! TCP-listener для подій від winmux-agent (всередині гостя).
//! Гість підключається до 10.0.2.2:agent_port (host через SLIRP) і шле JSON-line events.

use crate::port_manager::PortManager;
use anyhow::Result;
use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use winmux_shared::GuestEvent;

pub struct AgentListener {
    _join: JoinHandle<()>,
}

pub fn start(addr: &str, port_mgr: Arc<PortManager>) -> Result<AgentListener> {
    let listener = TcpListener::bind(addr)?;
    crate::log_info(&format!("agent listener: {}", listener.local_addr()?));
    let join = thread::spawn(move || {
        for incoming in listener.incoming() {
            match incoming {
                Ok(stream) => {
                    let mgr = port_mgr.clone();
                    thread::spawn(move || handle_agent(stream, mgr));
                }
                Err(e) => crate::log_warn(&format!("accept err: {e}")),
            }
        }
    });
    Ok(AgentListener { _join: join })
}

fn handle_agent(stream: std::net::TcpStream, port_mgr: Arc<PortManager>) {
    let peer = stream.peer_addr().map(|a| a.to_string()).unwrap_or_default();
    crate::log_info(&format!("agent connected from {peer}"));
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        match line {
            Ok(l) => {
                let l = l.trim();
                if l.is_empty() { continue; }
                match serde_json::from_str::<GuestEvent>(l) {
                    Ok(event) => handle_event(event, &port_mgr),
                    Err(e) => crate::log_warn(&format!("bad agent JSON: {e}; line={l}")),
                }
            }
            Err(_) => break,
        }
    }
    crate::log_info(&format!("agent disconnected: {peer}"));
}

fn handle_event(event: GuestEvent, port_mgr: &PortManager) {
    match event {
        GuestEvent::Ready { protocol_version, guest_version, kernel } => {
            crate::log_info(&format!(
                "guest ready: proto={protocol_version} version={guest_version} kernel={kernel}"
            ));
        }
        GuestEvent::PortAdded { port, bind, proto, comm, .. } => {
            crate::log_info(&format!(
                "port_added: {port}/{proto:?} bind={bind} comm={comm:?}"
            ));
            // Скіпаємо порти, які ми вже самі прокинули вручну через config (наприклад 22 для SSH).
            // Скіпаємо локальні порти, які тільки в гості потрібні (sshd на 22 — вже прокинутий)
            if port == 22 {
                return;
            }
            if let Err(e) = port_mgr.add(port) {
                crate::log_warn(&format!("hostfwd_add({port}) failed: {e}"));
            }
        }
        GuestEvent::PortRemoved { port, .. } => {
            crate::log_info(&format!("port_removed: {port}"));
            if port == 22 { return; }
            if let Err(e) = port_mgr.remove(port) {
                crate::log_warn(&format!("hostfwd_remove({port}) failed: {e}"));
            }
        }
        GuestEvent::Heartbeat { uptime_sec } => {
            crate::log_info(&format!("guest heartbeat: uptime {uptime_sec}s"));
        }
        GuestEvent::Log { level, message } => {
            match level {
                winmux_shared::LogLevel::Info => crate::log_info(&format!("[guest] {message}")),
                winmux_shared::LogLevel::Warn => crate::log_warn(&format!("[guest] {message}")),
                winmux_shared::LogLevel::Error => crate::log_error(&format!("[guest] {message}")),
            }
        }
    }
}
