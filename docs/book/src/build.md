# Збірка з джерел

## Залежності (Linux build host)

```bash
sudo apt install -y \
    mingw-w64 musl-tools nsis nodejs npm \
    p7zip-full xorriso zip imagemagick python3-pip
pip3 install --user --break-system-packages pillow
```

## Rust + cross-compile targets

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
source ~/.cargo/env
rustup target add x86_64-pc-windows-gnu x86_64-unknown-linux-musl
```

## Tauri CLI

```bash
npm install -g @tauri-apps/cli@^2
```

## Збірка

```bash
cd /шлях/до/winmux

# UI build один раз
cd code/winmux-desktop/ui && npm install && cd -

# Повний build обох editions
bash scripts/build-cli.sh
bash scripts/build-desktop.sh

# → dist/winmux-cli-setup-vX.Y.Z.exe
# → dist/winmux-cli-portable-vX.Y.Z.zip
# → dist/winmux-desktop-setup-vX.Y.Z.exe
# → dist/winmux-desktop-portable-vX.Y.Z.zip
```

## Code signing (опційно)

```bash
sudo apt install -y osslsigncode

WINMUX_CODESIGN=1 \
  WINMUX_CERT=/path/to/your-cert.pfx \
  WINMUX_CERT_PASS='password' \
  bash scripts/build-desktop.sh
```

## Перебудова frozen image

Якщо треба оновити preinstalled пакети у Linux guest:

```bash
# 1. Запусти QEMU з frozen-vN базою + новим overlay
ssh dromanyuk@your-test-server '...QEMU command...'

# 2. SSH у guest, встанови що треба
ssh -p 2223 winmux@host 'sudo apt install ...'

# 3. Shutdown і compact
ssh -p 2223 winmux@host 'sudo poweroff'
qemu-img convert -O qcow2 -c overlay.qcow2 frozen-vN+1.qcow2

# 4. Постав у research/downloads/, build script підхопить найсвіжіший
```

## Структура коду (Cargo workspace)

```
code/
├── Cargo.toml               # workspace
├── winmux-shared/           # JSON-RPC types між controller і agent
├── winmux-controller/       # Windows daemon
├── winmux-agent/            # Linux guest daemon
├── winmux-init/             # Linux PID 1
└── winmux-desktop/
    ├── src-tauri/           # Tauri Rust backend
    └── ui/                  # React+xterm.js frontend
```

## Build artifacts

- `code/target/x86_64-pc-windows-gnu/release/winmux-controller.exe`
- `code/target/x86_64-pc-windows-gnu/release/winmux-desktop.exe`
- `code/target/x86_64-unknown-linux-musl/release/winmux-init`
- `code/target/x86_64-unknown-linux-musl/release/winmux-agent`
- `code/winmux-desktop/ui/dist/` — built UI assets
