# Усунення проблем

## Status застряг у "Starting..."

1. Подивись Controller log у sidebar — що там
2. Якщо `QEMU process died` — VM крашиться (часто WHPX в nested virt)
3. Натисни **Force kill all** → змінь `accel = "tcg"` у Settings → Start

## "Could not set up host forwarding rule"

Порт зайнятий іншою програмою (часто svchost тримає 2222).
Settings → змінь SSH port на 2223 / 2224.

## SIGILL "Illegal instruction"

Бінарник Node V8 (наприклад claude) очікує SSE4.2/AVX, а TCG базовий не дає.
Поточна версія WinMux вже передає `-cpu max` у QEMU при TCG — має працювати. Якщо не — оновись до останньої.

## "Failed to connect to api.anthropic.com"

Це **геоблок Anthropic** на українські IP (тільки для Claude Code login flow). Рішення: API key замість login, або VPN.

## Bash команда не знайдена (ping/curl)

В minimal cloud image їх не було. У frozen-v5+ вже є. Якщо у тебе старий — `sudo apt install iputils-ping curl`.

## "sudo: unable to resolve host (none)"

Стара версія init не set hostname. Швидкий fix:
```bash
echo "winmux-guest" | sudo tee /etc/hostname
sudo hostname winmux-guest
echo "127.0.0.1 winmux-guest localhost" | sudo tee /etc/hosts
```
У свіжих версіях (v0.1.0+) це автоматично.

## DNS не працює (apt update fails)

```bash
sudo rm -f /etc/resolv.conf
echo 'nameserver 8.8.8.8' | sudo tee /etc/resolv.conf
echo 'nameserver 1.1.1.1' | sudo tee -a /etc/resolv.conf
```

## Stop VM кнопка не реагує

Натисни **Force kill all** у Recovery секції (вб'є все примусово).

## Текст у терміналі "пливе" / артефакти

В останній версії використовуємо WebGL renderer — мало б зникнути. Якщо все ж є — перевір що WebView2 оновлений (Edge оновити).

## Installer пише "Error opening file for writing"

Якийсь WinMux-процес ще тримає файли:
1. Закрий installer (Abort)
2. PowerShell: `Stop-Process -Name winmux*,qemu* -Force`
3. Видали папку install (`%LOCALAPPDATA%\WinMux`)
4. Запусти setup знову

## QEMU крашить через 1с з WHPX

WHPX не любить `-cpu max` (VP exit code 4) — у нашому контролері при WHPX `-cpu` не передається. Якщо все одно крашить — у Settings вибери `tcg`.

WHPX також не працює в nested virtualization (VMware-VM, наприклад). Це обмеження Windows Hypervisor Platform.
