//! Shared types between winmux-controller (Windows) and winmux-agent (Linux guest).

use serde::{Deserialize, Serialize};

/// Версія протоколу. Змінюємо при breaking changes.
pub const PROTOCOL_VERSION: u32 = 1;

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
