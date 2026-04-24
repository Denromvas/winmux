# Швидкий старт

## 1. Завантаження

Скачай актуальний `winmux-desktop-setup-vX.Y.Z.exe` з [GitHub Releases](https://github.com/) (буде).

## 2. Установка

Подвійний клік на setup → Next → Install. Без UAC, без admin. Установлюється в `%LOCALAPPDATA%\WinMux\` (~510 МБ).

## 3. Перший запуск

Знайди ярлик **WinMux Desktop** у Start Menu або на робочому столі.

При натисканні **Start VM**:
1. Спочатку (один раз) — створюється overlay диск з base.qcow2 (~2 секунди)
2. QEMU стартує (~1 секунда)
3. Linux завантажується (~2 секунди)
4. У головному терміналі з'являється запит пароля
5. Введи `winmux` — і ти в bash

## 4. Перший корисний крок — `claude`

```bash
# Налаштування API key (один раз)
echo 'export ANTHROPIC_API_KEY="sk-ant-..."' >> ~/.bashrc
source ~/.bashrc

# Перевір
claude --version

# Запуск
claude
```

## 5. Робота з Windows-файлами

Claude бачить тільки файли всередині Linux. Щоб дати йому доступ до твоїх Windows-проектів:

```bash
winmux-mount
# → запитає Windows user, шлях, пароль
# → змонтує у ~/win/

cd ~/win/Projects/myapp
claude
```

Деталі: [Монтування Windows-папок →](mount.md)
