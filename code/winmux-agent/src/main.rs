//! winmux-agent - port watcher / event reporter inside Linux guest.
//!
//! Підключається TCP до 10.0.2.2:AGENT_PORT (host через SLIRP NAT), шле:
//!   - Ready (один раз при старті)
//!   - PortAdded / PortRemoved (при змінах /proc/net/tcp)
//!   - Heartbeat (кожні 5 сек)

use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io::Write;
use std::net::TcpStream;
use std::thread;
use std::time::{Duration, Instant};
use winmux_shared::{GuestEvent, LogLevel, PortProto, PROTOCOL_VERSION};

const DEFAULT_HOST: &str = "10.0.2.2";
const DEFAULT_PORT: u16 = 4445;
const POLL_INTERVAL: Duration = Duration::from_millis(500);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
const RECONNECT_DELAY: Duration = Duration::from_secs(2);

fn main() -> Result<()> {
    let host = env::var("WINMUX_HOST").unwrap_or_else(|_| DEFAULT_HOST.into());
    let port: u16 = env::var("WINMUX_PORT").ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_PORT);
    let addr = format!("{host}:{port}");

    eprintln!("[winmux-agent] connecting to {addr}");

    loop {
        match run_session(&addr) {
            Ok(()) => eprintln!("[winmux-agent] session ended cleanly, reconnecting..."),
            Err(e) => eprintln!("[winmux-agent] session error: {e}; reconnecting..."),
        }
        thread::sleep(RECONNECT_DELAY);
    }
}

fn run_session(addr: &str) -> Result<()> {
    let mut stream = TcpStream::connect(addr).context("connect to host")?;
    eprintln!("[winmux-agent] connected");

    // Ready
    let kernel = fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".into());
    let ready = GuestEvent::Ready {
        protocol_version: PROTOCOL_VERSION,
        guest_version: env!("CARGO_PKG_VERSION").into(),
        kernel,
    };
    send(&mut stream, &ready)?;

    let started = Instant::now();
    let mut last_heartbeat = Instant::now();
    let mut prev_ports: HashSet<PortKey> = HashSet::new();

    loop {
        let now_ports = scan_listen_ports();
        // diff
        for p in now_ports.difference(&prev_ports) {
            let event = GuestEvent::PortAdded {
                port: p.port,
                bind: p.bind.clone(),
                proto: p.proto,
                pid: p.pid,
                comm: p.comm.clone(),
            };
            send(&mut stream, &event)?;
        }
        for p in prev_ports.difference(&now_ports) {
            let event = GuestEvent::PortRemoved {
                port: p.port,
                bind: p.bind.clone(),
                proto: p.proto,
            };
            send(&mut stream, &event)?;
        }
        prev_ports = now_ports;

        // Heartbeat
        if last_heartbeat.elapsed() >= HEARTBEAT_INTERVAL {
            let hb = GuestEvent::Heartbeat { uptime_sec: started.elapsed().as_secs() };
            send(&mut stream, &hb)?;
            last_heartbeat = Instant::now();
        }

        thread::sleep(POLL_INTERVAL);
    }
}

fn send(stream: &mut TcpStream, event: &GuestEvent) -> Result<()> {
    let mut s = serde_json::to_string(event)?;
    s.push('\n');
    stream.write_all(s.as_bytes()).context("send event")?;
    stream.flush().ok();
    Ok(())
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct PortKey {
    port: u16,
    bind: String,
    proto: PortProto,
    pid: Option<u32>,
    comm: Option<String>,
}

fn scan_listen_ports() -> HashSet<PortKey> {
    let mut out = HashSet::new();
    // Зчитуємо inode → pid/comm
    let inode_map = build_inode_map();

    if let Ok(content) = fs::read_to_string("/proc/net/tcp") {
        parse_proc_net_tcp(&content, PortProto::Tcp, &inode_map, &mut out);
    }
    if let Ok(content) = fs::read_to_string("/proc/net/tcp6") {
        parse_proc_net_tcp(&content, PortProto::Tcp6, &inode_map, &mut out);
    }
    out
}

/// Будує мапу inode → (pid, comm) шляхом скана /proc/<pid>/fd/*.
fn build_inode_map() -> HashMap<u64, (u32, String)> {
    let mut map = HashMap::new();
    let proc_dir = match fs::read_dir("/proc") {
        Ok(d) => d,
        Err(_) => return map,
    };
    for entry in proc_dir.flatten() {
        let name = entry.file_name();
        let name = match name.to_str() { Some(s) => s, None => continue };
        let pid: u32 = match name.parse() { Ok(p) => p, Err(_) => continue };
        let comm = fs::read_to_string(format!("/proc/{pid}/comm"))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let fd_dir = format!("/proc/{pid}/fd");
        if let Ok(fds) = fs::read_dir(&fd_dir) {
            for fd in fds.flatten() {
                if let Ok(target) = fs::read_link(fd.path()) {
                    let s = target.to_string_lossy();
                    // socket:[12345]
                    if let Some(stripped) = s.strip_prefix("socket:[") {
                        if let Some(num) = stripped.strip_suffix("]") {
                            if let Ok(inode) = num.parse::<u64>() {
                                map.insert(inode, (pid, comm.clone()));
                            }
                        }
                    }
                }
            }
        }
    }
    map
}

/// Парсить /proc/net/tcp{,6}, додає LISTEN-сокети.
fn parse_proc_net_tcp(
    content: &str,
    proto: PortProto,
    inodes: &HashMap<u64, (u32, String)>,
    out: &mut HashSet<PortKey>,
) {
    for line in content.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 { continue; }
        let local_addr = parts[1];
        let state = parts[3];
        if state != "0A" { continue; } // LISTEN
        let inode_idx = if proto == PortProto::Tcp { 9 } else { 9 };
        let inode: u64 = parts[inode_idx].parse().unwrap_or(0);

        let (addr_hex, port_hex) = match local_addr.split_once(':') {
            Some(p) => p,
            None => continue,
        };
        let port = u16::from_str_radix(port_hex, 16).unwrap_or(0);
        if port == 0 { continue; }

        // hex IP → readable
        let bind = match proto {
            PortProto::Tcp => parse_v4(addr_hex),
            PortProto::Tcp6 => parse_v6(addr_hex),
        };
        // skip non-listening (e.g., random low addrs); accept 0.0.0.0 / 127.0.0.1 / ::
        let pid_comm = inodes.get(&inode);
        let key = PortKey {
            port,
            bind,
            proto,
            pid: pid_comm.map(|p| p.0),
            comm: pid_comm.map(|p| p.1.clone()),
        };
        out.insert(key);
    }
}

fn parse_v4(hex: &str) -> String {
    if hex.len() != 8 { return hex.into(); }
    let mut bytes = [0u8; 4];
    for i in 0..4 {
        bytes[i] = u8::from_str_radix(&hex[i*2..i*2+2], 16).unwrap_or(0);
    }
    // /proc/net/tcp пише little-endian
    format!("{}.{}.{}.{}", bytes[3], bytes[2], bytes[1], bytes[0])
}

fn parse_v6(hex: &str) -> String {
    // Спрощено — для нашого UI достатньо
    if hex == "00000000000000000000000000000000" { "::".into() } else { hex.into() }
}
