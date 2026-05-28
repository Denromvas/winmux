# WinMux

> **Run a full Linux environment — and AI coding agents like Claude Code — on Windows. No WSL. No Hyper-V. No admin rights. One `.exe`.**

![logo](assets/branding/winmux-v2.png)

[![Latest release](https://img.shields.io/github/v/release/Denromvas/winmux?label=download&style=for-the-badge)](https://github.com/Denromvas/winmux/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg?style=for-the-badge)](LICENSE)

---

## Why WinMux

You want to use Claude Code (or any Linux-native AI agent / dev tool) but you're on Windows. Your options today:

- **WSL** — needs admin, Windows feature toggles, reboots, often blocked on corporate machines.
- **A Mac** — buy a $1500 laptop just to run a CLI.
- **A cloud VM** — monthly bill, latency, your files aren't local.

**WinMux is a fourth option.** Download one portable `.exe`, double-click it, and ~10 seconds later you have a real Ubuntu shell with Claude Code pre-installed — running entirely on your Windows machine, with your Windows folders auto-mounted. No admin prompt. Nothing to configure.

It works the same on a developer's Win11 laptop and on a locked-down Windows Server you reach over RDP.

## Download

**→ [Get the latest installer](https://github.com/Denromvas/winmux/releases/latest)**

| Edition | Best for | Download |
|---|---|---|
| **Desktop** | Developers, AI agents, drag-and-drop | `winmux-desktop-setup-vX.Y.Z.exe` |
| **CLI** | DevOps, Windows Server, scripting | `winmux-cli-setup-vX.Y.Z.exe` |

Installs per-user into `%LOCALAPPDATA%\WinMux\` — **no administrator rights required**.

> ⚠️ The installer is currently unsigned (code signing in progress), so Windows SmartScreen may warn on first run. Click **More info → Run anyway**.

## What you get

- **One portable `.exe`** (~510 MB — includes QEMU + a full Ubuntu 24.04 image).
- **Real Ubuntu shell** in ~10 seconds (custom Rust init, no systemd boot wait).
- **Claude Code pre-installed** — plus `node` 22, `npm`, `git`, `tmux`, `python3`.
- **Your Windows files auto-mounted** at `/workspace` (your user folder, over SSH, zero-config).
- **Shared localhost** — any port you open in Linux (`3000`, `8080`, …) is instantly reachable at `127.0.0.1:N` on Windows.
- **Modern terminal UI** (Tauri 2 + xterm.js): tabs, splits, 6 themes, command palette (`Ctrl+Shift+P`), scrollback search (`Ctrl+F`).
- **Built for AI agents:**
  - 📋 Paste a screenshot straight into Claude with `Ctrl+Shift+V` (even inside its TUI).
  - 🖱 Drag a file from Explorer → its path becomes a guest path Claude can read.
  - 🤖 Live AI activity sidebar (streams Claude's tool calls).
  - 📸 VM snapshots — `savevm` before `claude --dangerously-skip-permissions`, roll back instantly if it breaks something.
  - 🌐 Mini-browser tabs for your dev servers — click a forwarded port, it opens inside WinMux.
- **Run services that survive reboots** — drop a script in `~/.winmux/services/` and it auto-starts on every boot (Telegram bots, schedulers, workers). No systemd needed.

## Quick start

1. Download and run `winmux-desktop-setup-vX.Y.Z.exe`.
2. Launch **WinMux** — the VM starts automatically.
3. A bash prompt appears. Your Windows user folder is at `/workspace`.

```bash
# inside the Linux shell
cd /workspace            # your Windows files are here
export ANTHROPIC_API_KEY="sk-ant-..."   # or use a Claude subscription
claude                   # start coding with AI

# expose a dev server — reachable at http://127.0.0.1:3000 on Windows
cd /workspace/my-app && npm run dev
```

> Tip: right-click any folder in Windows Explorer → **Open in WinMux** to jump straight in. Add a `.winmux/config.toml` with an `init_command` for zero-touch project startup.

## Performance

WinMux auto-detects the fastest available accelerator:

- **WHPX** (Windows Hypervisor Platform) — near-native speed. Used automatically when available.
- **TCG** (software emulation) — universal fallback, slower but works everywhere.

For best speed, ensure WHPX can run: disable the **full Hyper-V role** and **Memory Integrity / Core Isolation** (both reserve the hypervisor and force WinMux onto TCG). WSL2 and Docker Desktop keep working via HypervisorPlatform.

## Architecture

```
┌────────────────────────────────────────────────────────┐
│                     WINDOWS HOST                        │
│  ┌──────────────────┐    ┌──────────────────────┐       │
│  │  winmux-desktop  │◄──►│  winmux-controller   │       │
│  │  (Tauri+xterm.js)│IPC │  (lifecycle, QMP)    │       │
│  └──────────────────┘    └──────────┬───────────┘       │
│                                      │ spawns            │
│                            ┌─────────▼─────────┐         │
│                            │ qemu-system-x86_64 │        │
│                            └─────────┬─────────┘         │
└──────────────────────────────────────┼─────────────────┘
                                        │ virtio-{net,serial}
                          ┌─────────────▼─────────────┐
                          │   Ubuntu 24.04 Minimal    │
                          │   winmux-init (PID 1)     │
                          │   ↓ sshd + agent + bash   │
                          │   ↓ user services         │
                          └───────────────────────────┘
```

## Tech stack

| Component | Tech |
|---|---|
| Hypervisor | QEMU 11.0 (user-mode, no admin) |
| Guest OS | Ubuntu 24.04 Minimal |
| Init | Custom `winmux-init` in Rust (replaces systemd, ~300 ms boot) |
| Network | SLIRP NAT + agent-driven QMP `hostfwd_add` (auto port forwarding) |
| Shared FS | sshfs over the guest's SSH to Windows OpenSSH server |
| Desktop UI | Tauri 2 + React 18 + xterm.js |
| Controller | Rust (sync, `std::net` + `std::thread`) |
| Cross-compile | mingw-w64 (Windows), musl (Linux guest) |
| Installer | NSIS (per-user, auto-installs WebView2 runtime if missing) |

## Build from source

```bash
# Dependencies (Linux build host):
sudo apt install -y mingw-w64 musl-tools nsis nodejs npm

# Rust + targets:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
rustup target add x86_64-pc-windows-gnu x86_64-unknown-linux-musl
npm install -g @tauri-apps/cli@^2

# Build both editions:
cd code/winmux-desktop/ui && npm install && cd -
VERSION=0.1.11 bash scripts/build-desktop.sh   # → dist/winmux-desktop-setup-v0.1.11.exe
VERSION=0.1.11 bash scripts/build-cli.sh       # → dist/winmux-cli-setup-v0.1.11.exe
```

## Roadmap

**Shipped (v0.1.x alpha):**
- ✅ Core engine, zero-config auto-mount, passwordless SSH
- ✅ Desktop UI: tabs, splits, themes, command palette, mini-browser
- ✅ Snapshots, scrollback search, clipboard-image & file drop for AI
- ✅ AI activity sidebar, per-project config, user-service autostart
- ✅ WHPX/TCG auto-detect, built-in updater, opt-out telemetry

**Toward v1.0 (public):**
- 🔲 Code signing (SignPath.io for OSS) — remove SmartScreen warning
- 🔲 Smaller image (compress rootfs)
- 🔲 macOS / Linux host editions (same guest, native controller)
- 🔲 Public beta program

## License

MIT — see [LICENSE](LICENSE). Bundled QEMU is under GPLv2 (see `qemu/COPYING`).

## Author

Denis Romanyuk · claudetaistra@gmail.com

---

*WinMux exists so anyone can use Linux-native AI tooling on the computer they already own — no second machine, no cloud bill, no admin fight.*
