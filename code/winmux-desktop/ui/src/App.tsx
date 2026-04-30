import React, { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import "./App.css";
import Settings from "./Settings";
import Terminal from "./Terminal";
import CommandPalette, { Cmd } from "./CommandPalette";
import { t } from "./i18n";

interface VmStatus {
  state: "stopped" | "starting" | "running" | "error";
  pid?: number;
  uptime_sec?: number;
  ports: number[];
  guest_kernel?: string;
}

type TabKind = "term" | "browser";

interface Tab {
  id: number;          // tab logical id (== first pane's id)
  title: string;
  kind: TabKind;
  panes: number[];     // pane ids (each one is a backend tab/PTY) — only for term
  layout: "single" | "h-split" | "v-split";
  focusedPane: number;
  url?: string;        // for kind=browser
}

const themes: Record<string, any> = {
  "winmux-dark": { background:"#0a0e27",foreground:"#e0e6f0",cursor:"#00ff88",black:"#1a1a2e",red:"#ff6b6b",green:"#00ff88",yellow:"#ffd166",blue:"#669bbc",magenta:"#c77dff",cyan:"#80ffdb",white:"#e0e6f0",brightBlack:"#4a4a6a",brightRed:"#ff8787",brightGreen:"#5fff9f",brightYellow:"#ffe184",brightBlue:"#a3c9e2",brightMagenta:"#e0aaff",brightCyan:"#a8ffe6",brightWhite:"#ffffff" },
  "dracula": { background:"#282a36",foreground:"#f8f8f2",cursor:"#f8f8f2",black:"#21222c",red:"#ff5555",green:"#50fa7b",yellow:"#f1fa8c",blue:"#bd93f9",magenta:"#ff79c6",cyan:"#8be9fd",white:"#f8f8f2",brightBlack:"#6272a4",brightRed:"#ff6e6e",brightGreen:"#69ff94",brightYellow:"#ffffa5",brightBlue:"#d6acff",brightMagenta:"#ff92df",brightCyan:"#a4ffff",brightWhite:"#ffffff" },
  "tokyo-night": { background:"#1a1b26",foreground:"#c0caf5",cursor:"#c0caf5",black:"#15161e",red:"#f7768e",green:"#9ece6a",yellow:"#e0af68",blue:"#7aa2f7",magenta:"#bb9af7",cyan:"#7dcfff",white:"#a9b1d6",brightBlack:"#414868",brightRed:"#f7768e",brightGreen:"#9ece6a",brightYellow:"#e0af68",brightBlue:"#7aa2f7",brightMagenta:"#bb9af7",brightCyan:"#7dcfff",brightWhite:"#c0caf5" },
  "solarized-dark": { background:"#002b36",foreground:"#839496",cursor:"#93a1a1",black:"#073642",red:"#dc322f",green:"#859900",yellow:"#b58900",blue:"#268bd2",magenta:"#d33682",cyan:"#2aa198",white:"#eee8d5",brightBlack:"#586e75",brightRed:"#cb4b16",brightGreen:"#586e75",brightYellow:"#657b83",brightBlue:"#839496",brightMagenta:"#6c71c4",brightCyan:"#93a1a1",brightWhite:"#fdf6e3" },
  "catppuccin-mocha": { background:"#1e1e2e",foreground:"#cdd6f4",cursor:"#f5e0dc",black:"#45475a",red:"#f38ba8",green:"#a6e3a1",yellow:"#f9e2af",blue:"#89b4fa",magenta:"#f5c2e7",cyan:"#94e2d5",white:"#bac2de",brightBlack:"#585b70",brightRed:"#f38ba8",brightGreen:"#a6e3a1",brightYellow:"#f9e2af",brightBlue:"#89b4fa",brightMagenta:"#f5c2e7",brightCyan:"#94e2d5",brightWhite:"#a6adc8" },
  "github-dark": { background:"#0d1117",foreground:"#c9d1d9",cursor:"#58a6ff",black:"#484f58",red:"#ff7b72",green:"#3fb950",yellow:"#d29922",blue:"#58a6ff",magenta:"#bc8cff",cyan:"#39c5cf",white:"#b1bac4",brightBlack:"#6e7681",brightRed:"#ffa198",brightGreen:"#56d364",brightYellow:"#e3b341",brightBlue:"#79c0ff",brightMagenta:"#d2a8ff",brightCyan:"#56d4dd",brightWhite:"#f0f6fc" },
};

export default function App() {
  const [status, setStatus] = useState<VmStatus>({ state: "stopped", ports: [] });
  const [logs, setLogs] = useState<string[]>([]);
  const [tabs, setTabs] = useState<Tab[]>([]);
  const [activeTab, setActiveTab] = useState<number | null>(null);
  const [fontSize, setFontSize] = useState<number>(() => parseInt(localStorage.getItem("winmux.fontSize") || "14", 10));
  const [themeName, setThemeName] = useState<string>(() => localStorage.getItem("winmux.theme") || "winmux-dark");
  const [showSettings, setShowSettings] = useState(false);
  const [showPalette, setShowPalette] = useState(false);
  const [telemetryAsk, setTelemetryAsk] = useState(false);

  // Ctrl+Shift+P → command palette
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey && (e.key === "P" || e.key === "p")) {
        e.preventDefault(); setShowPalette(true);
      }
      if (e.ctrlKey && e.shiftKey && (e.key === "T" || e.key === "t")) {
        e.preventDefault();
        if (status.state === "running") invoke("open_tab").catch(console.error);
      }
      if (e.ctrlKey && e.shiftKey && (e.key === "D" || e.key === "d")) {
        e.preventDefault();
        if (status.state === "running") splitPane("v-split");
      }
      if (e.ctrlKey && e.shiftKey && (e.key === "S" || e.key === "s")) {
        e.preventDefault();
        if (status.state === "running") splitPane("h-split");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [status.state]);

  useEffect(() => { localStorage.setItem("winmux.fontSize", String(fontSize)); }, [fontSize]);
  useEffect(() => { localStorage.setItem("winmux.theme", themeName); }, [themeName]);

  useEffect(() => {
    invoke<{asked:boolean,enabled:boolean}>("telemetry_status").then(s => {
      if (!s.asked) setTelemetryAsk(true);
    }).catch(() => {});
  }, []);

  const [aiEvents, setAiEvents] = useState<any[]>([]);
  const projectInitFired = useRef(false);

  // Backend events
  useEffect(() => {
    const u1 = listen<VmStatus>("vm-status", async (e) => {
      setStatus(e.payload);
      // Coming up — apply per-project config (init_command in first terminal)
      if (e.payload.state === "running" && !projectInitFired.current) {
        projectInitFired.current = true;
        try {
          const projPath = await invoke<string | null>("opened_path");
          if (projPath) {
            const cfg = await invoke<any>("load_project_config", { path: projPath });
            const initCmd = cfg?.init_command;
            if (initCmd) {
              const id = await invoke<number>("open_tab");
              setTimeout(() => invoke("send_input", { tabId: id, data: initCmd + "\n" }), 1500);
            }
          }
        } catch {}
      }
    });
    const u2 = listen<string>("controller-log", (e) => setLogs(p => [...p.slice(-200), e.payload]));
    const u3 = listen<number>("tab-opened", (e) => {
      const id = e.payload;
      setTabs(prev => {
        if (prev.some(t => t.kind === "term" && t.panes.includes(id))) return prev;
        return [...prev, { id, title: `bash ${id}`, kind: "term", panes: [id], layout: "single", focusedPane: id }];
      });
      setActiveTab(id);
    });

    // Poll AI events every 2s
    const aiTimer = setInterval(async () => {
      try {
        const ev = await invoke<any[]>("ai_tail");
        setAiEvents(ev.slice(-15));
      } catch {}
    }, 2000);

    return () => { u1.then(f=>f()); u2.then(f=>f()); u3.then(f=>f()); clearInterval(aiTimer); };
  }, []);

  const startVm = () => invoke("start_vm").catch(console.error);
  const stopVm = () => { invoke("stop_vm").catch(console.error); setTabs([]); setActiveTab(null); };
  const openSsh = () => invoke("open_ssh").catch(console.error);
  const newTab = async () => {
    try { await invoke("open_tab"); } catch (e) { console.error(e); }
  };

  const openBrowserTab = (url: string) => {
    // Synthetic id (negative to avoid clash with backend tab ids).
    const id = -Math.floor(Date.now() % 1_000_000);
    const u = url.startsWith("http") ? url : `http://${url}`;
    setTabs(prev => [...prev, { id, title: new URL(u).host, kind: "browser", panes: [], layout: "single", focusedPane: id, url: u }]);
    setActiveTab(id);
  };
  const closeTab = async (id: number) => {
    const t = tabs.find(t => t.id === id);
    if (t && t.kind === "term") {
      for (const p of t.panes) {
        try { await invoke("close_tab", { tabId: p }); } catch {}
      }
    }
    setTabs(prev => {
      const next = prev.filter(t => t.id !== id);
      if (activeTab === id) setActiveTab(next.length ? next[next.length - 1].id : null);
      return next;
    });
  };

  const btnStyle: React.CSSProperties = { padding: "4px 10px", background: "#414868", color: "#e0e6f0", border: "none", borderRadius: 3, cursor: "pointer", fontSize: 13 };

  const splitPane = async (orientation: "h-split" | "v-split") => {
    if (activeTab === null) return;
    const tab = tabs.find(t => t.id === activeTab);
    if (!tab) return;
    if (tab.panes.length >= 2) return;  // MVP: max 2 panes per tab
    try {
      const newPaneId = await invoke<number>("open_tab");
      setTabs(prev => prev.map(t =>
        t.id === activeTab
          ? { ...t, panes: [...t.panes, newPaneId], layout: orientation, focusedPane: newPaneId }
          : t
      ));
    } catch (e) { console.error(e); }
  };

  const closePane = async (tabId: number, paneId: number) => {
    try { await invoke("close_tab", { tabId: paneId }); } catch {}
    setTabs(prev => prev.map(t => {
      if (t.id !== tabId) return t;
      const panes = t.panes.filter(p => p !== paneId);
      if (panes.length === 0) return null as any;
      return { ...t, panes, layout: "single" as const,
        focusedPane: panes.includes(t.focusedPane) ? t.focusedPane : panes[0] };
    }).filter(Boolean));
  };

  const forceKill = async () => {
    if (!confirm("Force kill all WinMux/QEMU processes?")) return;
    await invoke("force_kill_all").catch(console.error);
    setTabs([]); setActiveTab(null);
  };
  const resetSession = async () => {
    if (!confirm("Reset session — видалить overlay, наступний запуск з чистого state. OK?")) return;
    await invoke("reset_session").catch(console.error);
    setTabs([]); setActiveTab(null);
  };

  // Drag-and-drop (native files)
  useEffect(() => {
    let unsub: (() => void) | undefined;
    (async () => {
      const wv = getCurrentWebview();
      const u = await wv.onDragDropEvent(async (event) => {
        if (event.payload.type === "drop") {
          const paths = (event.payload as any).paths as string[];
          if (paths?.length) {
            try {
              const ins = await invoke<string>("drop_paths", { paths });
              // Pasting only into active tab
              if (activeTab !== null) {
                await invoke("send_input", { tabId: activeTab, data: ins });
              }
            } catch (e) { console.error(e); }
          }
        }
      });
      unsub = u;
    })();
    return () => { if (unsub) unsub(); };
  }, [activeTab]);

  // Clipboard image paste → save → insert path
  useEffect(() => {
    const onPaste = async (e: ClipboardEvent) => {
      const items = e.clipboardData?.items;
      if (!items) return;
      for (let i = 0; i < items.length; i++) {
        const item = items[i];
        if (item.type.startsWith("image/")) {
          e.preventDefault();
          const blob = item.getAsFile();
          if (blob) {
            const ext = item.type.split("/")[1] || "png";
            const buf = await blob.arrayBuffer();
            const path = await invoke<string>("save_image_drop", {
              bytes: Array.from(new Uint8Array(buf)), ext,
            });
            if (activeTab !== null) {
              await invoke("send_input", { tabId: activeTab, data: path });
            }
          }
          return;
        }
      }
    };
    window.addEventListener("paste", onPaste as any);
    return () => window.removeEventListener("paste", onPaste as any);
  }, [activeTab]);

  const stateColor = { stopped: "#888", starting: "#ffd166", running: "#00ff88", error: "#ff6b6b" }[status.state];

  return (
    <div className="winmux-app">
      <div className="titlebar" data-tauri-drag-region>
        <span className="title">WinMux</span>
        <span className="status" style={{ color: stateColor }}>
          ● {status.state.toUpperCase()}
          {status.uptime_sec ? ` · ${status.uptime_sec}s` : ""}
          {status.guest_kernel ? ` · ${status.guest_kernel}` : ""}
        </span>
        <div className="window-controls">
          <button onClick={() => invoke("window_minimize")}>—</button>
          <button onClick={() => invoke("window_maximize")}>□</button>
          <button onClick={() => invoke("window_close")}>×</button>
        </div>
      </div>

      <div className="main">
        <aside className="sidebar">
          {status.state !== "running" && (
            <section>
              <h3>VM</h3>
              <button onClick={startVm} className="btn-start" disabled={status.state === "starting"}>
                {status.state === "starting" ? t("vm.starting") : t("vm.start")}
              </button>
            </section>
          )}

          <section>
            <h3>{t("ports.title")}</h3>
            {status.ports.length === 0 ? (
              <p className="empty">{t("ports.none")}</p>
            ) : (
              <ul className="ports">
                {status.ports.map(p => (
                  <li key={p} style={{ display: "flex", gap: 4, alignItems: "center" }}>
                    <a style={{ flex: 1, cursor: "pointer" }}
                       onClick={() => openBrowserTab(`http://127.0.0.1:${p}`)}
                       title="Open in WinMux browser tab">:{p}</a>
                    <button title="Open in system browser" style={{ padding: "0 4px", fontSize: 10, background: "transparent", border: "1px solid #4a5680", color: "#a3c9e2", borderRadius: 2, cursor: "pointer" }}
                            onClick={() => invoke("open_url", { url: `http://127.0.0.1:${p}` })}>↗</button>
                  </li>
                ))}
              </ul>
            )}
          </section>

          <section>
            <h3>🤖 AI activity {aiEvents.length > 0 && <span style={{ color: "#00ff88" }}>● live</span>}</h3>
            {aiEvents.length === 0 ? (
              <p className="empty" style={{ fontSize: 11 }}>
                Run <code style={{ background: "#1a1b26", padding: "1px 4px", borderRadius: 2 }}>claude --output-format stream-json --verbose | tee ~/.winmux/claude.jsonl</code>
                <br/>to see live thoughts here
              </p>
            ) : (
              <div style={{ maxHeight: 200, overflowY: "auto", fontSize: 11, fontFamily: "monospace" }}>
                {aiEvents.map((ev, i) => {
                  const t = ev.type || ev.event || "?";
                  const color = t === "tool_use" ? "#ffd166" : t === "tool_result" ? "#80ffdb" : t === "assistant" ? "#a3c9e2" : "#4a5680";
                  const text = ev.message?.content?.[0]?.text || ev.tool_name || ev.name || JSON.stringify(ev).slice(0, 100);
                  return (
                    <div key={i} style={{ borderLeft: `2px solid ${color}`, padding: "2px 6px", marginBottom: 2 }}>
                      <span style={{ color, fontWeight: "bold" }}>{t}</span>
                      <div style={{ color: "#a9b1d6", whiteSpace: "pre-wrap", overflow: "hidden", textOverflow: "ellipsis", maxHeight: 60 }}>
                        {String(text).slice(0, 200)}
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </section>

          <section>
            <h3>View — {fontSize}px</h3>
            <div style={{ display: "flex", gap: 4 }}>
              <button onClick={() => setFontSize(s => Math.max(8, s - 1))} className="btn-warn" style={{ flex: 1 }}>A−</button>
              <button onClick={() => setFontSize(14)} className="btn-warn" style={{ flex: 1 }}>14</button>
              <button onClick={() => setFontSize(s => Math.min(32, s + 1))} className="btn-warn" style={{ flex: 1 }}>A+</button>
            </div>
            <select value={themeName} onChange={(e) => setThemeName(e.target.value)} className="theme-picker" style={{ marginTop: 6 }}>
              <option value="winmux-dark">WinMux Dark</option>
              <option value="dracula">Dracula</option>
              <option value="tokyo-night">Tokyo Night</option>
              <option value="solarized-dark">Solarized Dark</option>
              <option value="catppuccin-mocha">Catppuccin Mocha</option>
              <option value="github-dark">GitHub Dark</option>
            </select>
          </section>

          <section>
            <h3>{t("actions.title")}</h3>
            <button onClick={() => setShowPalette(true)} className="btn-warn" title="Ctrl+Shift+P">{t("actions.palette")}</button>
            <button onClick={() => setShowSettings(true)} className="btn-warn">{t("actions.settings")}</button>
            {status.state === "running" && (
              <button onClick={openSsh} className="btn-warn">{t("actions.ssh")}</button>
            )}
            <details style={{ marginTop: 8 }}>
              <summary style={{ cursor: "pointer", color: "#4a5680", fontSize: 11, padding: "4px 0" }}>
                {t("actions.advanced")}
              </summary>
              <div style={{ display: "flex", flexDirection: "column", gap: 4, marginTop: 6 }}>
                {status.state === "running" && (
                  <button onClick={stopVm} className="btn-warn">{t("vm.stop")}</button>
                )}
                <button onClick={forceKill} className="btn-warn">{t("recovery.kill")}</button>
                <button onClick={resetSession} className="btn-warn">{t("recovery.reset")}</button>
              </div>
            </details>
          </section>

          <section className="logs-section">
            <h3>{t("log.title")}</h3>
            <pre className="logs">{logs.slice(-30).join("\n")}</pre>
          </section>
        </aside>

        <main className="terminal-pane">
          <div className="tab-bar">
            {tabs.map(tab => (
              <div
                key={tab.id}
                className={`tab ${activeTab === tab.id ? "active" : ""}`}
                onClick={() => setActiveTab(tab.id)}
              >
                <span>{tab.title}</span>
                <button
                  className="tab-close"
                  onClick={(e) => { e.stopPropagation(); closeTab(tab.id); }}
                >×</button>
              </div>
            ))}
            {status.state === "running" && (
              <button className="tab-new" onClick={newTab} title="Нова вкладка (Ctrl+Shift+T)">+</button>
            )}
          </div>

          <div className="terminal-area">
            {tabs.length === 0 && status.state !== "running" && (
              <div className="welcome">
                <h1>WinMux Desktop</h1>
                <p>Натисни <b>Start VM</b> у бічній панелі.</p>
              </div>
            )}
            {tabs.length === 0 && status.state === "running" && (
              <div className="welcome">
                <p>Гість завантажений. Натисни <b>+</b> щоб відкрити термінал.</p>
              </div>
            )}
            {tabs.map(tab => tab.kind === "browser" ? (
              <div key={tab.id} style={{ display: activeTab === tab.id ? "flex" : "none", flexDirection: "column", height: "100%", background: "#0a0e27" }}>
                <div style={{ display: "flex", gap: 4, padding: 4, background: "#1a1b26", borderBottom: "1px solid #414868" }}>
                  <button onClick={() => { const f = document.getElementById(`bf-${tab.id}`) as HTMLIFrameElement | null; f?.contentWindow?.history.back(); }} title="Back" style={btnStyle}>←</button>
                  <button onClick={() => { const f = document.getElementById(`bf-${tab.id}`) as HTMLIFrameElement | null; f?.contentWindow?.history.forward(); }} title="Forward" style={btnStyle}>→</button>
                  <button onClick={() => { const f = document.getElementById(`bf-${tab.id}`) as HTMLIFrameElement | null; if (f) f.src = f.src; }} title="Reload" style={btnStyle}>⟳</button>
                  <input
                    defaultValue={tab.url}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        const url = (e.currentTarget.value.startsWith("http") ? e.currentTarget.value : `http://${e.currentTarget.value}`);
                        setTabs(p => p.map(t => t.id === tab.id ? { ...t, url, title: new URL(url).host } : t));
                      }
                    }}
                    style={{ flex: 1, background: "#0a0e27", color: "#e0e6f0", border: "1px solid #4a5680", borderRadius: 3, padding: "4px 8px", outline: "none", fontFamily: "monospace", fontSize: 12 }}
                  />
                  <button onClick={() => invoke("open_url", { url: tab.url })} title="Open in system browser" style={btnStyle}>↗</button>
                </div>
                <iframe id={`bf-${tab.id}`} src={tab.url} style={{ flex: 1, border: "none", background: "white" }} />
              </div>
            ) : (
              <div
                key={tab.id}
                className="tab-content"
                style={{
                  display: activeTab === tab.id ? "grid" : "none",
                  gridTemplateColumns: tab.layout === "v-split" ? "1fr 1fr" : "1fr",
                  gridTemplateRows: tab.layout === "h-split" ? "1fr 1fr" : "1fr",
                  gap: tab.layout === "single" ? 0 : 4,
                  height: "100%",
                  width: "100%",
                }}
              >
                {tab.panes.map(paneId => (
                  <div
                    key={paneId}
                    className={`pane ${tab.focusedPane === paneId ? "focused" : ""}`}
                    onClick={() => setTabs(p => p.map(t => t.id === tab.id ? { ...t, focusedPane: paneId } : t))}
                  >
                    {tab.panes.length > 1 && (
                      <button
                        className="pane-close"
                        onClick={(e) => { e.stopPropagation(); closePane(tab.id, paneId); }}
                        title="Close pane"
                      >×</button>
                    )}
                    <Terminal
                      tabId={paneId}
                      active={activeTab === tab.id && tab.focusedPane === paneId}
                      fontSize={fontSize}
                      theme={themes[themeName] || themes["winmux-dark"]}
                    />
                  </div>
                ))}
              </div>
            ))}
          </div>
        </main>
      </div>

      {showSettings && <Settings onClose={() => setShowSettings(false)} />}

      {showPalette && (() => {
        const cmds: Cmd[] = [
          { id: "tab.new", title: "New terminal tab", shortcut: "Ctrl+Shift+T", run: () => invoke("open_tab"),
            hint: "Open another bash session in a new tab" },
          ...(activeTab !== null ? [
            { id: "tab.close", title: "Close current tab", shortcut: "Ctrl+W",
              run: () => closeTab(activeTab) } as Cmd,
            { id: "pane.split.v", title: "Split pane vertically (right)",
              shortcut: "Ctrl+Shift+D", run: () => splitPane("v-split"),
              hint: "Add a second terminal side-by-side" } as Cmd,
            { id: "pane.split.h", title: "Split pane horizontally (down)",
              shortcut: "Ctrl+Shift+S", run: () => splitPane("h-split"),
              hint: "Add a second terminal stacked below" } as Cmd,
          ] : []),
          ...(status.state === "running"
            ? [
                { id: "vm.stop", title: "Stop VM", run: stopVm } as Cmd,
                { id: "vm.ssh", title: "Open SSH in PowerShell", run: openSsh } as Cmd,
              ]
            : [{ id: "vm.start", title: "Start VM", run: startVm } as Cmd]),
          { id: "vm.kill", title: "Force kill all WinMux/QEMU", run: forceKill, hint: "When buttons don't respond" },
          { id: "vm.reset", title: "Reset session", run: resetSession, hint: "Wipe overlay disk, fresh boot next time" },
          { id: "settings.open", title: "Open Settings", run: () => setShowSettings(true) },
          { id: "view.font.up", title: "Increase font size", shortcut: "Ctrl++", run: () => setFontSize(s => Math.min(32, s + 1)) },
          { id: "view.font.down", title: "Decrease font size", shortcut: "Ctrl+-", run: () => setFontSize(s => Math.max(8, s - 1)) },
          { id: "view.font.reset", title: "Reset font size to 14", run: () => setFontSize(14) },
          ...["winmux-dark","dracula","tokyo-night","solarized-dark","catppuccin-mocha","github-dark"].map(t => ({
            id: `theme.${t}`, title: `Switch theme: ${t}`, run: () => setThemeName(t)
          } as Cmd)),
          { id: "telemetry.toggle", title: "Toggle telemetry", run: async () => {
            const s = await invoke<{enabled:boolean}>("telemetry_status");
            await invoke("telemetry_set", { enabled: !s.enabled });
          } },
          { id: "update.check", title: "Check for updates",
            run: async () => {
              const r = await invoke<any>("check_update");
              if (r.available) {
                if (confirm(`New version ${r.available} available!\n\n${r.notes || ""}\n\nDownload and install now?`)) {
                  invoke("download_and_install_update", { url: r.download_url });
                }
              } else {
                alert(`You're on the latest version: ${r.current}`);
              }
            }
          },
          { id: "url.docs", title: "Open documentation", run: () => invoke("open_url", { url: "https://github.com/Denromvas/winmux/blob/main/docs/TZ.md" }) },
          { id: "url.github", title: "Open GitHub repo", run: () => invoke("open_url", { url: "https://github.com/Denromvas/winmux" }) },
          { id: "url.landing", title: "Open winmux website", run: () => invoke("open_url", { url: "https://github.com/Denromvas/winmux" }) },
          { id: "claude.auto", title: "Run claude in auto-mode (--dangerously-skip-permissions)",
            hint: "Opens new tab with auto-mode prompt — Claude executes everything without asking",
            run: async () => {
              const id = await invoke<number>("open_tab");
              setTimeout(() => {
                invoke("send_input", { tabId: id, data: 'claude --dangerously-skip-permissions ' });
              }, 1500);
            }
          },
          { id: "claude.normal", title: "Run claude (interactive)",
            run: async () => {
              const id = await invoke<number>("open_tab");
              setTimeout(() => {
                invoke("send_input", { tabId: id, data: 'claude\n' });
              }, 1500);
            }
          },
          { id: "claude.monitored", title: "🤖 Run claude with live AI sidebar",
            hint: "Pipes JSONL events to ~/.winmux/claude.jsonl — sidebar shows live activity",
            run: async () => {
              const id = await invoke<number>("open_tab");
              setTimeout(() => {
                invoke("send_input", { tabId: id, data: 'mkdir -p ~/.winmux && claude --dangerously-skip-permissions --output-format stream-json --verbose 2>&1 | tee ~/.winmux/claude.jsonl\n' });
              }, 1500);
            }
          },
          { id: "claude.tmux", title: "Run claude in tmux (survives disconnect)",
            run: async () => {
              const id = await invoke<number>("open_tab");
              setTimeout(() => {
                invoke("send_input", { tabId: id, data: 'tmux new -s claude-auto -d "claude --dangerously-skip-permissions" && tmux attach -t claude-auto\n' });
              }, 1500);
            }
          },
          { id: "guest.mount-help", title: "Show winmux-mount help (manual mount)",
            run: async () => {
              const id = await invoke<number>("open_tab");
              setTimeout(() => {
                invoke("send_input", { tabId: id, data: 'winmux-mount -h\n' });
              }, 1500);
            }
          },
          { id: "clipboard.image", title: "📋 Paste clipboard image to guest (Ctrl+Shift+V)",
            hint: "Saves Windows-clipboard image to host, injects /workspace path so Claude can attach it",
            run: async () => {
              try {
                const path = await invoke<string>("paste_image_to_guest");
                if (activeTab !== null) {
                  const tab = tabs.find(t => t.id === activeTab);
                  const paneId = tab?.focusedPane;
                  if (paneId !== undefined) await invoke("send_input", { tabId: paneId, data: path + " " });
                  alert(`✓ Image saved → ${path}\nNow press Enter in the terminal (path is already typed)`);
                }
              } catch (e) { alert(`${e}`); }
            }
          },
          { id: "browser.open", title: "🌐 Open URL in new browser tab",
            hint: "Embedded browser inside WinMux — useful for forwarded ports (3000, 8080, ...)",
            run: () => {
              const u = prompt("URL (or just localhost:port):", "http://127.0.0.1:3000");
              if (u) openBrowserTab(u);
            }
          },
          { id: "term.search", title: "🔍 Search in terminal scrollback",
            hint: "Same as Ctrl+F inside any terminal",
            run: () => {
              const ev = new KeyboardEvent("keydown", { key: "f", ctrlKey: true, bubbles: true });
              document.activeElement?.dispatchEvent(ev);
            }
          },
          { id: "snapshot.save", title: "📸 Save VM snapshot",
            hint: "Freeze current guest state — restore later if claude --auto breaks something",
            run: async () => {
              const name = prompt("Snapshot name (letters/digits/_, no spaces):", `snap-${new Date().toISOString().slice(0,16).replace(/[T:-]/g,'')}`);
              if (!name) return;
              try {
                const out = await invoke<string>("snapshot_save", { name });
                alert(`✓ ${out.trim() || `Saved '${name}'`}`);
              } catch (e) { alert(`Failed: ${e}`); }
            }
          },
          { id: "snapshot.restore", title: "⏪ Restore VM snapshot",
            hint: "Discards current state and reverts to a saved snapshot",
            run: async () => {
              try {
                const list = await invoke<string>("snapshot_list");
                const name = prompt(`Available snapshots:\n${list}\n\nName to restore:`);
                if (!name) return;
                if (!confirm(`Restore '${name}'? Current state and all open terminals will be lost.`)) return;
                const out = await invoke<string>("snapshot_restore", { name });
                alert(`✓ ${out.trim()}`);
              } catch (e) { alert(`Failed: ${e}`); }
            }
          },
          { id: "snapshot.list", title: "📋 List VM snapshots",
            run: async () => {
              try {
                const list = await invoke<string>("snapshot_list");
                alert(list || "No snapshots yet.");
              } catch (e) { alert(`Failed: ${e}`); }
            }
          },
          { id: "snapshot.delete", title: "🗑 Delete VM snapshot",
            run: async () => {
              try {
                const list = await invoke<string>("snapshot_list");
                const name = prompt(`Available snapshots:\n${list}\n\nName to delete:`);
                if (!name) return;
                const out = await invoke<string>("snapshot_delete", { name });
                alert(`✓ ${out.trim()}`);
              } catch (e) { alert(`Failed: ${e}`); }
            }
          },
        ];
        return <CommandPalette commands={cmds} onClose={() => setShowPalette(false)} />;
      })()}

      {telemetryAsk && (
        <div className="settings-overlay">
          <div className="settings-modal" style={{ maxWidth: 520 }}>
            <h2>Анонімна телеметрія</h2>
            <p style={{ color: "#c0caf5", fontSize: 13, lineHeight: 1.5 }}>
              Допомогти нам покращувати WinMux? Ми збираємо <b>тільки</b>:
              версію WinMux/Windows, тип CPU, теми/розмір шрифту,
              лічильники використання фіч (drop, port-forward тощо)
              і крах-репорти. <b>Жодних</b> файлів, шляхів, команд, IP, паролів.
            </p>
            <p style={{ color: "#4a5680", fontSize: 12 }}>
              Self-hosted: <code>telemetry.denromvas.website</code> · Open source.
            </p>
            <div style={{ display: "flex", gap: 8, justifyContent: "flex-end", marginTop: 16 }}>
              <button className="btn-warn" onClick={async () => {
                await invoke("telemetry_set", { enabled: false });
                setTelemetryAsk(false);
              }}>Не надсилати</button>
              <button className="btn-start" onClick={async () => {
                await invoke("telemetry_set", { enabled: true });
                setTelemetryAsk(false);
              }}>Згоден</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
