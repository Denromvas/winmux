//! Anonymous telemetry client.
//! Stricter than usual: opt-out, self-hosted, no IP/file/command content collected.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

const ENDPOINT: &str = "https://telemetry.denromvas.website/v1/event";
const TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Serialize)]
struct Event<'a> {
    install_uuid: String,
    event_type: &'a str,
    winmux_version: &'a str,
    os_version: String,
    qemu_version: Option<String>,
    payload: serde_json::Value,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Settings {
    pub enabled: bool,
    pub install_uuid: String,
    pub asked_user: bool,  // чи показано welcome opt-out screen
}

fn settings_path() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    let dir = exe.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    dir.join("telemetry.toml")
}

pub fn load_settings() -> Settings {
    let p = settings_path();
    let s = std::fs::read_to_string(&p).unwrap_or_default();
    let mut cfg = Settings::default();
    for line in s.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if let Some((k, v)) = line.split_once('=') {
            let k = k.trim();
            let v = v.trim().trim_matches('"');
            match k {
                "enabled" => cfg.enabled = v == "true",
                "install_uuid" => cfg.install_uuid = v.into(),
                "asked_user" => cfg.asked_user = v == "true",
                _ => {}
            }
        }
    }
    if cfg.install_uuid.is_empty() {
        cfg.install_uuid = uuid::Uuid::new_v4().to_string();
        let _ = save_settings(&cfg);
    }
    cfg
}

pub fn save_settings(cfg: &Settings) -> std::io::Result<()> {
    let s = format!(
        "enabled = {}\ninstall_uuid = \"{}\"\nasked_user = {}\n",
        cfg.enabled, cfg.install_uuid, cfg.asked_user,
    );
    std::fs::write(settings_path(), s)
}

static SETTINGS: Mutex<Option<Settings>> = Mutex::new(None);

pub fn init() -> Settings {
    let cfg = load_settings();
    *SETTINGS.lock().unwrap() = Some(Settings {
        enabled: cfg.enabled,
        install_uuid: cfg.install_uuid.clone(),
        asked_user: cfg.asked_user,
    });
    cfg
}

pub fn set_enabled(enabled: bool) {
    let mut g = SETTINGS.lock().unwrap();
    if let Some(s) = g.as_mut() {
        s.enabled = enabled;
        s.asked_user = true;
        let _ = save_settings(s);
    }
}

pub fn is_asked() -> bool {
    SETTINGS.lock().unwrap().as_ref().map(|s| s.asked_user).unwrap_or(false)
}

pub fn install_uuid() -> Option<String> {
    SETTINGS.lock().unwrap().as_ref().map(|s| s.install_uuid.clone())
}

pub fn track(event_type: &'static str, payload: serde_json::Value) {
    let cfg = match SETTINGS.lock().unwrap().as_ref() {
        Some(s) if s.enabled => s.install_uuid.clone(),
        _ => return,
    };
    let winmux_version = env!("CARGO_PKG_VERSION");
    let os_version = os_string();
    let qemu_version: Option<String> = None;
    thread::spawn(move || {
        let event = Event {
            install_uuid: cfg,
            event_type,
            winmux_version,
            os_version,
            qemu_version,
            payload,
        };
        let body = match serde_json::to_string(&event) {
            Ok(b) => b,
            Err(_) => return,
        };
        let _ = ureq::AgentBuilder::new()
            .timeout(TIMEOUT)
            .build()
            .post(ENDPOINT)
            .set("Content-Type", "application/json")
            .send_string(&body);
    });
}

fn os_string() -> String {
    #[cfg(windows)]
    {
        // Простий спосіб без extra deps: викликати "ver"
        if let Ok(out) = std::process::Command::new("cmd").args(["/C", "ver"]).output() {
            return String::from_utf8_lossy(&out.stdout).trim().to_string();
        }
    }
    "unknown".into()
}
