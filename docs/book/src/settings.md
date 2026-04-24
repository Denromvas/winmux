# Налаштування

## Через UI (Desktop edition)

Sidebar → **⚙ Settings** — модальне вікно для редагування основних параметрів.

Доступні налаштування:
- **RAM** — пам'ять для VM (1G, 2G, 4G тощо)
- **vCPUs** — кількість віртуальних CPU
- **Accelerator** — `tcg` / `auto` / `whpx`
- **SSH port** — на хості, для доступу до guest
- **QMP/Agent ports** — внутрішні

Зміни зберігаються в `winmux.toml` і вступають у силу після наступного **Stop+Start VM**.

## Через файл `winmux.toml`

Шлях: `%LOCALAPPDATA%\WinMux\winmux.toml`

```toml
qemu_binary = "qemu/qemu-system-x86_64.exe"
disk = "rootfs/user.qcow2"
kernel = "rootfs/vmlinuz"
kernel_append = "root=/dev/vda1 rw init=/sbin/winmux-init console=ttyS0 panic=10"
ram = "2G"
smp = 4
accel = "tcg"      # "tcg" | "auto" | "whpx"
qmp_port = 4444
agent_port = 4445
ssh_port = 2223
serial_log = "logs/boot.log"
hidden = true
```

## Шрифт і теми

У sidebar: секція **View**:
- Кнопки `A−` / `14` / `A+` для розміру шрифту
- Dropdown з 6 готовими темами:
  - WinMux Dark (default)
  - Dracula
  - Tokyo Night
  - Solarized Dark
  - Catppuccin Mocha
  - GitHub Dark

Налаштування зберігаються в localStorage браузера (per-user, persistent).

## Accelerator: TCG vs WHPX

- **TCG** — програмна емуляція. Працює завжди. Boot ~2-5 сек. Сумісність 100%.
- **WHPX** — Windows Hypervisor Platform. Boot ~0.5-1 сек. Потребує:
  - HypervisorPlatform feature увімкнений (через PowerShell admin: `Enable-WindowsOptionalFeature -Online -FeatureName HypervisorPlatform -All`)
  - SLAT (Second Level Address Translation) у CPU — є на всіх Intel/AMD з 2010+
  - Не запускається в nested virt (наприклад VMware-VM зазвичай не дає SLAT)
- **auto** — спочатку пробує WHPX, якщо crash → fallback на TCG.
