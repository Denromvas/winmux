//! winmux-controller - Windows-side daemon.
//!
//! Запускає QEMU, керує його lifecycle, проксіює QMP, отримує події
//! від guest agent, автоматично робить hostfwd_add/del.

mod config;
mod qemu;
mod qmp;
mod agent_listener;
mod port_manager;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "winmux-controller", version, about = "WinMux controller daemon")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Запустити guest VM і слухати події
    Start {
        /// Путь до конфігу TOML
        #[arg(short, long, default_value = "winmux.toml")]
        config: PathBuf,
    },
    /// Створити шаблон конфігу
    InitConfig {
        #[arg(default_value = "winmux.toml")]
        path: PathBuf,
    },
    /// Відкрити SSH у запущений guest (через PowerShell/cmd)
    Ssh {
        /// Команда для виконання у guest (інакше — інтерактивна оболонка)
        cmd: Vec<String>,
    },
    /// Показати tail boot.log
    Logs {
        #[arg(short = 'n', long, default_value = "30")]
        lines: usize,
    },
    /// Перевірити чи VM запущена + порти
    Status,
    /// Зупинити QEMU процеси (force kill)
    Stop,
    /// Версія
    Version,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Version => {
            println!("winmux-controller {}", env!("CARGO_PKG_VERSION"));
            println!("protocol version: {}", winmux_shared::PROTOCOL_VERSION);
        }
        Cmd::InitConfig { path } => {
            config::write_template(&path)?;
            println!("Config written: {}", path.display());
        }
        Cmd::Start { config } => {
            run(&config)?;
        }
        Cmd::Ssh { cmd } => {
            cmd_ssh(&cmd)?;
        }
        Cmd::Logs { lines } => {
            cmd_logs(lines)?;
        }
        Cmd::Status => {
            cmd_status()?;
        }
        Cmd::Stop => {
            cmd_stop()?;
        }
    }
    Ok(())
}

fn read_ssh_port() -> u16 {
    std::env::current_exe().ok()
        .and_then(|exe| exe.parent().map(|p| p.join("winmux.toml")))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| s.lines()
            .find_map(|l| l.trim().strip_prefix("ssh_port = ")
                .and_then(|v| v.trim().parse().ok())))
        .unwrap_or(2223)
}

fn cmd_ssh(args: &[String]) -> Result<()> {
    let port = read_ssh_port();
    let exe_dir = std::env::current_exe()?.parent().unwrap().to_path_buf();
    let key = exe_dir.join("ssh").join("id_winmux_ed25519");

    let mut cmd = std::process::Command::new("ssh.exe");
    cmd.arg("-p").arg(port.to_string())
       .arg("-o").arg("StrictHostKeyChecking=no")
       .arg("-o").arg("UserKnownHostsFile=NUL");
    if key.exists() {
        cmd.arg("-i").arg(&key)
           .arg("-o").arg("IdentitiesOnly=yes");
    }
    cmd.arg("winmux@127.0.0.1");
    if !args.is_empty() {
        cmd.args(args);
    }
    let status = cmd.status().context("spawn ssh.exe")?;
    std::process::exit(status.code().unwrap_or(1));
}

fn cmd_logs(lines: usize) -> Result<()> {
    let exe_dir = std::env::current_exe()?.parent().unwrap().to_path_buf();
    let log = exe_dir.join("logs").join("boot.log");
    if !log.exists() {
        anyhow::bail!("no log at {}", log.display());
    }
    let content = std::fs::read_to_string(&log)?;
    let total: Vec<&str> = content.lines().collect();
    let from = total.len().saturating_sub(lines);
    for line in &total[from..] {
        println!("{line}");
    }
    Ok(())
}

fn cmd_status() -> Result<()> {
    use std::net::TcpStream;
    use std::time::Duration;

    let port = read_ssh_port();
    let exe_dir = std::env::current_exe()?.parent().unwrap().to_path_buf();

    println!("WinMux v{}", env!("CARGO_PKG_VERSION"));
    println!("Install:  {}", exe_dir.display());

    // QEMU pid?
    #[cfg(windows)]
    {
        let out = std::process::Command::new("tasklist.exe")
            .args(["/FI", "IMAGENAME eq qemu-system-x86_64.exe", "/FO", "CSV", "/NH"])
            .output();
        if let Ok(o) = out {
            let s = String::from_utf8_lossy(&o.stdout);
            if s.contains("qemu-system-x86_64") {
                let parts: Vec<&str> = s.split(',').collect();
                let pid = parts.get(1).map(|p| p.trim_matches('"')).unwrap_or("?");
                println!("QEMU:     running (PID {pid})");
            } else {
                println!("QEMU:     not running");
            }
        }
    }

    // SSH port reachable?
    match TcpStream::connect_timeout(&format!("127.0.0.1:{port}").parse().unwrap(), Duration::from_secs(2)) {
        Ok(_) => println!("SSH:     127.0.0.1:{port} ✓"),
        Err(_) => println!("SSH:     127.0.0.1:{port} ✗ (not reachable)"),
    }
    Ok(())
}

fn cmd_stop() -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        for image in ["qemu-system-x86_64.exe", "qemu-system-x86_64w.exe", "winmux.exe"] {
            let _ = std::process::Command::new("taskkill.exe")
                .args(["/F", "/T", "/IM", image])
                .creation_flags(CREATE_NO_WINDOW)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
        println!("Killed: qemu-system-x86_64, winmux");
    }
    Ok(())
}

fn run(config_path: &PathBuf) -> Result<()> {
    let cfg = config::load(config_path)
        .with_context(|| format!("loading config {}", config_path.display()))?;
    log_info(&format!("config loaded: workdir={}", cfg.workdir.display()));

    // Ctrl-C / SIGTERM handler
    let shutdown = Arc::new(AtomicBool::new(false));
    {
        let s = shutdown.clone();
        ctrlc_simple(move || {
            log_info("ctrl-c received, shutting down...");
            s.store(true, Ordering::SeqCst);
        });
    }

    // Старт QEMU
    let mut vm = qemu::Vm::launch(&cfg)?;
    log_info(&format!("QEMU launched: PID {}", vm.pid()));

    // Чекаємо поки QMP стане доступний
    let qmp_addr = format!("127.0.0.1:{}", cfg.qmp_port);
    let mut qmp = qmp::QmpClient::connect_with_retry(&qmp_addr, Duration::from_secs(15))
        .with_context(|| format!("QMP connect to {qmp_addr}"))?;
    log_info(&format!("QMP connected at {qmp_addr}"));

    // Стартуємо port manager
    let port_mgr = Arc::new(port_manager::PortManager::new(qmp.command_sender()));

    // Слухаємо guest agent (TCP)
    let agent_listen = format!("127.0.0.1:{}", cfg.agent_port);
    let _agent = agent_listener::start(&agent_listen, port_mgr.clone())?;
    log_info(&format!("agent listener on {agent_listen}"));

    // Спіннемо QMP read loop в окремому потоці
    qmp.spawn_reader();

    log_info("READY. Press Ctrl-C to stop.");

    // Main loop: чекаємо shutdown signal
    while !shutdown.load(Ordering::SeqCst) {
        if !vm.is_alive() {
            log_warn("QEMU process died unexpectedly");
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    log_info("stopping QEMU...");
    let _ = vm.shutdown_graceful();
    std::thread::sleep(Duration::from_secs(2));
    if vm.is_alive() {
        log_warn("graceful shutdown timeout, killing");
        let _ = vm.kill();
    }
    log_info("done.");
    Ok(())
}

// ----- minimal logging -----
pub fn log_info(msg: &str) {
    println!("[INFO ] {} {msg}", now_iso());
}
pub fn log_warn(msg: &str) {
    println!("[WARN ] {} {msg}", now_iso());
}
pub fn log_error(msg: &str) {
    eprintln!("[ERROR] {} {msg}", now_iso());
}
fn now_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = d.as_secs();
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

// ----- minimal Ctrl-C handler без зовнішніх crate -----
fn ctrlc_simple<F: Fn() + Send + 'static>(callback: F) {
    #[cfg(windows)]
    {
        use std::sync::Mutex;
        static CB: Mutex<Option<Box<dyn Fn() + Send>>> = Mutex::new(None);
        *CB.lock().unwrap() = Some(Box::new(callback));
        unsafe extern "system" fn handler(_: u32) -> i32 {
            if let Some(cb) = CB.lock().unwrap().as_ref() {
                cb();
            }
            1 // TRUE — handled
        }
        unsafe {
            extern "system" {
                fn SetConsoleCtrlHandler(handler: unsafe extern "system" fn(u32) -> i32, add: i32) -> i32;
            }
            SetConsoleCtrlHandler(handler, 1);
        }
    }
    #[cfg(unix)]
    {
        // Тестування на Linux — простий signal handler
        use std::sync::Mutex;
        static CB: Mutex<Option<Box<dyn Fn() + Send>>> = Mutex::new(None);
        *CB.lock().unwrap() = Some(Box::new(callback));
        unsafe extern "C" fn handler(_: i32) {
            if let Some(cb) = CB.lock().unwrap().as_ref() {
                cb();
            }
        }
        unsafe {
            libc_signal(2 /* SIGINT */, handler as usize);
        }
    }
}

#[cfg(unix)]
unsafe fn libc_signal(sig: i32, handler: usize) {
    extern "C" {
        fn signal(sig: i32, handler: usize) -> usize;
    }
    signal(sig, handler);
}
