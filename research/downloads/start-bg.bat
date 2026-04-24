@echo off
REM Background start — stdout/stderr to files. No interactive console.
cd /d "%~dp0"

if not exist user.qcow2 (
    qemu\qemu-img.exe create -f qcow2 -b "%CD%\ubuntu-24.04.img" -F qcow2 user.qcow2 20G
)

REM Параметр %1: accelerator (whpx або tcg)
set ACCEL=%1
if "%ACCEL%"=="" set ACCEL=whpx,kernel-irqchip=off

echo [%date% %time%] Starting QEMU with accel=%ACCEL%

REM NB: 9p (fsdev) НЕ підтримується в офіційному QEMU для Windows.
REM Для shared FS треба кастомна збірка або SMB.

qemu\qemu-system-x86_64.exe ^
  -accel %ACCEL% ^
  -m 2G ^
  -smp 4 ^
  -drive file=user.qcow2,if=virtio,format=qcow2 ^
  -drive file=seed.iso,if=virtio,format=raw,readonly=on ^
  -netdev user,id=n0,hostfwd=tcp::2222-:22,hostfwd=tcp::8080-:8080 ^
  -device virtio-net-pci,netdev=n0 ^
  -display none ^
  -serial file:boot.log ^
  -monitor none
