use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Робоча директорія (де лежать qemu/, qcow2-файли тощо)
    #[serde(default = "default_workdir")]
    pub workdir: PathBuf,

    /// Шлях до qemu-system-x86_64.exe (відносно workdir або абсолютний)
    #[serde(default = "default_qemu_path")]
    pub qemu_binary: PathBuf,

    /// Шлях до диск-образу
    #[serde(default = "default_disk")]
    pub disk: PathBuf,

    /// Шлях до kernel (якщо є — використовуємо direct kernel boot)
    pub kernel: Option<PathBuf>,

    /// Append для kernel cmdline
    #[serde(default = "default_kernel_append")]
    pub kernel_append: String,

    /// RAM (в форматі QEMU: "1G", "2G", "512M")
    #[serde(default = "default_ram")]
    pub ram: String,

    /// Кількість vCPU
    #[serde(default = "default_smp")]
    pub smp: u32,

    /// Accelerator: "whpx", "tcg", "auto"
    #[serde(default = "default_accel")]
    pub accel: String,

    /// QMP TCP порт на хості (контролер підключається сюди)
    #[serde(default = "default_qmp_port")]
    pub qmp_port: u16,

    /// Агент TCP порт на хості (контролер слухає, агент підключається через NAT 10.0.2.2:port)
    #[serde(default = "default_agent_port")]
    pub agent_port: u16,

    /// SSH host port (фіксований hostfwd для SSH-доступу)
    #[serde(default = "default_ssh_port")]
    pub ssh_port: u16,

    /// Terminal mux port — host chardev socket для virtio-serial каналу терміналу
    /// (низьколатентний транспорт замість SSH+SLIRP)
    #[serde(default = "default_term_port")]
    pub term_port: u16,

    /// Шлях для serial console log
    #[serde(default = "default_serial_log")]
    pub serial_log: PathBuf,

    /// Чи використовувати hidden-window для QEMU (без console)
    #[serde(default = "default_true")]
    pub hidden: bool,
}

fn default_workdir() -> PathBuf { PathBuf::from(".") }
fn default_qemu_path() -> PathBuf { PathBuf::from("qemu/qemu-system-x86_64.exe") }
fn default_disk() -> PathBuf { PathBuf::from("init-test.qcow2") }
fn default_kernel_append() -> String {
    "root=/dev/vda1 rw init=/sbin/winmux-init console=ttyS0 panic=10".into()
}
fn default_ram() -> String { "4G".into() }
fn default_smp() -> u32 { 8 }
fn default_accel() -> String { "auto".into() }
fn default_qmp_port() -> u16 { 4444 }
fn default_agent_port() -> u16 { 4445 }
fn default_ssh_port() -> u16 { 2222 }
fn default_term_port() -> u16 { 4446 }
fn default_serial_log() -> PathBuf { PathBuf::from("boot.log") }
fn default_true() -> bool { true }

pub fn load(path: &Path) -> Result<Config> {
    let s = std::fs::read_to_string(path)?;
    let cfg: Config = toml_lite::parse(&s)?;
    Ok(cfg)
}

pub fn write_template(path: &Path) -> Result<()> {
    let template = r#"# WinMux controller config

# Робоча директорія (зазвичай тут лежать qemu/, qcow2, kernel)
workdir = "."

# Шлях до QEMU бінарника (відносно workdir)
qemu_binary = "qemu/qemu-system-x86_64.exe"

# Диск
disk = "init-test.qcow2"

# Direct kernel boot (опціонально). Якщо вказано — стартує kernel напряму без GRUB.
kernel = "vmlinuz"
kernel_append = "root=/dev/vda1 rw init=/sbin/winmux-init console=ttyS0 panic=10"

# Ресурси
ram = "1G"
smp = 8

# Accelerator: "whpx" / "tcg" / "auto" (auto = whpx якщо доступний, інакше tcg)
accel = "auto"

# Порти
qmp_port = 4444
agent_port = 4445
ssh_port = 2222

# Лог serial-console гостя
serial_log = "boot.log"

# Сховати QEMU window
hidden = true
"#;
    std::fs::write(path, template)?;
    Ok(())
}

// ----- mini TOML parser (щоб не тягнути цілий toml crate, бо нам потрібен мінімум) -----
mod toml_lite {
    use anyhow::{anyhow, Result};
    use serde::Deserialize;

    pub fn parse<T: for<'de> Deserialize<'de>>(s: &str) -> Result<T> {
        // Конвертуємо TOML в JSON через дуже простий парсер: key = value на верхньому рівні.
        // Для нашого config все скаляри (без вкладених секцій), тому це OK.
        let mut json = serde_json::Map::new();
        for raw_line in s.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
                continue;
            }
            let (k, v) = line.split_once('=').ok_or_else(|| anyhow!("bad line: {line}"))?;
            let k = k.trim().trim_matches('"').to_string();
            let v = v.trim();
            // strip trailing comment
            let v = v.split('#').next().unwrap_or(v).trim();
            let value = if v.starts_with('"') && v.ends_with('"') {
                serde_json::Value::String(v.trim_matches('"').to_string())
            } else if v == "true" || v == "false" {
                serde_json::Value::Bool(v == "true")
            } else if let Ok(n) = v.parse::<i64>() {
                serde_json::Value::Number(n.into())
            } else {
                serde_json::Value::String(v.to_string())
            };
            json.insert(k, value);
        }
        let v = serde_json::Value::Object(json);
        Ok(serde_json::from_value(v)?)
    }
}
