# FAQ

## Чи це віртуалка? Як WSL?

WinMux запускає QEMU у user-mode — це віртуалка, але *без потреби в Hyper-V/WSL*. Тобі не треба прав адміністратора, не треба перезавантаження, не треба нічого ввімкнути в BIOS (хоча WHPX дасть прискорення якщо є).

WSL2 теж віртуалка, але потребує Hyper-V Platform → права адміна та перезавантаження. На багатьох Windows Server це заборонено політикою. WinMux — обходить це обмеження.

## Чи це повільно?

На TCG (без WHPX) — ~2-5 сек boot, ~10-15% накладок на CPU. Прийнятно для bash, git, npm install. Для важких обчислень — увімкни WHPX.

З WHPX — практично native speed.

## Чи зберігаються мої файли між запусками?

Так. У `%LOCALAPPDATA%\WinMux\rootfs\user.qcow2` — це твій overlay над base image. Все що ти зробив у Linux (apt install, npm install, файли в `~/`) — там.

**Reset session** видаляє overlay і все обнуляється до фабричного frozen image. Це безпечно для твоїх Windows-файлів — вони на хості, в overlay не зачіпаються.

## Чи працює Docker?

Docker daemon у guest — теоретично так, але треба багато налаштувань. Зараз не підтримуємо з коробки. **Docker CLI** (`docker` команда без demon) — можна встановити, з'єднується з Docker Desktop на хості.

## Чи безпечно?

WinMux — не пісочниця для захисту від користувача. Гість має повний доступ до твоїх Windows-файлів через спільну ФС, до інтернету, до localhost-портів. Ізоляція рівнозначна тому, що дає процес у Windows.

Чого гість НЕ може:
- Виконати щось як Windows-Administrator
- Зашкодити файлам інших Windows-юзерів
- Залізти в інші процеси Windows

## Чи можу видалити WinMux і не залишити сліду?

Так:
1. Add/Remove Programs → WinMux Desktop → Uninstall
2. Або вручну: `Remove-Item "$env:LOCALAPPDATA\WinMux" -Recurse -Force`
3. Реєстр: `Remove-Item HKCU:\Software\WinMux -Recurse`

## Як оновитись?

Зараз — переустановити свіжий setup.exe (overlay user.qcow2 і конфіг збережуться). Auto-update планується.

## А Anthropic геоблок з України?

Так, Claude Code login не працює з UA IP. **API key (sk-ant-...) працює звідусіль**. Або VPN.

## Чи це open source?

Так, MIT. Bundled QEMU під GPLv2 (qemu/COPYING).
