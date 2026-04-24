# Установка

## Системні вимоги

- **Windows 10 1809+** (build 17763), Windows 11, Windows Server 2019/2022
- **x86_64** (Intel/AMD)
- **4 ГБ RAM** мінімум, рекомендується 8 ГБ (з них 2 ГБ для VM)
- **1.5 ГБ вільного місця** на диску
- WebView2 Runtime (вже стоїть на Win10/11 за замовч.; ставиться окремо для Server Core)
- НЕ потрібно: Hyper-V, WSL, права адміністратора

## Способи установки

### NSIS Installer (рекомендовано)

`winmux-desktop-setup-vX.Y.Z.exe` (~508 МБ):
- Per-user install у `%LOCALAPPDATA%\WinMux\`
- Створює Start Menu shortcuts
- Додає "Open in WinMux" у Explorer context menu (правий клік на папці)
- Реєструє в Add/Remove Programs

```cmd
:: Інтерактивно
winmux-desktop-setup-v0.1.0.exe

:: Silent
winmux-desktop-setup-v0.1.0.exe /S
```

### Portable ZIP

`winmux-desktop-portable-vX.Y.Z.zip` (~552 МБ):
- Розпакуй куди хочеш (наприклад на флешку)
- Запусти `winmux-desktop.exe`
- Без реєстру, без shortcuts, повністю portable

## Видалення

- Через Add/Remove Programs (стандартний шлях)
- Або вручну: видали `%LOCALAPPDATA%\WinMux\` та папки в Start Menu

## Multi-user на одному Windows

Кожен Windows-юзер ставить свою копію в свій `%LOCALAPPDATA%\WinMux\`. Повна ізоляція.
