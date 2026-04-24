# Changelog

## [0.1.0] — 2026-04-24 (alpha)

First public alpha release.

### Added
- Two editions: WinMux **CLI** (`winmux.exe` only) and WinMux **Desktop** (Tauri+xterm.js GUI).
- `winmux-controller` (Rust, Windows): manages QEMU lifecycle, QMP client, port-watcher bridge.
- `winmux-init` (Rust, Linux PID 1): replaces systemd; mounts /proc/sys/dev, brings up network with DHCP→static fallback, sets persistent hostname & DNS, starts sshd + agent.
- `winmux-agent` (Rust, in-guest): polls /proc/net/tcp, reports new LISTEN ports to controller via TCP through SLIRP NAT.
- Auto port forwarding: any port opened in guest → instantly available on Windows `127.0.0.1:NNNN`.
- WHPX support with TCG fallback (auto-detected, configurable).
- Custom `winmux-init` boots in ~300 ms; full SSH-ready in ~2 s on TCG, target <1 s with WHPX.
- Frozen rootfs v5: Ubuntu 24.04 Minimal + Node.js 22 + Claude Code 2.1 + git/tmux/sshfs/sshpass + ping/curl.
- `winmux-mount` interactive helper for sshfs Windows-folder mounting.
- Tauri Desktop UI: PTY terminal (portable-pty), 6 themes (WinMux Dark / Dracula / Tokyo Night / Solarized / Catppuccin / GitHub Dark), font size controls, drag-and-drop files+URLs+clipboard images, sidebar with status / ports / logs / Recovery, custom titlebar.
- WebGL/Canvas renderer for crisp text without artifacts.
- Drag-and-drop: files from Explorer, URLs from browser, images from clipboard (saved to `drops/`).
- Bulletproof cleanup: auto-kill zombie processes on start, Stop / Force kill / Reset session buttons.
- NSIS installers (per-user, no admin) + portable ZIPs for both editions.
- Windows Explorer context menu: "Open in WinMux" on folders / drives / background.

### Known limitations
- Anthropic Claude Code requires API key for use from Ukraine (geographic restriction on subscription/login flow). API keys (`sk-ant-...`) work everywhere.
- Multi-tab / splits: planned, not yet implemented.
- Auto-mount Windows folders via SSH key: planned, currently manual via `winmux-mount`.
- Auto-update: planned via Tauri updater.
- Telemetry server: planned, currently no telemetry collected.
- Code signing: not yet — Windows SmartScreen will warn on first launch (click "More info → Run anyway"). SignPath.io OSS signing planned for v1.0.

### Tech facts
- Bundle size: ~510 MB installer (QEMU x86_64-only 156 MB + Linux rootfs 493 MB + Tauri exe 4.4 MB + WebView2Loader 156 KB + controller 657 KB).
- Memory at idle: ~500 MB (QEMU 2 GB allocated, ~970 MB used during work).
- Source: Rust (controller/agent/init/Tauri backend) + TypeScript/React (Tauri UI).
