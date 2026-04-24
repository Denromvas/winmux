# Contributing to WinMux

Дякую за інтерес! 🎉

## Швидкий старт для розробників

### Залежності (Linux build host)

```bash
sudo apt install -y mingw-w64 musl-tools nsis nodejs npm \
                    p7zip-full xorriso zip imagemagick python3-pip
pip3 install --user --break-system-packages pillow

# Rust + targets
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
source ~/.cargo/env
rustup target add x86_64-pc-windows-gnu x86_64-unknown-linux-musl

# Tauri CLI
npm install -g @tauri-apps/cli@^2
```

### Build

```bash
# UI один раз
cd code/winmux-desktop/ui && npm install && cd -

# Повний build обох editions
bash scripts/build-cli.sh
bash scripts/build-desktop.sh
# → dist/winmux-cli-setup-vX.Y.Z.exe
# → dist/winmux-desktop-setup-vX.Y.Z.exe
```

### Структура

```
code/
├── winmux-shared/       # JSON-RPC types
├── winmux-controller/   # Windows daemon (Rust)
├── winmux-init/         # Linux PID 1 (Rust musl)
├── winmux-agent/        # Linux port watcher (Rust musl)
└── winmux-desktop/
    ├── src-tauri/       # Tauri Rust backend
    └── ui/              # React+xterm.js frontend
```

### Стиль коду

- **Rust**: `cargo fmt` + `cargo clippy`. Уникай `unwrap()` у production paths — використовуй `Result<>` або `expect("причина")`.
- **TypeScript**: 2-space indentation. Немає примусового linter — здоровий глузд.
- Коментарі — по необхідності, переважно для **why** (не **what**).

## Тестування

Перед PR перевір:
1. `bash scripts/build-desktop.sh` проходить локально
2. `winmux-desktop-setup-v*.exe` встановлюється на чистій Windows VM (можна VirtualBox)
3. `winmux start` піднімає QEMU без admin
4. `winmux stop` коректно вбиває процеси

## Pull Requests

1. Fork → branch з описовою назвою (`feature/multi-tab`, `fix/dns-resolv-conf`)
2. Малі коміти з осмисленими повідомленнями
3. Опиши **навіщо** ці зміни в PR description
4. Підв'яжи issue якщо є (`Fixes #N`)

## Reporting bugs

Використовуй [issue templates](.github/ISSUE_TEMPLATE/). Додавай:
- Версію WinMux (sidebar / `winmux.exe version`)
- Windows version (`winver`)
- Останні рядки `%LOCALAPPDATA%\WinMux\logs\boot.log`
- Чи bare metal / VM

## License

Внески → MIT (як весь проект).
