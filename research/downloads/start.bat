@echo off
REM ============================================================
REM WinMux PoC — first boot script
REM Запускає Ubuntu 24.04 у QEMU user-mode без прав адміна
REM ============================================================

setlocal enabledelayedexpansion
cd /d "%~dp0"

echo.
echo ===== WinMux PoC v0.1 =====
echo Working dir: %CD%
echo.

REM 1. Створюємо overlay-диск, якщо його ще немає
if not exist "user.qcow2" (
    echo [1] Creating overlay disk user.qcow2...
    qemu\qemu-img.exe create -f qcow2 -b "%CD%\ubuntu-24.04.img" -F qcow2 user.qcow2 20G
    if errorlevel 1 (
        echo FAIL: cannot create overlay disk
        pause
        exit /b 1
    )
)

REM 2. Параметри запуску
set ACCEL=whpx,kernel-irqchip=off
set RAM=2G
set CPUS=4

echo [2] Boot params: accel=%ACCEL%, ram=%RAM%, cpus=%CPUS%
echo.
echo === Starting QEMU... (Ctrl-A X to quit serial console) ===
echo.

REM 3. Запуск QEMU
REM   - accel WHPX (fallback на TCG якщо не підтримується)
REM   - serial → stdio (видно у вікні cmd)
REM   - nographic — без графічного вікна
REM   - host port 2222 → guest 22 (для майбутнього SSH)
REM   - host port 8080 → guest 8080 (для тестів веб-сервера)
REM   - 9p shared dir: %CD%\share → /workspace в госту
REM   - cloud-init seed.iso

qemu\qemu-system-x86_64.exe ^
  -accel %ACCEL% ^
  -accel tcg ^
  -m %RAM% ^
  -smp %CPUS% ^
  -cpu max ^
  -drive file=user.qcow2,if=virtio,format=qcow2 ^
  -drive file=seed.iso,if=virtio,format=raw,readonly=on ^
  -netdev user,id=n0,hostfwd=tcp::2222-:22,hostfwd=tcp::8080-:8080 ^
  -device virtio-net-pci,netdev=n0 ^
  -fsdev local,id=fs0,path=share,security_model=mapped-xattr ^
  -device virtio-9p-pci,fsdev=fs0,mount_tag=workspace ^
  -nographic ^
  -serial mon:stdio

echo.
echo === QEMU exited ===
endlocal
