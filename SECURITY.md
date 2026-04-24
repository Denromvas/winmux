# Security Policy

## Звітування вразливостей

Якщо знайшов security issue в WinMux — **не створюй public issue**. Напиши на email:

📧 **claudetaistra@gmail.com**

Або через GitHub Security Advisories:
https://github.com/Denromvas/winmux/security/advisories/new

Я відповім протягом 48 годин і виправлю критичні issues протягом тижня (поки проект-один-розробник).

## Що вважати security issue

- Можливість виконання довільного коду на хості з гостя через QEMU/SLIRP exploit
- Можливість escalate privileges (запустити щось як Administrator з юзерського процесу)
- Розкриття credentials (API keys, паролі, SSH keys) через telemetry, логи, чи crash reports
- Будь-яка можливість для інших Windows-юзерів читати чужі WinMux дані

## Що НЕ є security issue (за дизайном)

- Гість має повний доступ до твоїх Windows-файлів через sshfs — це фіча, не баг
- Ports forwarded на 127.0.0.1 (не 0.0.0.0) — на shared машинах це може бути видно іншим юзерам, але це опція в config
- WinMux НЕ є sandbox від користувача — він твій інструмент, працює з твоїми правами

## Підтримувані версії

| Version | Supported |
|---------|-----------|
| 0.1.x   | ✅ (alpha) |
| < 0.1   | ❌         |

Після v1.0 — security patches на N-1 minor (наприклад v1.0 + v0.9).

## Telemetry security

Telemetry сервер `telemetry.denromvas.website` — self-hosted, open source backend (`/home/dromanyuk/api/winmux-telemetry`).
Не приймає IP-адреси клієнтів (nginx не передає X-Forwarded-For в backend).
Зберігає тільки структуровані дані (версії, типи подій, лічильники), не приймає вільний текст.

Можна повністю відключити в Settings → "Telemetry" або у `%LOCALAPPDATA%\WinMux\telemetry.toml`:
```toml
enabled = false
```
