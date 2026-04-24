# Монтування Windows-папок

Linux всередині WinMux — це VM, тому Windows-файли йому за замовчуванням не видно. Щоб дати доступ — монтуємо через `sshfs` до OpenSSH-сервера на хості (Windows OpenSSH вже є з Win10 1803+).

## Інтерактивний спосіб

```bash
winmux-mount
```

Запитає:
- Windows user (default: `Admin`)
- Windows path (default: `/C:/Users/Admin`)
- Mount dir (default: `~/win`)
- Password

## CLI з аргументами

```bash
winmux-mount -u dromanyuk -p /D:/projects -m ~/proj
```

## Через env vars (без паролю в історії)

```bash
export WINMUX_HOST_USER=dromanyuk
export WINMUX_HOST_PATH=/E:/winmux-test
export WINMUX_HOST_PASS='your-host-password'  # одинарні лапки для безпеки!
winmux-mount
```

## Перевірка

```bash
ls ~/win
df -h ~/win
mount | grep fuse
```

## Демонтування

```bash
fusermount -u ~/win
```

## Важливі деталі

- Шлях у форматі `/<буква_диска>:/<шлях>` — тобто `C:\Users\Admin` стає `/C:/Users/Admin`
- Гість бачить файли як власні (uid mapping), тому редагування і збереження працюють
- Зміни з обох сторін видно в real-time
- Для watch-режимів (nodemon, vite dev) увімкни polling: `CHOKIDAR_USEPOLLING=1`

## Швидкість

| Операція | Швидкість |
|----------|-----------|
| Read великого файлу | ~85 МБ/с |
| Write | ~18 МБ/с |
| Read after write | ~40 МБ/с |
| Створити 100 малих файлів | ~73 файли/с |

## Майбутнє: auto-mount

У наступних релізах WinMux буде автоматично монтувати твою Windows-домашню папку у `/workspace` без жодних команд (через persistent SSH key).
