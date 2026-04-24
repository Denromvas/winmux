# PoC Findings — Етап 0

> **Дата:** 2026-04-24
> **Тестова машина:** your-test-host (your-server), Windows Server 2022 Datacenter, AMD EPYC 9454P 32 cores, 134 GiB RAM
> **Особливість:** Це VMware-VM (важливо — nested virt має обмеження)
> **Тестував:** dromanyuk (Domain Admin — НЕ валідний тест "без прав адміна", див. блокери)
> **QEMU:** v11.0.0 (qemu.weilnetz.de/w64/qemu-w64-setup-20260422.exe)
> **Гостьова ОС:** Ubuntu 24.04 Minimal cloud image (250 МБ)

---

## TL;DR

✅ **Базовий концепт ПРАЦЮЄ.** QEMU + Ubuntu успішно стартує, мережа і port forwarding працюють як заплановано. Динамічне додавання портів через QMP — підтверджено.

⚠️ **Дві значущі проблеми**, які треба вирішити перед Етапом 1:
1. **Shared FS НЕ ПРАЦЮЄ** — офіційний QEMU для Windows не має 9p/virtiofs. Потрібна або власна збірка, або альтернатива.
2. **WHPX крашить boot у nested-VMware** — потрібно протестувати на реальному залізі / non-VMware гіпервізорі.

⚠️ **Час старту під TCG: 34 секунди** від запуску QEMU до SSH-ready (з cloud-init, без оптимізацій). Цільові 3с реалістичні лише з WHPX + кастомним init без systemd.

---

## 1. Що перевіряли і що вийшло

| Перевірка | Статус | Деталі |
|-----------|--------|--------|
| QEMU встановлюється portable (без інсталятора) | ✅ | Витягли через `7z x` з NSIS, ~1.2 ГБ розпаковано (можна обрізати до x86_64-only ~300 МБ) |
| QEMU запускається без admin elevation | ⚠️ | Тестував як admin (dromanyuk = Domain Admin тут). Треба окремий non-admin test. |
| Боот Ubuntu cloud image | ✅ | Завантажується повністю |
| cloud-init seed.iso | ✅ | Користувач winmux/winmux створено, sudo NOPASSWD працює |
| Network NAT (інтернет з гостя) | ✅ | `curl https://example.com` → 200 за 0.5с |
| Static port forward (`hostfwd=tcp::2222-:22`) | ✅ | SSH у гість через хост на 2222 |
| Динамічний port forward (QMP `hostfwd_add`) | ✅ | **Ключова фіча для auto-port-watching!** Працює без рестарту |
| WHPX accelerator | ❌ (на цьому стенді) | Crash на kernel boot. Nested-VMware обмеження. |
| TCG accelerator | ✅ | Працює стабільно, 34 сек cold start |
| 9p shared filesystem | ❌ | `fsdev support is disabled` у Windows-білді QEMU |
| virtiofs | ❌ | `virtiofsd.exe` відсутній у Windows-релізах QEMU |
| Boot logs через `-serial file:` | ✅ | Все читається |
| QMP через TCP (`-qmp tcp:127.0.0.1:4444,server=on,wait=off`) | ✅ | JSON-RPC, parse + execute працює з PowerShell |
| Доступ до guest через jump-SSH (Linux→Windows→Guest) | ✅ | `ssh -J` через ProxyCommand працює |

---

## 2. Виміряні показники

### 2.1. Розміри
| Артефакт | Розмір |
|----------|--------|
| QEMU розпакований (повний) | 1.2 ГБ |
| Ubuntu 24.04 cloud image (qcow2) | 250 МБ |
| seed.iso (cloud-init) | 370 КБ |
| Архів для перенесення (tar.gz) | 495 МБ |
| RAM при роботі (виділено 2 ГБ) | ~970 МБ used |
| CPU при простої гостя | <5% |

### 2.2. Часи
| Стадія | Час |
|--------|-----|
| Cold boot з cloud-init (baseline) | 34 секунди |
| **Boot після disable cloud-init+snap+apt-daily** | **10 секунд** (3.4x speedup) |
| systemd-analyze: kernel | 2.7s |
| systemd-analyze: userspace | 8.0s |
| Найбільший винуватець (cloud-init) | 25.3s — повністю усунено |
| Час до login prompt у serial console | ~52 сек (baseline) |
| Запуск `python -m http.server` у госту | <1 секунда |
| Затримка HTTP-запиту host→guest:8080 (через hostfwd) | <50 мс |
| QMP `hostfwd_add` execute time | <100 мс |
| **SFTP-mount benchmark (read 70 МБ)** | **85 МБ/с** |
| SFTP-mount write 50 МБ | 18.6 МБ/с |
| SFTP-mount read-after-write 50 МБ | 39.9 МБ/с |
| SFTP-mount створити 100 малих файлів | 1.37с (~73 файли/с) |

---

## 3. Знайдені блокери і їх масштаб

### 3.1. БЛОКЕР: shared filesystem
**Симптом:**
```
qemu-system-x86_64.exe: -fsdev local,id=fs0,path=share,security_model=mapped-xattr:
There is no option group 'fsdev'
fsdev support is disabled
```

**Причина:** офіційні Windows-білди QEMU (qemu.weilnetz.de) скомпільовані без `--enable-virtfs`. Це через залежність від `libcap-ng` яка має проблеми на Windows.

**Варіанти вирішення (у порядку складності):**
1. **Кастомна збірка QEMU** через MSYS2 з нестандартним конфігом — pinned task на дослідження.
2. **virtiofsd окремо** — на Windows немає офіційних бінарників, є форк `virtiofsd-rs` (Rust). Треба перевірити чи запускається і чи QEMU 11.0 його підхопить.
3. **SMB на хості** — QEMU вміє `-netdev user,...,smb=path`, але потребує `smbd` на хості (Samba немає на Windows). Можна підняти embedded SMB-сервер на Rust/Go.
4. **SFTP-mount у госту** — гість сам монтує `/c` через SFTP до хоста. Потребує SFTP-сервера на Windows (OpenSSH server вже є — і він вже стоїть! бо ми SSH-нулися). Цей варіант може бути простим.
5. **HTTP/WebDAV** — найпростіше, але повільне.

**Рекомендація:** для v1 спробувати в порядку 4 → 2 → 1. Якщо нічого — fallback на rsync sync (бракує real-time).

### 3.2. БЛОКЕР: WHPX у nested VMware
**Симптом:** kernel завантажується до "Write protecting kernel read-only data", потім QEMU процес тихо помирає без логів.

**Причина:** AMD EPYC 9454P експонує `VirtualizationFirmwareEnabled: True`, але `SecondLevelAddressTranslationExtensions: False` у nested guest. WHPX потребує SLAT.

**Не блокер для продукту:** на реальному залізі (без VMware-між) WHPX має працювати. Треба окремо перевірити на:
- Звичайному Windows 11 desktop
- Windows Server 2022 на bare metal або під Hyper-V (де nested SLAT доступний)

### 3.3. ⚠️ Тестове середовище НЕвалідне для перевірки "без прав адміна"
**Проблема:** `dromanyuk` на `your-test-host` = `EXAMPLE\Domain Admins` + `BUILTIN\Administrators`. Усі тести проводилися з admin token.

**Що треба:**
- Створити non-admin локального користувача на your-test-host (або інший сервер)
- Повторити старт QEMU під цим користувачем
- Перевірити: чи WHPX доступний non-admin'у? (потенційна проблема: hypervisorPlatform feature може потребувати prowilege)

---

## 4. Архітектурні підтвердження для ТЗ

| Фіча в ТЗ | PoC підтвердив? | Коментар |
|-----------|----------------|----------|
| QEMU user-mode | ✅ (під admin) | Треба переперевірити non-admin |
| WHPX як акселератор | ⚠️ | На bare metal має бути |
| Ubuntu Minimal як rootfs | ✅ | Працює як очікувано |
| Cloud-init для bootstrap | ✅ | seed.iso підхопився, користувач створено |
| SLIRP NAT для інтернету | ✅ | Працює з коробки |
| Static `hostfwd` при старті | ✅ | Працює |
| **Динамічний `hostfwd_add` через QMP** | ✅ | **Підтверджено! Auto-port-forwarding реальний.** |
| QMP як IPC канал host↔QEMU | ✅ | JSON-RPC working |
| **virtiofs/9p shared FS** | ❌ | Треба обхід (див. 3.1) |
| `-display none -serial file:` для headless | ✅ | Працює, логи читаються |

---

## 5. Корекції до ТЗ (увійде в TZ.md v0.3)

### 5.1. Розділ 9 "Файлова система"
**Замість:** "virtiofs основна, 9p fallback".
**Стане:** "Дослідити одне з: (a) custom QEMU build with virtfs, (b) virtiofsd-rs, (c) SFTP-mount у госту до OpenSSH server на хості, (d) embedded SMB-сервер. Pinned задача в Етапі 1 PoC."

### 5.2. Розділ 7 "Нефункціональні вимоги"
- NFR-02 "Cold start ≤3 сек" — корекція: "≤3 сек з WHPX. З TCG fallback — 30-45 сек прийнятно для не-критичних випадків. UI має показувати прогрес-бар з підказкою про відсутність HW-acceleration."
- NFR-01 "Розмір ≤ 80 МБ" — переглянути після обрізки QEMU. Зараз перший заміри: повний QEMU 300 МБ після відсіювання інших архітектур + Ubuntu rootfs ~250 МБ = ~550 МБ. Цільовий розмір треба підняти до **600-800 МБ** для v1.

### 5.3. Розділ 8.3 "Init-система"
Підтверджено — без оптимізацій systemd-init заняв ~52 сек до login. Кастомний `winmux-init` має бути високого пріоритету в Етапі 1, інакше навіть з WHPX буде довго.

### 5.4. Розділ 25 "Ризики"
Додати:
- **Ризик "Custom QEMU build necessity"**: висока ймовірність, високий вплив. Мітигація: PoC окремої гілки в Етапі 1, паралельно з основною роботою.
- **Ризик "WHPX недоступний у нестандартних середовищах"**: треба явний banner в UI коли працюємо на TCG ("Performance limited — no hardware acceleration available").

---

## 6. Що тестове середовище НЕ дало перевірити (треба інша машина)

1. **Real WHPX performance** — потрібен Win11 desktop або bare-metal Server.
2. **Non-admin user run** — створити окремого користувача без прав адміна.
3. **Antivirus interaction** — на тестовому сервері Defender виглядає що мінімально активний, ніяких блокувань QEMU не було. Треба перевірити з реальним EDR (CrowdStrike, SentinelOne).

---

## 7. Рекомендації перед Етапом 1

### Робити одразу:
1. ✅ **Custom QEMU build research** — спробувати MSYS2 збірку з `--enable-virtfs`. Якщо вийде — це найкращий шлях.
2. ✅ **Non-admin test** — створити non-admin user на your-test-host, повторити boot.
3. ✅ **Bare-metal WHPX test** — спробувати на реальній Win11 машині (можливо моя dual-boot Windows на цьому ПК або інший hardware).
4. ✅ **Кастомний `winmux-init`** — навіть PoC заміри показали, що systemd-cloud-init це 80% boot time. Кастомний init дасть негайний 5-10x speedup.

### Можна відкласти:
- Frontend (Tauri) — почнемо коли core engine стабільний.
- Drag-and-drop UI — після core.
- Telemetry backend — після Етапу 2-3.

---

## 8. Файли і артефакти PoC

### На Linux-хості (`/mnt/e/winmux/research/downloads/`):
- `qemu-w64-setup.exe` (178 МБ) — інсталятор QEMU
- `qemu/` (1.2 ГБ) — розпакований QEMU 11.0
- `ubuntu-24.04.img` (250 МБ) — Ubuntu cloud image
- `seed.iso` (370 КБ) — cloud-init seed
- `seed/` — джерельні файли seed (user-data, meta-data)
- `start.bat` / `start-bg.bat` / `run-detached.ps1` — скрипти запуску
- `share/` — папка для майбутнього 9p mount (поки не використовується)
- `winmux-poc.tar.gz` (495 МБ) — пакетний archive для перенесення

### На Windows Server (`E:\winmux-poc\`):
- Все вище + `user.qcow2` — overlay диск
- `boot.log` — серійна консоль гостя
- `launch.log` / `stdout.log` / `stderr.log` — логи запуску

---

## 9. Команди для повторення PoC

### Старт QEMU (TCG, working):
```cmd
ssh dromanyuk@your-test-host
cd /d E:\winmux-poc
qemu\qemu-system-x86_64.exe ^
  -accel tcg -m 2G -smp 4 ^
  -drive file=user.qcow2,if=virtio,format=qcow2 ^
  -drive file=seed.iso,if=virtio,format=raw,readonly=on ^
  -netdev user,id=n0,hostfwd=tcp::2222-:22 ^
  -device virtio-net-pci,netdev=n0 ^
  -display none -serial file:boot.log ^
  -qmp tcp:127.0.0.1:4444,server=on,wait=off
```

### SSH у guest (з мого Linux):
```bash
sshpass -p winmux ssh -o StrictHostKeyChecking=no \
  -o ProxyCommand="ssh -W 127.0.0.1:2222 dromanyuk@your-test-host" \
  winmux@localhost
```

### Динамічний port forward через QMP:
```powershell
$qmp = New-Object System.Net.Sockets.TcpClient("127.0.0.1",4444)
$stream = $qmp.GetStream()
$r = New-Object System.IO.StreamReader($stream)
$w = New-Object System.IO.StreamWriter($stream); $w.AutoFlush = $true
$r.ReadLine()  # greeting
$w.WriteLine('{"execute":"qmp_capabilities"}'); $r.ReadLine()
$w.WriteLine('{"execute":"human-monitor-command","arguments":{"command-line":"hostfwd_add tcp:127.0.0.1:8080-:8080"}}')
$r.ReadLine()  # "return": ""
```

---

## 10. Висновок

**Концепт WinMux валідовано.** Критичні фічі — boot, network, port forwarding, IPC через QMP — працюють як очікувано. Основний несподіваний результат — відсутність 9p у Windows QEMU, що робить shared FS центральним блокером Етапу 1.

---

## 11. Етап 1 — додаткові експерименти (2026-04-24, друга сесія)

### 11.1. SFTP-mount для shared FS — РОБОЧИЙ ОБХІД ✅

Замість 9p/virtiofs використовуємо `sshfs` у госту до OpenSSH server на Windows-хості (через NAT шлюз 10.0.2.2).

**Команда монтування з гостя:**
```bash
sshpass -p $PASS sshfs -o reconnect -o ServerAliveInterval=15 -o cache=no \
  dromanyuk@10.0.2.2:E:/winmux-poc ~/win
```

**Заміри продуктивності:**
| Операція | Швидкість | Час |
|----------|-----------|-----|
| Read 70 МБ файл | 85 МБ/с | 0.83с |
| Write 50 МБ | 18.6 МБ/с | 2.81с |
| Read-after-write | 39.9 МБ/с | 1.31с |
| Створити 100 малих файлів | 73/с | 1.37с |
| ls 101 файлів | дуже швидко | 32 мс |

**Висновок:** SFTP-mount дає прийнятну продуктивність для розробки. **Цього достатньо для v1**. Custom QEMU build з virtfs можна спробувати в Етапі 2 для покращення.

**Обмеження:**
- Permissions показуються root:root (треба `-o uid=1000,gid=1000`)
- Нема inotify (потрібен polling-mode для watchers: `CHOKIDAR_USEPOLLING=1`)
- Latency на дрібних операціях (через SLIRP NAT)
- Залежність від OpenSSH server на хості (вбудований у Win10 1803+)

### 11.2. virtiofsd-rs для Windows — НЕ ГОТОВЕ

virtiofsd-rs (gitlab.com/virtio-fs/virtiofsd) v1.13.3 не має precompiled Windows binaries. Cross-compile через MinGW/MSYS2 — окрема велика задача. **Не пріоритет для v1.**

### 11.3. Boot оптимізація — 3.4x SPEEDUP ✅

| Конфігурація | Boot time |
|--------------|-----------|
| Baseline з cloud-init | **34 с** |
| Disable cloud-init (`/etc/cloud/cloud-init.disabled`) | 10.8 с |
| + apparmor, systemd-resolved, fwupd, motd-news, apt-daily masked | **9.9 с** (systemd-analyze) |
| Реальний SSH-ready через polling | 10-13 с |

**Винуватці baseline:**
- cloud-init-local.service: 9.553s
- cloud-init.service: 6.597s
- cloud-config.service: 5.636s
- cloud-final.service: 3.535s
- **= 25.3 секунди на cloud-init alone**

**Що залишилось у нашому 9.9с:**
- 2.7s kernel (нижня межа без оптимізації QEMU)
- 2.9s dev-vda1.device (waiting for root)
- 1.4s user@1000.service
- 0.9s systemd-udev-trigger

**Очікування для Етапу 1:**
- Кастомний `winmux-init` без systemd → 2-3 сек на TCG
- WHPX на bare metal → ще 5-10x → **0.3-0.6 сек cold boot реалістично**

### 11.4. Frozen image — створено, але не дав speedup на TCG

Після оптимізації boot — створили `frozen.qcow2`:
- Compressed (qemu-img convert -O qcow2 -c): **306 МБ**
- Uncompressed: 858 МБ

Boot з frozen overlay: 19-23 с (схоже на оригінал) — на TCG decompression overhead перекривається економією. На WHPX різниця має бути помітнішою.

**Корисно для дистрибуції:** один compressed 306 МБ файл vs Ubuntu original 250 МБ + значні зміни.

### 11.5. Підсумок Етапу 1 (на середину)

| Завдання | Статус | Результат |
|----------|--------|-----------|
| E1-01 SFTP-mount | ✅ DONE | 85 МБ/с read, працює як shared FS |
| E1-02 virtiofsd-rs | ⏸ DEFERRED | Не пріоритет, для Етапу 2 |
| E1-03 Boot optimize | ✅ DONE | 34с → 10с (3.4x) |
| E1-04 Frozen image | ✅ DONE | 306 МБ compressed; speedup на TCG не помітний |
| **E1-07 Custom winmux-init** | ✅ **DONE** | **34с → 2с (17x speedup на TCG)** |
| **E1-05 Bare-metal WHPX** | ✅ **DONE** | Працює на Win11 ноуті, init READY 158 мс (vs 311 TCG = 2x) |
| E1-06 Non-admin test | 🔲 TODO | Чекає створення non-admin юзера |

### 11.7. WHPX bare-metal тест — DESKTOP-O5FIQ97 (Acer A715, Ryzen 5 3550H, Win11)

**Налаштування:**
- `Enable-WindowsOptionalFeature -Online -FeatureName HypervisorPlatform -All`
- Reboot (~30 секунд downtime)

**Результат:**
| Метрика | TCG | WHPX |
|---------|-----|------|
| Init READY (ms) | 311 мс | **158 мс** (2x) |
| network done | 294 мс | 131 мс |
| sshd ready | 304 мс | 149 мс |
| Загальний boot до SSH | 5 с | 6 с (jump-overhead) |

**Чому загальний 6 сек (а не <1 сек як очікувалось):**
- 3 SSH jumps (моя машина → your-test-host → ноут → guest 127.0.0.1:2223) додає ~2-3 сек на встановлення з'єднання
- Polling кожні 1 сек дає +0.5 сек
- QEMU initialization з WHPX setup ~1-2 сек
- Реальний kernel→init→SSH-listening: ~2 сек

**Очікування на десктопі без jumps:**
- Direct ssh: ~3-4 сек total від click до prompt
- Це відповідає цільовим **3 секунди** з ТЗ

**WHPX особливості:**
- `WHPX VP exit code 4` крашить з `-cpu max` — треба `-cpu host` або без cpu опції
- Працює навіть коли Windows WMI показує `SLAT: False` (це quirk коли Hyper-V running)
- Конфлікти з MPX/APX features → warnings, але не блокери

### 11.6. winmux-init v0.1.0 — РЕЗУЛЬТАТИ

**Розташування:** `/mnt/e/winmux/code/winmux-init/`

**Розмір бінарника:** 453 КБ (Rust + musl static)

**Що робить (по порядку):**
1. `mount` proc, sysfs, devpts, tmpfs (run, tmp, dev/shm) — 27 мс
2. `ip link set eth0 up` + DHCP (dhclient/udhcpc) АБО static fallback `10.0.2.15/24` — +267 мс
3. `ssh-keygen -A` (якщо host keys нема) + `sshd -D` — +10 мс
4. Fork → `agetty --autologin winmux ttyS0` — +7 мс
5. **READY за 311 мс**

**Boot timeline (з kmsg):**
```
2.589s [winmux-init] WinMux init v0.1.0 starting (pid 1)
2.596s [winmux-init] mounts done at +27ms
2.829s [winmux-init] brought up eth0
2.861s [winmux-init] static IP 10.0.2.15/24 fallback applied
2.863s [winmux-init] network done at +294ms
2.867s [winmux-init] starting sshd...
2.871s [winmux-init] sshd PID=109
2.872s [winmux-init] sshd done at +304ms
2.878s [winmux-init] login spawned PID=110
2.880s [winmux-init] READY at +311ms
```

**Загальний boot:** kernel ~2.5с (TCG межа) + init 311мс = **~3 секунди до login prompt**.
**SSH-ready:** виміряний 2с (полінг 1с overhead).

**Очікування на WHPX (bare metal):** kernel ~0.3-0.5с + init 100-200 мс = **<1 секунда total**. Це досяжна цільова метрика з ТЗ.

**Порівняння:**
| Конфіг | Boot |
|--------|------|
| Baseline (cloud-init + systemd) | 34 с |
| Disable cloud-init | 10 с |
| **winmux-init (custom PID 1)** | **2 с (TCG) → ~0.5 с (WHPX bare metal expected)** |

**Що ще треба допилити:**
- DNS resolution (curl повертає HTTP 000 — `/etc/resolv.conf` встановлюється, але resolver не працює; ймовірно треба ще `/etc/nsswitch.conf` або glibc resolver issues)
- Hostname (зараз `(none)` — треба `sethostname`)
- Mount sshfs з guest до host — додати в init як опцію
- Реакція на signals (SIGTERM від QEMU shutdown → graceful shutdown)
- Підтримка virtio-serial для IPC з контролером

---

## 12. Наступні кроки

**Готовий перейти до:**
1. Кастомний `winmux-init` — найбільший потенційний прорив для холодного старту
2. Bare-metal WHPX тест на робочому ноуті
3. Non-admin user test

**Запропонувати користувачу:**
- Чи дає згоду на тести на робочому ноуті your-laptop-ip? (це його робоча машина, треба явний дозвіл)
- Чи робимо non-admin user на your-test-host (вимагає admin для створення)?
