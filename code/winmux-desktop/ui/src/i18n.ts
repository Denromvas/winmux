// Простий i18n без додаткових залежностей
type Lang = "uk" | "en";

const dict: Record<string, Record<Lang, string>> = {
  "vm.start": { uk: "Запустити VM", en: "Start VM" },
  "vm.starting": { uk: "Запускається...", en: "Starting..." },
  "vm.stop": { uk: "Зупинити VM", en: "Stop VM" },
  "vm.running": { uk: "Працює", en: "Running" },
  "vm.stopped": { uk: "Зупинено", en: "Stopped" },
  "ports.title": { uk: "Прокинуті порти", en: "Ports forwarded" },
  "ports.none": { uk: "немає", en: "none" },
  "view.title": { uk: "Вигляд", en: "View" },
  "actions.title": { uk: "Дії", en: "Actions" },
  "actions.palette": { uk: "⌘ Палітра команд", en: "⌘ Command palette" },
  "actions.settings": { uk: "⚙ Налаштування", en: "⚙ Settings" },
  "actions.ssh": { uk: "↗ SSH у новому вікні", en: "↗ SSH in new window" },
  "actions.advanced": { uk: "⚙ Розширене", en: "⚙ Advanced" },
  "recovery.kill": { uk: "Force kill all", en: "Force kill all" },
  "recovery.reset": { uk: "Reset session", en: "Reset session" },
  "log.title": { uk: "Лог контролера", en: "Controller log" },
  "tab.welcome.stopped": { uk: "Натисни Start VM у бічній панелі.", en: "Press Start VM in the sidebar." },
  "tab.welcome.running": { uk: "Гість завантажений. Натисни + щоб відкрити термінал.", en: "Guest is up. Press + to open a terminal." },
  "telemetry.title": { uk: "Анонімна телеметрія", en: "Anonymous telemetry" },
  "telemetry.body": {
    uk: "Допомогти нам покращувати WinMux? Збираємо тільки версію WinMux/Windows, тип CPU, теми/розмір шрифту, лічильники використання фіч і крах-репорти. Жодних файлів, шляхів, команд, IP, паролів.",
    en: "Help us improve WinMux? We collect only WinMux/Windows version, CPU type, themes/font, feature counters and crash reports. No files, paths, commands, IPs, passwords.",
  },
  "telemetry.host": { uk: "Self-hosted: ", en: "Self-hosted: " },
  "telemetry.no": { uk: "Не надсилати", en: "Don't send" },
  "telemetry.yes": { uk: "Згоден", en: "Allow" },
};

let current: Lang = (localStorage.getItem("winmux.lang") as Lang) || "uk";

export function getLang(): Lang { return current; }
export function setLang(l: Lang) {
  current = l;
  localStorage.setItem("winmux.lang", l);
  // Простий refresh: перезавантажити.
  // Для більш плавного UX треба React Context, але для альфи OK.
  window.location.reload();
}
export function t(key: string): string {
  const e = dict[key];
  if (!e) return key;
  return e[current] || e.en || key;
}
