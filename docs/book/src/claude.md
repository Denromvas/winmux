# Claude Code в WinMux

WinMux спеціально оптимізований для роботи з `claude_code` — повноцінний POSIX-shell, всі стандартні утиліти, Node.js v22, нормальний bash.

## Налаштування API key

Найпростіший спосіб:

```bash
# Один раз
echo 'export ANTHROPIC_API_KEY="sk-ant-..."' >> ~/.bashrc
source ~/.bashrc

# Перевірка
claude --version
```

## Геоблокування Anthropic

Anthropic блокує використання Claude Code login flow з України та інших країн зі списку. **Але API keys (`sk-ant-...`) працюють звідусіль.**

Якщо хочеш використовувати login замість API key:
- Через VPN (WireGuard на маршрутизатор / комерційний VPN)
- На цьому WinMux: `sudo apt install wireguard && sudo cp wg0.conf /etc/wireguard/ && sudo wg-quick up wg0`

## Робочий процес

```bash
# Змонтуй свій проект
winmux-mount -u dromanyuk -p /D:/projects/myapp -m ~/proj
cd ~/proj

# Запусти claude
claude

# Або з prompt одразу
claude "проаналізуй структуру проекту"
```

## Drag-and-drop файлів у claude (з Windows!)

У Desktop-edition можна:

1. **Перетягнути файл** з Explorer прямо у вікно WinMux → шлях вставиться у командний рядок
2. **Перетягнути картинку** з браузера → збережеться в `drops/` і шлях вставиться
3. **Ctrl+V зображення з буфера** → те саме

Після цього просто пиши `claude` чи `claude --image=<path>` — агент бачить файл.

## Корисні аліаси

Додай у `~/.bashrc`:

```bash
alias cc='claude'
alias ccs='claude --no-interactive'
# Щоб claude бачив проект з Windows
alias project='cd ~/win/Projects && ls'
```
