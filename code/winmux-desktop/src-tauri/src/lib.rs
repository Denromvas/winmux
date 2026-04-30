//! WinMux Desktop — Tauri 2 application.
//!
//! Architecture:
//!   - Spawns winmux-controller.exe as a child process (using bundled binary).
//!   - Reads stdout/stderr → emits "controller-log" events to UI.
//!   - Polls controller status (later: dedicated IPC channel) → emits "vm-status".
//!   - For terminal: SSH into guest at 127.0.0.1:2223 via std::process::Command + PTY.
//!     For MVP we just shell out using `ssh.exe` from Windows OpenSSH (always available
//!     on Win10 1803+).

mod controller;
mod terminal;
mod telemetry;

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{Manager, State, Emitter, Listener};
use tauri::tray::{TrayIconBuilder, MouseButton, MouseButtonState, TrayIconEvent};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
#[allow(unused_imports)]
use std::sync::Arc;

#[derive(Default)]
pub struct AppState {
    pub controller: Mutex<Option<controller::ControllerHandle>>,
    pub terminals: Mutex<std::collections::HashMap<u32, terminal::TerminalHandle>>,
    pub status: Mutex<VmStatus>,
    pub opened_path: Mutex<Option<String>>,
    pub next_tab_id: Mutex<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VmStatus {
    pub state: String,        // "stopped" | "starting" | "running" | "error"
    pub pid: Option<u32>,
    pub uptime_sec: Option<u64>,
    pub ports: Vec<u16>,
    pub guest_kernel: Option<String>,
}

#[tauri::command]
fn start_vm(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.controller.lock().unwrap();
    if guard.is_some() {
        return Err("controller already running".into());
    }
    let app_log = app.clone();
    let app_status = app.clone();
    let app_status2 = app.clone();
    let handle = controller::spawn(
        app,
        move |line| {
            let _ = app_log.emit("controller-log", line);
        },
        move |status| {
            // Emit status to UI
            let _ = app_status.emit("vm-status", status.clone());
            // Auto-spawn перший PTY-tab коли VM ready (UI може створювати ще через open_tab)
            if status.state == "running" {
                let app_term = app_status2.clone();
                if let Some(ws) = app_term.try_state::<AppState>() {
                    let already = !ws.terminals.lock().unwrap().is_empty();
                    if !already {
                        std::thread::sleep(std::time::Duration::from_secs(2));
                        let _ = open_tab_internal(&app_term, &ws);
                    }
                }
            }
        },
    ).map_err(|e| e.to_string())?;

    *guard = Some(handle);
    Ok(())
}

#[tauri::command]
fn open_ssh(state: State<'_, AppState>) -> Result<(), String> {
    // Знайдемо controller workdir щоб правильно вказати конфіг (port із winmux.toml)
    let port: u16 = {
        let guard = state.controller.lock().unwrap();
        if guard.is_none() {
            return Err("VM is not running".into());
        }
        2223 // дефолтний; пізніше — читати з config
    };
    // Запускаємо Windows Terminal або PowerShell з ssh
    #[cfg(windows)]
    {
        // Спершу пробуємо Windows Terminal (wt.exe), якщо немає — звичайний PowerShell
        let mut wt = std::process::Command::new("wt.exe");
        wt.args([
            "new-tab", "--title", "WinMux SSH",
            "powershell.exe", "-NoExit", "-Command",
            &format!("ssh -p {port} -o StrictHostKeyChecking=no -o UserKnownHostsFile=NUL winmux@127.0.0.1"),
        ]);
        if wt.spawn().is_ok() { return Ok(()); }

        let mut ps = std::process::Command::new("powershell.exe");
        ps.args([
            "-NoExit", "-Command",
            &format!("ssh -p {port} -o StrictHostKeyChecking=no -o UserKnownHostsFile=NUL winmux@127.0.0.1"),
        ]);
        ps.spawn().map_err(|e| format!("failed to launch powershell: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
fn stop_vm(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    stop_vm_internal(&state)?;
    // Push fresh status to UI so the buttons re-enable.
    let st = state.status.lock().unwrap().clone();
    let _ = app.emit("vm-status", st);
    let _ = app.emit("controller-log", "[desktop] stop_vm done".to_string());
    Ok(())
}

/// Internal helper used by both `stop_vm` command and window-close handler.
fn stop_vm_internal(state: &AppState) -> Result<(), String> {
    {
        let mut guard = state.controller.lock().unwrap();
        if let Some(mut h) = guard.take() {
            let _ = h.kill();
        }
    }
    {
        let mut termg = state.terminals.lock().unwrap();
        for (_, mut t) in termg.drain() {
            let _ = t.kill();
        }
    }
    force_cleanup_processes();
    let mut st = state.status.lock().unwrap();
    st.state = "stopped".into();
    st.pid = None;
    st.uptime_sec = None;
    st.ports.clear();
    Ok(())
}

#[tauri::command]
fn force_kill_all(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let _ = stop_vm_internal(&state);
    force_cleanup_processes();
    let st = state.status.lock().unwrap().clone();
    let _ = app.emit("vm-status", st);
    let _ = app.emit("controller-log", "[desktop] force_kill_all done".to_string());
    Ok("Killed: qemu, winmux controller, ssh terminals".into())
}

#[tauri::command]
fn reset_session(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let _ = stop_vm_internal(&state);
    force_cleanup_processes();
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().ok_or("no parent for current exe")?;
    let overlay = dir.join("rootfs").join("user.qcow2");
    let st = state.status.lock().unwrap().clone();
    let _ = app.emit("vm-status", st);
    if overlay.exists() {
        std::fs::remove_file(&overlay).map_err(|e| format!("delete overlay: {e}"))?;
        let msg = format!("Reset done. Removed: {}", overlay.display());
        let _ = app.emit("controller-log", format!("[desktop] {msg}"));
        Ok(msg)
    } else {
        let _ = app.emit("controller-log", "[desktop] reset: no overlay to remove".to_string());
        Ok("Reset done. No overlay to remove.".into())
    }
}

/// Force-kill any leftover WinMux processes (own + child).
#[cfg(windows)]
fn force_cleanup_processes() {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    // Не вбиваємо самого winmux-desktop.exe (це ми) — тільки controller і qemu
    for image in ["qemu-system-x86_64.exe", "qemu-system-x86_64w.exe", "winmux.exe"] {
        let _ = std::process::Command::new("taskkill.exe")
            .args(["/F", "/T", "/IM", image])
            .creation_flags(CREATE_NO_WINDOW)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
}

#[cfg(not(windows))]
fn force_cleanup_processes() {
    // best-effort на linux/macos для розробки
    for image in ["qemu-system-x86_64", "winmux-controller"] {
        let _ = std::process::Command::new("pkill")
            .args(["-f", image])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
}

#[tauri::command]
fn send_input(tab_id: u32, data: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.terminals.lock().unwrap();
    if let Some(t) = guard.get_mut(&tab_id) {
        t.write(&data).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn open_tab_internal(app: &tauri::AppHandle, state: &AppState) -> Result<u32, String> {
    let id = {
        let mut g = state.next_tab_id.lock().unwrap();
        *g += 1;
        *g
    };
    let app_emit = app.clone();
    let handle = terminal::spawn_ssh(2223, move |chunk| {
        let _ = app_emit.emit(&format!("term-output:{id}"), chunk);
    }).map_err(|e| e.to_string())?;
    state.terminals.lock().unwrap().insert(id, handle);
    let _ = app.emit("tab-opened", id);
    Ok(id)
}

#[tauri::command]
fn open_tab(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<u32, String> {
    open_tab_internal(&app, &state)
}

#[tauri::command]
fn close_tab(tab_id: u32, state: State<'_, AppState>) -> Result<(), String> {
    let mut g = state.terminals.lock().unwrap();
    if let Some(mut t) = g.remove(&tab_id) {
        let _ = t.kill();
    }
    Ok(())
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    open::that(url).map_err(|e| e.to_string())
}

/// Returns Windows-path (or null) that was passed as CLI arg
/// (e.g. when launched via Explorer "Open in WinMux" context menu).
#[tauri::command]
fn opened_path(state: State<'_, AppState>) -> Option<String> {
    state.opened_path.lock().unwrap().clone()
}

/// Tail last N lines of guest's ~/.winmux/claude.jsonl over SSH.
/// Frontend polls this every 1s for the AI activity sidebar.
/// Returns parsed lines (one JSON event per line) — frontend filters
/// to assistant_message / tool_use / tool_result / thinking events.
#[tauri::command]
fn ai_tail() -> Result<Vec<serde_json::Value>, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let exe_dir = std::env::current_exe().ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .ok_or("no exe dir")?;
    let key = exe_dir.join("ssh").join("id_winmux_ed25519");
    let mut cmd = std::process::Command::new("ssh.exe");
    cmd.args([
        "-p", "2223", "-o", "StrictHostKeyChecking=no",
        "-o", "UserKnownHostsFile=NUL", "-o", "BatchMode=yes",
        "-o", "ConnectTimeout=2",
    ]);
    if key.exists() {
        cmd.arg("-i").arg(&key).arg("-o").arg("IdentitiesOnly=yes");
    }
    cmd.arg("winmux@127.0.0.1")
       .arg("test -f ~/.winmux/claude.jsonl && tail -50 ~/.winmux/claude.jsonl");
    cmd.creation_flags(CREATE_NO_WINDOW);
    let out = cmd.output().map_err(|e| e.to_string())?;
    let s = String::from_utf8_lossy(&out.stdout);
    let events: Vec<serde_json::Value> = s.lines()
        .filter_map(|l| serde_json::from_str(l.trim()).ok())
        .collect();
    Ok(events)
}

/// Read .winmux/config.toml from a host folder and return its content as JSON.
/// Used by the frontend when WinMux is launched against a project folder
/// (Explorer "Open in WinMux") to apply per-project init_command / RAM / etc.
#[tauri::command]
fn load_project_config(path: String) -> Option<serde_json::Value> {
    let cfg = std::path::Path::new(&path).join(".winmux").join("config.toml");
    let txt = std::fs::read_to_string(&cfg).ok()?;
    let val: toml::Value = toml::from_str(&txt).ok()?;
    serde_json::to_value(val).ok()
}

#[derive(serde::Serialize)]
struct TelemetryStatus {
    asked: bool,
    enabled: bool,
}

#[tauri::command]
fn telemetry_status() -> TelemetryStatus {
    let cfg = telemetry::load_settings();
    TelemetryStatus { asked: cfg.asked_user, enabled: cfg.enabled }
}

#[tauri::command]
fn telemetry_set(enabled: bool) {
    telemetry::set_enabled(enabled);
    telemetry::track("telemetry_optin_changed", serde_json::json!({ "enabled": enabled }));
}

#[derive(serde::Serialize)]
struct UpdateInfo {
    current: String,
    available: Option<String>,
    notes: Option<String>,
    download_url: Option<String>,
}

#[tauri::command]
fn check_update() -> Result<UpdateInfo, String> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let res = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .get("https://raw.githubusercontent.com/Denromvas/winmux/main/web/version.json")
        .call()
        .map_err(|e| format!("network: {e}"))?;
    let body: serde_json::Value = res.into_json().map_err(|e| e.to_string())?;
    let latest = body.get("latest").and_then(|v| v.as_str()).map(String::from);
    let notes = body.get("notes").and_then(|v| v.as_str()).map(String::from);
    let download_url = body.get("desktop_setup").and_then(|v| v.as_str()).map(String::from);
    let available = match (&latest, &current) {
        (Some(l), c) if l != c => latest.clone(),
        _ => None,
    };
    Ok(UpdateInfo { current, available, notes, download_url })
}

#[tauri::command]
fn download_and_install_update(url: String) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let install_dir = exe.parent().ok_or("no parent")?.to_path_buf();
    let setup = std::env::temp_dir().join("winmux-update-setup.exe");
    let res = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(300))
        .call()
        .map_err(|e| format!("download: {e}"))?;
    let mut f = std::fs::File::create(&setup).map_err(|e| e.to_string())?;
    std::io::copy(&mut res.into_reader(), &mut f).map_err(|e| e.to_string())?;
    drop(f);
    // Запускаємо silent install у ту саму папку
    std::process::Command::new(&setup)
        .args(["/S", &format!("/D={}", install_dir.display())])
        .spawn()
        .map_err(|e| format!("spawn setup: {e}"))?;
    // Завершуємо себе щоб дати installer-у замінити exe
    std::process::exit(0);
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsConfig {
    pub ram: String,
    pub smp: u32,
    pub accel: String,
    pub ssh_port: u16,
    pub qmp_port: u16,
    pub agent_port: u16,
}

#[tauri::command]
fn read_settings() -> Result<SettingsConfig, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let cfg_path = exe.parent().ok_or("no parent")?.join("winmux.toml");
    let s = std::fs::read_to_string(&cfg_path)
        .map_err(|e| format!("read {}: {e}", cfg_path.display()))?;

    let mut ram = "2G".to_string();
    let mut smp: u32 = 4;
    let mut accel = "tcg".to_string();
    let mut ssh_port: u16 = 2223;
    let mut qmp_port: u16 = 4444;
    let mut agent_port: u16 = 4445;

    for line in s.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if let Some((k, v)) = line.split_once('=') {
            let k = k.trim();
            let v = v.trim().trim_matches('"');
            match k {
                "ram" => ram = v.into(),
                "smp" => smp = v.parse().unwrap_or(smp),
                "accel" => accel = v.into(),
                "ssh_port" => ssh_port = v.parse().unwrap_or(ssh_port),
                "qmp_port" => qmp_port = v.parse().unwrap_or(qmp_port),
                "agent_port" => agent_port = v.parse().unwrap_or(agent_port),
                _ => {}
            }
        }
    }
    Ok(SettingsConfig { ram, smp, accel, ssh_port, qmp_port, agent_port })
}

#[tauri::command]
fn write_settings(cfg: SettingsConfig) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let cfg_path = exe.parent().ok_or("no parent")?.join("winmux.toml");
    let s = std::fs::read_to_string(&cfg_path).map_err(|e| e.to_string())?;
    // Просте rewrite по ключах, зберігаючи інші параметри
    let mut out = String::new();
    let keys = ["ram", "smp", "accel", "ssh_port", "qmp_port", "agent_port"];
    let mut written = std::collections::HashSet::new();
    for line in s.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            out.push_str(line); out.push('\n'); continue;
        }
        if let Some((k, _)) = trimmed.split_once('=') {
            let k = k.trim();
            if keys.contains(&k) {
                let new_val = match k {
                    "ram" => format!("ram = \"{}\"", cfg.ram),
                    "smp" => format!("smp = {}", cfg.smp),
                    "accel" => format!("accel = \"{}\"", cfg.accel),
                    "ssh_port" => format!("ssh_port = {}", cfg.ssh_port),
                    "qmp_port" => format!("qmp_port = {}", cfg.qmp_port),
                    "agent_port" => format!("agent_port = {}", cfg.agent_port),
                    _ => unreachable!(),
                };
                out.push_str(&new_val); out.push('\n');
                written.insert(k);
                continue;
            }
        }
        out.push_str(line); out.push('\n');
    }
    // Якщо якісь не було — додаємо в кінець
    for k in keys {
        if !written.contains(k) {
            let line = match k {
                "ram" => format!("ram = \"{}\"\n", cfg.ram),
                "smp" => format!("smp = {}\n", cfg.smp),
                "accel" => format!("accel = \"{}\"\n", cfg.accel),
                "ssh_port" => format!("ssh_port = {}\n", cfg.ssh_port),
                "qmp_port" => format!("qmp_port = {}\n", cfg.qmp_port),
                "agent_port" => format!("agent_port = {}\n", cfg.agent_port),
                _ => unreachable!(),
            };
            out.push_str(&line);
        }
    }
    std::fs::write(&cfg_path, out).map_err(|e| e.to_string())?;
    Ok(())
}

/// Drag-and-drop helper: для кожного дропнутого Windows-шляху повертає
/// "поліпшений" текст для вставки в термінал.
/// Якщо файл уже на спільній ФС (через sshfs ~/win) — даємо guest-path.
/// Інакше копіюємо у $HOME/.winmux-drops/ через scp і даємо guest-path до неї.
/// (MVP: просто інвертуємо шлях C:\Users\... → /mnt/c/Users/... — гість сам зробить mkdir+copy
///  через Windows OpenSSH назад. Поки тільки повертаємо текст для вставки.)
#[tauri::command]
fn drop_paths(paths: Vec<String>) -> Result<String, String> {
    // Простий формат: shell-escaped шляхи через пробіл, конвертовані до POSIX-стилю
    // (це не sshfs-mount шлях, а просто візуально красиво — що користувач може потім використати).
    let parts: Vec<String> = paths.iter().map(|p| {
        let win = p.replace('\\', "/");
        // Якщо містить пробіли — лапки
        if win.contains(' ') { format!("'{win}'") } else { win }
    }).collect();
    Ok(parts.join(" "))
}

/// Зберегти зображення (binary) у %LOCALAPPDATA%\WinMux\drops\img-TIMESTAMP.png
/// і повернути POSIX-шлях для інсерту.
#[tauri::command]
fn save_image_drop(bytes: Vec<u8>, ext: String) -> Result<String, String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().ok_or("no parent")?.join("drops");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let safe_ext = if ext.is_empty() || ext.len() > 5 { "png".into() } else { ext };
    let filename = format!("drop-{ts}.{safe_ext}");
    let path = dir.join(&filename);
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    let win_path = path.to_string_lossy().to_string();
    Ok(win_path.replace('\\', "/"))
}

#[tauri::command]
fn resize_term(tab_id: u32, cols: u16, rows: u16, state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.terminals.lock().unwrap();
    if let Some(t) = guard.get(&tab_id) {
        t.resize(cols, rows).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn window_minimize(window: tauri::Window) -> Result<(), String> {
    window.minimize().map_err(|e| e.to_string())
}

#[tauri::command]
fn window_maximize(window: tauri::Window) -> Result<(), String> {
    if window.is_maximized().unwrap_or(false) {
        window.unmaximize().map_err(|e| e.to_string())
    } else {
        window.maximize().map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn window_close(window: tauri::Window) -> Result<(), String> {
    window.close().map_err(|e| e.to_string())
}

// ---------- Snapshots: shell out to bundled winmux.exe ----------

fn winmux_exe_path() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("winmux.exe")))
        .unwrap_or_else(|| std::path::PathBuf::from("winmux.exe"))
}

fn run_winmux(args: &[&str]) -> Result<String, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let out = std::process::Command::new(winmux_exe_path())
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    let s = String::from_utf8_lossy(&out.stdout).to_string()
        + &String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        Err(s)
    } else {
        Ok(s)
    }
}

#[tauri::command]
fn snapshot_save(name: String) -> Result<String, String> {
    run_winmux(&["snapshot", "save", &name])
}

#[tauri::command]
fn snapshot_restore(name: String) -> Result<String, String> {
    run_winmux(&["snapshot", "restore", &name])
}

#[tauri::command]
fn snapshot_delete(name: String) -> Result<String, String> {
    run_winmux(&["snapshot", "delete", &name])
}

#[tauri::command]
fn snapshot_list() -> Result<String, String> {
    run_winmux(&["snapshot", "list"])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            start_vm,
            stop_vm,
            force_kill_all,
            reset_session,
            send_input,
            resize_term,
            open_tab,
            close_tab,
            open_url,
            open_ssh,
            opened_path,
            load_project_config,
            ai_tail,
            read_settings,
            write_settings,
            telemetry_status,
            telemetry_set,
            check_update,
            download_and_install_update,
            drop_paths,
            save_image_drop,
            window_minimize,
            window_maximize,
            window_close,
            snapshot_save,
            snapshot_restore,
            snapshot_delete,
            snapshot_list,
        ])
        .setup(|app| {
            // CLI args: якщо запущено через "Open in WinMux" з Explorer →
            // arg 1 = path до папки. Передамо у frontend через event при ready.
            let args: Vec<String> = std::env::args().skip(1).collect();
            if let Some(path) = args.first() {
                let p = path.clone();
                // Store у state, frontend забере через invoke
                if let Some(s) = app.try_state::<AppState>() {
                    *s.opened_path.lock().unwrap() = Some(p);
                }
            }

            // Auto-cleanup any zombies from a previous crash so we start clean.
            force_cleanup_processes();

            // Init telemetry; track app_start (no-op if disabled / not opted-in yet).
            telemetry::init();
            telemetry::track("app_start", serde_json::json!({}));

            // Zero-config UX: auto-start VM в фоні (через 200ms щоб UI вспів стати на ноги).
            // Користувач може вимкнути через winmux.toml: auto_start = false
            let auto_start = std::env::current_exe().ok()
                .and_then(|exe| exe.parent().map(|p| p.join("winmux.toml")))
                .and_then(|p| std::fs::read_to_string(p).ok())
                .map(|s| !s.lines().any(|l| l.trim() == "auto_start = false"))
                .unwrap_or(true);
            if auto_start {
                let app_clone = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    if let Some(state) = app_clone.try_state::<AppState>() {
                        let app_log = app_clone.clone();
                        let app_status = app_clone.clone();
                        let app_status2 = app_clone.clone();
                        if state.controller.lock().unwrap().is_none() {
                            let result = controller::spawn(
                                app_clone.clone(),
                                move |line| { let _ = app_log.emit("controller-log", line); },
                                move |status| {
                                    let _ = app_status.emit("vm-status", status.clone());
                                    if status.state == "running" {
                                        let app_term = app_status2.clone();
                                        if let Some(ws) = app_term.try_state::<AppState>() {
                                            let already = !ws.terminals.lock().unwrap().is_empty();
                                            if !already {
                                                std::thread::sleep(std::time::Duration::from_secs(2));
                                                let _ = open_tab_internal(&app_term, &ws);
                                            }
                                        }
                                    }
                                },
                            );
                            match result {
                                Ok(handle) => *state.controller.lock().unwrap() = Some(handle),
                                Err(e) => {
                                    let _ = app_clone.emit("controller-log",
                                        format!("[desktop] auto-start failed: {e}"));
                                }
                            }
                        }
                    }
                });
            }

            // System tray
            let show_item = MenuItem::with_id(app, "show", "Show WinMux", true, None::<&str>)?;
            let start_item = MenuItem::with_id(app, "start", "Start VM", true, None::<&str>)?;
            let stop_item = MenuItem::with_id(app, "stop", "Stop VM", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(app)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[&show_item, &start_item, &stop_item, &separator, &quit_item],
            )?;

            let tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("WinMux • Stopped")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "show" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.set_focus();
                                let _ = w.unminimize();
                            }
                        }
                        "start" => {
                            // Reuse start_vm command по invoke
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.eval("window.__TAURI__?.core.invoke('start_vm');");
                            }
                        }
                        "stop" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.eval("window.__TAURI__?.core.invoke('stop_vm');");
                            }
                        }
                        "quit" => {
                            // graceful: kill VM, then exit
                            if let Some(s) = app.try_state::<AppState>() {
                                let _ = stop_vm_internal(&s);
                            }
                            force_cleanup_processes();
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        if let Some(w) = tray.app_handle().get_webview_window("main") {
                            if w.is_visible().unwrap_or(false) {
                                let _ = w.hide();
                            } else {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            // Watch vm-status events from frontend → keep tray tooltip + window title in sync.
            // This is our minimal "tray notification": passive but always visible on hover.
            let tray_handle = tray.clone();
            let app_handle_for_listen = app.handle().clone();
            app.listen("vm-status", move |event| {
                let payload = event.payload();
                let state = if payload.contains("\"running\"") { "Running ✓" }
                    else if payload.contains("\"starting\"") { "Starting…" }
                    else if payload.contains("\"error\"") { "Error ⚠" }
                    else { "Stopped" };
                let _ = tray_handle.set_tooltip(Some(format!("WinMux • {state}")));
                if let Some(w) = app_handle_for_listen.get_webview_window("main") {
                    let _ = w.set_title(&format!("WinMux • {state}"));
                }
            });
            // Перехоплюємо закриття вікна щоб обов'язково kill дочірні процеси.
            let app_handle = app.handle().clone();
            if let Some(window) = app.get_webview_window("main") {
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { .. } = event {
                        if let Some(s) = app_handle.try_state::<AppState>() {
                            let _ = stop_vm_internal(&s);
                        }
                        force_cleanup_processes();
                    }
                });
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
