# Roadmap

## v0.1.0 (alpha) — DONE ✅

- [x] Core engine: controller + init + agent на Rust
- [x] Auto port forwarding через QMP
- [x] Custom init без systemd (boot ~2 сек)
- [x] PTY терминал у Tauri вікні через portable-pty
- [x] Drag-and-drop файлів/URL/clipboard images
- [x] 6 готових тем + font controls
- [x] System tray
- [x] Windows Explorer "Open in WinMux" context menu
- [x] Settings UI (RAM/CPU/accel/ports)
- [x] NSIS installer (per-user, no admin) + portable ZIP
- [x] Frozen image v5: Ubuntu + Node 22 + Claude Code + tools
- [x] `winmux-mount` helper для sshfs Windows-папок
- [x] Bulletproof cleanup: auto-kill зомбі, Force kill, Reset session
- [x] WebGL renderer для xterm.js (крейдяна графіка)
- [x] MIT license, README, CHANGELOG, .github templates
- [x] Documentation site (mdBook)

## v0.2.0 (beta) — IN PROGRESS

- [ ] **Auto-mount Windows folders via SSH key** (zero-config) — головна
- [ ] **Multi-tab + splits** у Tauri (як Wezterm)
- [ ] **Auto-update** через Tauri updater (signed releases)
- [ ] **Telemetry server** (opt-out, self-hosted)
- [ ] AI status panel у sidebar (live thoughts of running claude)
- [ ] Command palette (Ctrl+Shift+P)
- [ ] OSC 8 hyperlinks (clickable URLs з кольорами)
- [ ] WHPX auto-detect з graceful fallback на TCG

## v1.0 (public)

- [ ] **Code signing** (SignPath.io for OSS — безкоштовно)
- [ ] Public landing page (winmux.app або поддомен)
- [ ] Бета-програма: 5-10 тестерів, GitHub Issues
- [ ] Українська та англійська локалізація UI
- [ ] Telemetry public dashboard

## v2.0+ (відкладено)

- GUI Linux програми (через X11/Wayland проксі)
- "Server edition" — multi-user shared install
- Linux/macOS-сторона (запустити WinMux-guest на іншій ОС)
- Vendored snapshots: швидкі pre-installed dev стеки (postgres, redis, k8s tools)
- Marketplace середовищ
