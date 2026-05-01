#!/bin/bash
# Build WinMux CLI edition: portable ZIP + NSIS installer
set -euo pipefail

VERSION="${VERSION:-0.1.0}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILD="$ROOT/build/cli"
DIST="$ROOT/dist"
QEMU_SRC="$ROOT/research/downloads/qemu"
ROOTFS_SRC="$ROOT/research/downloads"  # frozen.qcow2 + vmlinuz

mkdir -p "$BUILD" "$DIST"
rm -rf "$BUILD"/*

echo "==> [1/6] Building Rust binaries..."
cd "$ROOT/code"
source "$HOME/.cargo/env"

# Linux musl static — для гостя
cargo build --release --target x86_64-unknown-linux-musl -p winmux-init -p winmux-agent 2>&1 | tail -3

# Windows GNU — для контролера
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
    cargo build --release --target x86_64-pc-windows-gnu -p winmux-controller 2>&1 | tail -3

echo "==> [2/6] Slimming QEMU (x86_64-only)..."
mkdir -p "$BUILD/qemu/share"

# Тільки потрібні exe
cp "$QEMU_SRC/qemu-system-x86_64.exe" "$BUILD/qemu/"
cp "$QEMU_SRC/qemu-system-x86_64w.exe" "$BUILD/qemu/" 2>/dev/null || true
cp "$QEMU_SRC/qemu-img.exe" "$BUILD/qemu/"

# Усі DLL (вони shared, треба всі — інакше qemu не запуститься)
cp "$QEMU_SRC"/*.dll "$BUILD/qemu/" 2>/dev/null || true

# share/ — потрібні файли для x86_64
SHARE_FILES=(
    "bios-256k.bin" "bios.bin" "bios-microvm.bin"
    "vgabios-stdvga.bin" "vgabios-virtio.bin" "vgabios-vmware.bin"
    "vgabios-cirrus.bin" "vgabios-bochs-display.bin" "vgabios-ramfb.bin"
    "vgabios-qxl.bin" "vgabios-ati.bin"
    "edk2-x86_64-code.fd" "edk2-i386-vars.fd" "edk2-i386-code.fd"
    "kvmvapic.bin" "linuxboot.bin" "linuxboot_dma.bin" "multiboot.bin" "multiboot_dma.bin"
    "pvh.bin" "pxe-virtio.rom" "pxe-e1000.rom" "pxe-rtl8139.rom" "pxe-pcnet.rom" "pxe-ne2k_pci.rom"
    "efi-virtio.rom" "efi-e1000.rom" "efi-rtl8139.rom" "efi-pcnet.rom" "efi-ne2k_pci.rom"
    "qboot.rom" "sgabios.bin" "vgabios.bin"
    "kvm-spdm.bin"
)
for f in "${SHARE_FILES[@]}"; do
    if [[ -f "$QEMU_SRC/share/$f" ]]; then
        cp "$QEMU_SRC/share/$f" "$BUILD/qemu/share/"
    fi
done

# COPYING для GPL compliance
cp "$QEMU_SRC/COPYING" "$BUILD/qemu/" 2>/dev/null || true
cp "$QEMU_SRC/COPYING.LIB" "$BUILD/qemu/" 2>/dev/null || true

QEMU_SIZE=$(du -sh "$BUILD/qemu" | cut -f1)
echo "  QEMU bundle: $QEMU_SIZE"

echo "==> [3/6] Copying rootfs and kernel..."
mkdir -p "$BUILD/rootfs"
ROOTFS_FILE="$ROOTFS_SRC/frozen-v7.qcow2"
[[ -f "$ROOTFS_FILE" ]] || ROOTFS_FILE="$ROOTFS_SRC/frozen-v6.qcow2"
[[ -f "$ROOTFS_FILE" ]] || ROOTFS_FILE="$ROOTFS_SRC/frozen.qcow2"
echo "  using $(basename "$ROOTFS_FILE") ($(du -h "$ROOTFS_FILE" | cut -f1))"
cp "$ROOTFS_FILE" "$BUILD/rootfs/base.qcow2"
# vmlinuz — лежить на сервері, скопіюємо сюди якщо є
if [[ -f /tmp/winmux-vmlinuz ]]; then
    cp /tmp/winmux-vmlinuz "$BUILD/rootfs/vmlinuz"
fi
ROOTFS_SIZE=$(du -sh "$BUILD/rootfs" | cut -f1)
echo "  Rootfs: $ROOTFS_SIZE"

echo "==> [4/6] Copying controller and config template..."
cp "$ROOT/code/target/x86_64-pc-windows-gnu/release/winmux-controller.exe" "$BUILD/winmux.exe"

cat > "$BUILD/winmux.toml" <<'EOF'
# WinMux configuration
# This file is generated on first run if missing.

# Где лежат бінарники QEMU (відносно цього файлу)
qemu_binary = "qemu/qemu-system-x86_64.exe"

# Образ rootfs (read-only base + overlay буде створено автоматично як user.qcow2)
disk = "rootfs/user.qcow2"

# Direct kernel boot — швидко, без GRUB
kernel = "rootfs/vmlinuz"
kernel_append = "root=/dev/vda1 rw init=/sbin/winmux-init console=ttyS0 panic=10"

# Ресурси VM
ram = "4G"
smp = 8

# Acceleration: "auto" = WHPX якщо доступний, fallback TCG
accel = "auto"

# Ports
qmp_port = 4444
agent_port = 4445
ssh_port = 2223

# Logs
serial_log = "logs/boot.log"
hidden = true
EOF

# Wrapper PowerShell скрипт для зручного запуску
cat > "$BUILD/winmux.ps1" <<'EOF'
# WinMux launcher (PowerShell wrapper)
param([Parameter(ValueFromRemainingArguments=$true)] $Args)
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
& "$here\winmux.exe" @Args
EOF

# README-quickstart
cat > "$BUILD/README.txt" <<EOF
WinMux CLI v$VERSION
====================

Quick start:
1. Open PowerShell or cmd in this folder.
2. Run: .\winmux.exe start
3. In another window: ssh -p 2222 winmux@127.0.0.1   (password: winmux)

Files:
- winmux.exe       — main controller
- winmux.toml      — config (edit RAM/CPU/ports here)
- qemu\            — QEMU bundle (x86_64 only)
- rootfs\          — Ubuntu 24.04 image + kernel
- logs\            — boot logs (created on first run)

Commands:
- winmux start             — boot the VM, run controller
- winmux init-config       — write a fresh winmux.toml
- winmux version

Default user: winmux / winmux  (sudo NOPASSWD)
EOF

mkdir -p "$BUILD/logs"

echo "==> [5/6] Creating portable ZIP..."
cd "$BUILD"
ZIP_NAME="winmux-cli-portable-v${VERSION}.zip"
rm -f "$DIST/$ZIP_NAME"
(cd "$BUILD" && zip -r9 "$DIST/$ZIP_NAME" . -x "logs/*" >/dev/null)
ZIP_SIZE=$(du -sh "$DIST/$ZIP_NAME" | cut -f1)
echo "  $DIST/$ZIP_NAME ($ZIP_SIZE)"

echo "==> [6/6] Building NSIS installer..."
if command -v makensis >/dev/null 2>&1; then
    NSIS_SCRIPT="$ROOT/scripts/winmux-cli.nsi"
    if [[ -f "$NSIS_SCRIPT" ]]; then
        makensis -DVERSION="$VERSION" -DBUILD_DIR="$BUILD" -DDIST_DIR="$DIST" "$NSIS_SCRIPT" 2>&1 | tail -5
    else
        echo "  (skipping — $NSIS_SCRIPT not found)"
    fi
else
    echo "  (skipping — makensis not installed; run 'sudo apt install nsis')"
fi

echo
echo "===================================="
echo "Build complete!"
ls -lh "$DIST/" | tail -10
