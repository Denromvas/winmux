param(
    [string]$Accel = "tcg"
)
$ErrorActionPreference = "Stop"
Set-Location E:\winmux-poc

if (-not (Test-Path user.qcow2)) {
    & qemu\qemu-img.exe create -f qcow2 -b E:\winmux-poc\ubuntu-24.04.img -F qcow2 user.qcow2 20G | Out-Null
}

$qemuArgs = @(
    "-accel", $Accel,
    "-m", "2G",
    "-smp", "4",
    "-drive", "file=user.qcow2,if=virtio,format=qcow2",
    "-drive", "file=seed.iso,if=virtio,format=raw,readonly=on",
    "-netdev", "user,id=n0,hostfwd=tcp::2222-:22,hostfwd=tcp::8080-:8080",
    "-device", "virtio-net-pci,netdev=n0",
    "-display", "none",
    "-serial", "file:boot.log",
    "-monitor", "none",
    "-pidfile", "qemu.pid"
)

$start = Get-Date
"[$start] Launching QEMU detached, accel=$Accel..." | Tee-Object -FilePath launch.log

$proc = Start-Process -FilePath ".\qemu\qemu-system-x86_64.exe" `
    -ArgumentList $qemuArgs `
    -WindowStyle Hidden `
    -PassThru

$proc.Id | Out-File -FilePath qemu.pid.ps -Encoding ASCII

"PID: $($proc.Id)" | Tee-Object -FilePath launch.log -Append
"Done. Process detached. Check boot.log for output."
