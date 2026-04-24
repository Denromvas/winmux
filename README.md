# WinMux

> Портативне Linux-середовище для Windows у стилі Termux. Без WSL, Hyper-V, прав адміністратора.
> Створене для повноцінної роботи AI-агентів (особливо `claude_code`) на Windows-десктопах і віддалених Windows Server через RDP.

![logo](assets/branding/winmux-v2.png)

## Що це

- Один портативний `.exe` (~510 МБ).
- При запуску в фоні стартує мікро-Linux (Ubuntu 24.04 Minimal у QEMU user-mode).
- Спільний `localhost`: будь-який порт, відкритий у Linux, автоматично доступний на Windows `127.0.0.1:N`.
- Власний термінал на Tauri 2 + xterm.js: 6 тем, drag-and-drop, кастомний titlebar.
- Передвстановлено в Linux: `node` v22, `npm`, `claude` CLI, `git`, `tmux`, `sshfs`, `sshpass`.
- Без прав адміністратора — установка в `%LOCALAPPDATA%\WinMux\`.

## Дві edition

| | CLI | Desktop |
|---|---|---|
| Розмір | 506 МБ | 508 МБ |
| UI | через PowerShell | Tauri vікно з xterm.js |
| Use case | DevOps, Windows Server, scripts | Розробники, AI-агенти, drag-and-drop |
| Запуск | `winmux start` | подвійний клік на `winmux-desktop.exe` |

## Швидкий старт

1. Скачай `winmux-desktop-setup-v0.1.0.exe`
2. Запусти (без admin, у %LOCALAPPDATA%\WinMux\)
3. Натисни "Start VM" → за ~10 сек з'явиться bash у вікні
4. Логін: `winmux / winmux`

```bash
# у guest Linux
winmux-mount         # змонтуй Windows-папку → ~/win
claude               # API key через export ANTHROPIC_API_KEY="sk-ant-..."
```

## Документація

- [Технічне завдання (повне)](docs/TZ.md) — 27 розділів, ~1300 рядків
- [Findings PoC + Етап 1](research/poc-findings.md) — результати вимірів і експериментів
- [Roadmap](#roadmap)

## Архітектура

```
┌────────────────────────────────────────────────────────┐
│                     WINDOWS HOST                        │
│  ┌──────────────────┐    ┌──────────────────────┐     │
│  │  winmux-desktop  │◄──►│  winmux-controller   │     │
│  │  (Tauri+xterm.js)│IPC │  (lifecycle, QMP)    │     │
│  └──────────────────┘    └──────────┬───────────┘     │
│                                      │ spawns          │
│                            ┌─────────▼────────┐        │
│                            │ qemu-system-x86_64│        │
│                            └─────────┬────────┘        │
└──────────────────────────────────────┼──────────────────┘
                                       │ virtio-{net,serial}
                          ┌────────────▼──────────────┐
                          │   Ubuntu 24.04 Minimal    │
                          │   ┌─────────────────────┐ │
                          │   │ winmux-init (PID 1) │ │
                          │   │ ↓ winmux-agent      │ │
                          │   │ ↓ sshd + bash + ... │ │
                          │   └─────────────────────┘ │
                          └───────────────────────────┘
```

## Стек

| Component | Tech |
|-----------|------|
| Hypervisor | QEMU 11.0 (user-mode, no admin) |
| Guest OS | Ubuntu 24.04 Minimal |
| Init | Custom `winmux-init` in Rust (replaces systemd, ~300 ms boot) |
| Network | SLIRP NAT + agent-driven QMP `hostfwd_add` |
| Desktop UI | Tauri 2 + React 18 + xterm.js + WebGL renderer |
| Controller | Rust, no async, std::net + std::thread |
| Cross-compile | mingw-w64 (Windows), musl (Linux guest) |
| Installer | NSIS (per-user, no admin) |

## Збірка з джерел

```bash
# Залежності (Linux):
sudo apt install -y mingw-w64 musl-tools nsis nodejs npm

# Rust + targets:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
rustup target add x86_64-pc-windows-gnu x86_64-unknown-linux-musl

# Tauri CLI:
npm install -g @tauri-apps/cli@^2

# Build:
cd code/winmux-desktop/ui && npm install && cd -
bash scripts/build-desktop.sh
bash scripts/build-cli.sh
# → dist/winmux-desktop-setup-v0.1.0.exe
# → dist/winmux-cli-setup-v0.1.0.exe
```

## Roadmap

### v0.1.0 (alpha) — DONE ✅
- Core engine, PTY terminal, drag-and-drop, themes, NSIS installers, frozen-v5 з Claude Code

### v0.2.0 (beta) — IN PROGRESS
- Auto-mount Windows folders via SSH key (zero-config)
- Multi-tab + splits у Tauri
- Settings UI (без editing TOML вручну)
- System tray integration
- Auto-update via Tauri updater
- Documentation site

### v1.0 (public)
- Code signing (SignPath.io for OSS)
- Telemetry (opt-out, self-hosted)
- Бета-програма (5-10 тестерів)
- Landing page

## Ліцензія

MIT — див. [LICENSE](LICENSE). Bundled QEMU під GPLv2 (qemu/COPYING).

## Автор

Denis Romanyuk · claudetaistra@gmail.com
