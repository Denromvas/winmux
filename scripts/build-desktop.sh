#!/bin/bash
# Build WinMux Desktop edition: portable ZIP + NSIS installer
set -euo pipefail

VERSION="${VERSION:-0.1.0}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILD="$ROOT/build/desktop"
DIST="$ROOT/dist"
QEMU_SRC="$ROOT/research/downloads/qemu"
ROOTFS_SRC="$ROOT/research/downloads"

mkdir -p "$BUILD" "$DIST"
rm -rf "$BUILD"/*

echo "==> [1/7] Building Rust binaries..."
cd "$ROOT/code"
source "$HOME/.cargo/env"

cargo build --release --target x86_64-unknown-linux-musl -p winmux-init -p winmux-agent 2>&1 | tail -3
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
    cargo build --release --target x86_64-pc-windows-gnu -p winmux-controller 2>&1 | tail -3

echo "==> [2/7] Building Tauri UI..."
cd "$ROOT/code/winmux-desktop/ui"
npm run build 2>&1 | tail -3

cd "$ROOT/code"
echo "==> [3/7] Building winmux-desktop.exe..."
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
    cargo build --release --target x86_64-pc-windows-gnu -p winmux-desktop 2>&1 | tail -3

echo "==> [4/7] Slimming QEMU..."
mkdir -p "$BUILD/qemu/share"
cp "$QEMU_SRC/qemu-system-x86_64.exe" "$BUILD/qemu/"
cp "$QEMU_SRC/qemu-system-x86_64w.exe" "$BUILD/qemu/" 2>/dev/null || true
cp "$QEMU_SRC/qemu-img.exe" "$BUILD/qemu/"
cp "$QEMU_SRC"/*.dll "$BUILD/qemu/" 2>/dev/null || true
SHARE_FILES=(
    "bios-256k.bin" "bios.bin" "bios-microvm.bin"
    "vgabios-stdvga.bin" "vgabios-virtio.bin" "vgabios-vmware.bin"
    "vgabios-cirrus.bin" "vgabios-bochs-display.bin" "vgabios-ramfb.bin"
    "vgabios-qxl.bin" "vgabios-ati.bin"
    "edk2-x86_64-code.fd" "edk2-i386-vars.fd" "edk2-i386-code.fd"
    "kvmvapic.bin" "linuxboot.bin" "linuxboot_dma.bin" "multiboot.bin" "multiboot_dma.bin"
    "pvh.bin" "pxe-virtio.rom" "pxe-e1000.rom" "pxe-rtl8139.rom" "pxe-pcnet.rom" "pxe-ne2k_pci.rom"
    "efi-virtio.rom" "efi-e1000.rom" "efi-rtl8139.rom" "efi-pcnet.rom" "efi-ne2k_pci.rom"
    "qboot.rom" "sgabios.bin" "vgabios.bin" "kvm-spdm.bin"
)
for f in "${SHARE_FILES[@]}"; do
    [[ -f "$QEMU_SRC/share/$f" ]] && cp "$QEMU_SRC/share/$f" "$BUILD/qemu/share/"
done
cp "$QEMU_SRC/COPYING" "$BUILD/qemu/" 2>/dev/null || true
cp "$QEMU_SRC/COPYING.LIB" "$BUILD/qemu/" 2>/dev/null || true

echo "==> [5/7] Copying rootfs..."
mkdir -p "$BUILD/rootfs"
# v3 = з вшитими Node 22 + Claude Code + git + tmux + sshfs + sshpass
# v9 = v8 + winmux-agent з virtio-serial terminal mux (low-latency термінал)
ROOTFS_FILE="$ROOTFS_SRC/frozen-v9.qcow2"
[[ -f "$ROOTFS_FILE" ]] || ROOTFS_FILE="$ROOTFS_SRC/frozen-v8.qcow2"
[[ -f "$ROOTFS_FILE" ]] || ROOTFS_FILE="$ROOTFS_SRC/frozen-v7.qcow2"
[[ -f "$ROOTFS_FILE" ]] || ROOTFS_FILE="$ROOTFS_SRC/frozen-v6.qcow2"
[[ -f "$ROOTFS_FILE" ]] || ROOTFS_FILE="$ROOTFS_SRC/frozen.qcow2"
echo "  using $(basename "$ROOTFS_FILE") ($(du -h "$ROOTFS_FILE" | cut -f1))"
cp "$ROOTFS_FILE" "$BUILD/rootfs/base.qcow2"
[[ -f /tmp/winmux-vmlinuz ]] && cp /tmp/winmux-vmlinuz "$BUILD/rootfs/vmlinuz"

echo "==> [6/7] Bundling controller, desktop, configs..."
cp "$ROOT/code/target/x86_64-pc-windows-gnu/release/winmux-controller.exe" "$BUILD/winmux.exe"
cp "$ROOT/code/target/x86_64-pc-windows-gnu/release/winmux-desktop.exe" "$BUILD/winmux-desktop.exe"

# WebView2Loader.dll — обов'язково поряд з Tauri exe
WEBVIEW_DLL="$ROOT/research/downloads/webview2/WebView2Loader.dll"
if [[ -f "$WEBVIEW_DLL" ]]; then
    cp "$WEBVIEW_DLL" "$BUILD/WebView2Loader.dll"
    echo "  WebView2Loader.dll bundled"
else
    echo "  WARN: $WEBVIEW_DLL not found — Desktop will fail with 'WebView2Loader.dll not found'"
fi

cat > "$BUILD/winmux.toml" <<'EOF'
qemu_binary = "qemu/qemu-system-x86_64.exe"
disk = "rootfs/user.qcow2"
kernel = "rootfs/vmlinuz"
kernel_append = "root=/dev/vda1 rw init=/sbin/winmux-init console=ttyS0 panic=10 processor.max_cstate=1 intel_idle.max_cstate=0"
ram = "4G"
smp = 8
accel = "auto"
qmp_port = 4444
agent_port = 4445
ssh_port = 2223
serial_log = "logs/boot.log"
hidden = true
EOF

cat > "$BUILD/README.txt" <<EOF
WinMux Desktop v$VERSION
========================

Two ways to start:

GUI:
  Double-click winmux-desktop.exe

CLI:
  PowerShell: .\winmux.exe start

Default user inside Linux: winmux / winmux  (sudo NOPASSWD)
Files:
- winmux-desktop.exe — GUI (Tauri + xterm.js)
- winmux.exe         — controller (CLI mode, also embedded in GUI)
- winmux.toml        — config (RAM/CPU/ports)
- qemu\              — QEMU x86_64 only
- rootfs\            — Ubuntu base + kernel
EOF

mkdir -p "$BUILD/logs"

echo "==> [7/7] Creating ZIP and NSIS installer..."
cd "$BUILD"
ZIP_NAME="winmux-desktop-portable-v${VERSION}.zip"
rm -f "$DIST/$ZIP_NAME"
(cd "$BUILD" && zip -r9 "$DIST/$ZIP_NAME" . -x "logs/*" >/dev/null)
ZIP_SIZE=$(du -sh "$DIST/$ZIP_NAME" | cut -f1)
echo "  $DIST/$ZIP_NAME ($ZIP_SIZE)"

if command -v makensis >/dev/null 2>&1; then
    NSIS_SCRIPT="$ROOT/scripts/winmux-desktop.nsi"
    if [[ -f "$NSIS_SCRIPT" ]]; then
        makensis -DVERSION="$VERSION" -DBUILD_DIR="$BUILD" -DDIST_DIR="$DIST" "$NSIS_SCRIPT" 2>&1 | tail -5
    fi
fi

# Опційний code signing (якщо є cert)
# Use: WINMUX_CODESIGN=1 WINMUX_CERT=/path/cert.pfx WINMUX_CERT_PASS=... bash build-desktop.sh
if [[ "${WINMUX_CODESIGN:-0}" == "1" ]] && command -v osslsigncode >/dev/null 2>&1; then
    if [[ -n "${WINMUX_CERT:-}" ]] && [[ -f "$WINMUX_CERT" ]]; then
        echo "==> Signing executables with $WINMUX_CERT..."
        for exe in "$BUILD/winmux.exe" "$BUILD/winmux-desktop.exe" "$DIST/winmux-desktop-setup-v${VERSION}.exe"; do
            if [[ -f "$exe" ]]; then
                osslsigncode sign \
                    -pkcs12 "$WINMUX_CERT" \
                    -pass "${WINMUX_CERT_PASS:-}" \
                    -t http://timestamp.sectigo.com \
                    -in "$exe" -out "${exe}.signed" && mv "${exe}.signed" "$exe" && echo "  signed $exe"
            fi
        done
    fi
fi

echo
echo "===================================="
echo "Build complete!"
ls -lh "$DIST/" | tail -10
