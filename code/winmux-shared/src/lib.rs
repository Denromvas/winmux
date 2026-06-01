//! Shared types between winmux-controller (Windows) and winmux-agent (Linux guest).

use serde::{Deserialize, Serialize};

/// Версія протоколу. Змінюємо при breaking changes.
pub const PROTOCOL_VERSION: u32 = 1;

/// Terminal multiplexer over a single virtio-serial port (host ⇄ guest).
///
/// One byte-channel per terminal tab, framed so we can multiplex many tabs +
/// carry resize out-of-band on the SAME port. This replaces the SSH-over-SLIRP
/// terminal path: no crypto, no userspace TCP/IP stack — just the virtio ring.
///
/// Wire frame (big-endian): `[u32 channel][u8 kind][u32 len][payload len bytes]`
pub mod termmux {
    use std::io::{self, Read, Write};

    pub const KIND_OPEN: u8 = 1; // host→guest, payload = size (cols,rows): spawn shell
    pub const KIND_DATA: u8 = 2; // both ways, payload = raw bytes
    pub const KIND_RESIZE: u8 = 3; // host→guest, payload = size (cols,rows)
    pub const KIND_CLOSE: u8 = 4; // both ways, payload empty: shell gone / close tab

    /// Max single frame payload we accept (guards against desync). 1 MiB is
    /// far above any real terminal write.
    pub const MAX_PAYLOAD: u32 = 1 << 20;

    #[derive(Debug, Clone)]
    pub struct Frame {
        pub channel: u32,
        pub kind: u8,
        pub payload: Vec<u8>,
    }

    impl Frame {
        pub fn open(channel: u32, cols: u16, rows: u16) -> Self {
            Frame { channel, kind: KIND_OPEN, payload: pack_size(cols, rows) }
        }
        pub fn data(channel: u32, bytes: Vec<u8>) -> Self {
            Frame { channel, kind: KIND_DATA, payload: bytes }
        }
        pub fn resize(channel: u32, cols: u16, rows: u16) -> Self {
            Frame { channel, kind: KIND_RESIZE, payload: pack_size(cols, rows) }
        }
        pub fn close(channel: u32) -> Self {
            Frame { channel, kind: KIND_CLOSE, payload: Vec::new() }
        }

        /// Serialize header + payload into one buffer (single write = one frame).
        pub fn encode(&self) -> Vec<u8> {
            let mut out = Vec::with_capacity(9 + self.payload.len());
            out.extend_from_slice(&self.channel.to_be_bytes());
            out.push(self.kind);
            out.extend_from_slice(&(self.payload.len() as u32).to_be_bytes());
            out.extend_from_slice(&self.payload);
            out
        }

        /// Write the frame to `w` in one shot and flush.
        pub fn write_to<W: Write>(&self, w: &mut W) -> io::Result<()> {
            w.write_all(&self.encode())?;
            w.flush()
        }

        /// Block-read exactly one frame from `r`. Returns Err on EOF / bad length.
        pub fn read_from<R: Read>(r: &mut R) -> io::Result<Frame> {
            let mut hdr = [0u8; 9];
            r.read_exact(&mut hdr)?;
            let channel = u32::from_be_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]);
            let kind = hdr[4];
            let len = u32::from_be_bytes([hdr[5], hdr[6], hdr[7], hdr[8]]);
            if len > MAX_PAYLOAD {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
            }
            let mut payload = vec![0u8; len as usize];
            r.read_exact(&mut payload)?;
            Ok(Frame { channel, kind, payload })
        }

        /// Interpret payload as (cols, rows) for OPEN / RESIZE.
        pub fn size(&self) -> Option<(u16, u16)> {
            if self.payload.len() >= 4 {
                let cols = u16::from_be_bytes([self.payload[0], self.payload[1]]);
                let rows = u16::from_be_bytes([self.payload[2], self.payload[3]]);
                Some((cols, rows))
            } else {
                None
            }
        }
    }

    pub fn pack_size(cols: u16, rows: u16) -> Vec<u8> {
        let mut v = Vec::with_capacity(4);
        v.extend_from_slice(&cols.to_be_bytes());
        v.extend_from_slice(&rows.to_be_bytes());
        v
    }
}

/// Подія, яку Guest Agent надсилає до контролера.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GuestEvent {
    /// Гість готовий — agent стартанув.
    Ready {
        protocol_version: u32,
        guest_version: String,
        kernel: String,
    },
    /// Новий LISTEN порт у госту.
    PortAdded {
        port: u16,
        bind: String,           // "0.0.0.0", "127.0.0.1", "::"
        proto: PortProto,       // tcp / tcp6
        pid: Option<u32>,
        comm: Option<String>,
    },
    /// Порт перестав слухатися.
    PortRemoved {
        port: u16,
        bind: String,
        proto: PortProto,
    },
    /// Heartbeat (раз на 5-10 секунд).
    Heartbeat {
        uptime_sec: u64,
    },
    /// Інша діагностика.
    Log {
        level: LogLevel,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PortProto {
    Tcp,
    Tcp6,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

/// Команда host → guest (для майбутнього).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum HostCommand {
    /// Експорт env-змінних у /home/winmux/.profile.
    SetEnv { vars: Vec<(String, String)> },
    /// Інжект тексту в активну tmux-сесію.
    InjectText { session: String, text: String },
    /// Виконати команду в tmux-сесії.
    RunInTmux { session: String, command: String },
    /// Запит на shutdown.
    Shutdown,
    /// Запит heartbeat.
    Ping,
}

/// Уніфікована обгортка для повідомлень — спрощує парсинг по один-рядок-один-JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Message {
    Event(GuestEvent),
    Command(HostCommand),
}
