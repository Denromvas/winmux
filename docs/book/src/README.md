# WinMux

> Портативне Linux-середовище для Windows у стилі Termux. Без WSL, Hyper-V, прав адміністратора.

WinMux — це один портативний `.exe`, який запускає повноцінний Linux всередині Windows. Створений для:

- 🤖 Запуску `claude_code` та інших AI-агентів на Windows-десктопах і Windows Server через RDP
- 🛠️ DevOps на машинах де WSL заборонений політикою
- 🐧 Розробників які звикли до bash, але змушені працювати у Windows
- 📦 Швидкого спробування Linux без VM/dual-boot

## Дві edition

- **WinMux CLI** — лише `winmux.exe`, керування через PowerShell. Для серверів, скриптів, headless.
- **WinMux Desktop** — Tauri+xterm.js GUI. Для desktop використання.

## Швидкий приклад

```bash
# У будь-якій PowerShell на Windows після інсталяції:
winmux start

# В іншому вікні підключаєшся через SSH:
ssh -p 2223 winmux@127.0.0.1   # пароль: winmux
```

Або в Desktop-edition: подвійний клік на ярлику → з'явиться вікно з bash.

## Що всередині

- Ubuntu 24.04 Minimal (493 МБ frozen image)
- Node.js 22, npm
- Claude Code CLI (`claude`)
- git, tmux, htop, nano, vim
- sshfs, sshpass для монтування Windows-папок
- ping, curl, wget, openssh-client
- python3, build-essential

## Як працює

Контролер на Rust запускає QEMU у user-mode (без прав адміна). Linux всередині завантажується за ~2 секунди завдяки кастомному `winmux-init` (без systemd). Guest-agent моніторить порти і шле події контролеру через TCP — той автоматично прокидає їх на Windows `localhost`.

[Архітектура →](architecture.md) · [Швидкий старт →](quickstart.md)
