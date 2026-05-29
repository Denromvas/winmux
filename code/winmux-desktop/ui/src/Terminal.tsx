import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { WebglAddon } from "@xterm/addon-webgl";
import { CanvasAddon } from "@xterm/addon-canvas";
import { SearchAddon } from "@xterm/addon-search";
import "@xterm/xterm/css/xterm.css";

export interface TerminalProps {
  tabId: number;
  active: boolean;
  fontSize: number;
  theme: any;
  bottomPad: number;
}

export default function Terminal({ tabId, active, fontSize, theme, bottomPad }: TerminalProps) {
  const ref = useRef<HTMLDivElement>(null);
  const termRef = useRef<XTerm | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const searchRef = useRef<SearchAddon | null>(null);
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");

  useEffect(() => {
    if (!ref.current) return;
    const term = new XTerm({
      fontFamily: '"Cascadia Code", "JetBrains Mono", "Fira Code", "Consolas", monospace',
      fontSize,
      lineHeight: 1.0,
      letterSpacing: 0,
      cursorBlink: true,
      cursorStyle: "block",
      scrollback: 5000,
      allowProposedApi: true,
      smoothScrollDuration: 0,
      drawBoldTextInBrightColors: true,
      theme,
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.loadAddon(new WebLinksAddon((_e, url) => {
      invoke("open_url", { url }).catch(console.error);
    }));
    term.open(ref.current);
    // Canvas рендерер — стабільніший для TUI з box-drawing (Claude Code, btop, ranger).
    // WebGL швидший але часто дає артефакти/накладання символів.
    try { term.loadAddon(new CanvasAddon()); } catch {
      try {
        const webgl = new WebglAddon();
        webgl.onContextLoss(() => webgl.dispose());
        term.loadAddon(webgl);
      } catch {}
    }
    const search = new SearchAddon();
    term.loadAddon(search);
    fit.fit();
    termRef.current = term;
    fitRef.current = fit;
    searchRef.current = search;

    // Ctrl+F → toggle search bar (intercept before xterm)
    // Ctrl+Shift+V → if clipboard has image, save to host + inject guest path
    term.attachCustomKeyEventHandler((ev) => {
      if (ev.type === "keydown" && ev.ctrlKey && !ev.shiftKey && (ev.key === "f" || ev.key === "F")) {
        ev.preventDefault();
        setSearchOpen(prev => !prev);
        return false;
      }
      if (ev.type === "keydown" && ev.ctrlKey && ev.shiftKey && (ev.key === "V" || ev.key === "v")) {
        ev.preventDefault();
        invoke<string>("paste_image_to_guest").then(path => {
          // Inject the path into terminal stdin → Claude picks it up as attachment
          invoke("send_input", { tabId, data: path + " " });
        }).catch(err => {
          // Fall back: show notice in terminal
          term.write(`\r\n\x1b[33m[paste-image] ${err}\x1b[0m\r\n`);
        });
        return false;
      }
      if (ev.type === "keydown" && ev.key === "Escape" && searchOpen) {
        setSearchOpen(false);
        return false;
      }
      return true;
    });

    term.onData((data) => {
      invoke("send_input", { tabId, data }).catch(console.error);
    });
    term.onResize(({ cols, rows }) => {
      invoke("resize_term", { tabId, cols, rows }).catch(() => {});
    });

    const doFit = () => {
      requestAnimationFrame(() => {
        try {
          fit.fit();
          // Примусово синхронізувати PTY з реальним розміром xterm.
          // Без цього TUI (Claude Code) застряє в стартовому розмірі PTY і
          // малює UI у "вузькому полі", залишаючи нижні рядки порожніми.
          invoke("resize_term", { tabId, cols: term.cols, rows: term.rows }).catch(() => {});
          term.refresh(0, term.rows - 1);
        } catch {}
      });
    };
    window.addEventListener("resize", doFit);
    const ro = new ResizeObserver(doFit);
    if (ref.current) ro.observe(ref.current);
    const refits = [50, 200, 500, 1000].map(d => setTimeout(doFit, d));

    const unlisten = listen<string>(`term-output:${tabId}`, (e) => {
      term.write(e.payload);
    });

    return () => {
      window.removeEventListener("resize", doFit);
      ro.disconnect();
      refits.forEach(clearTimeout);
      unlisten.then(f => f());
      term.dispose();
    };
  }, [tabId, fontSize, theme]);

  useEffect(() => {
    if (active) {
      requestAnimationFrame(() => {
        try { fitRef.current?.fit(); } catch {}
        termRef.current?.focus();
      });
    }
  }, [active]);

  // Reserve empty space below the prompt: padding on the host element makes
  // FitAddon compute fewer rows, so the cursor floats above the window edge.
  useEffect(() => {
    const fit = fitRef.current;
    const term = termRef.current;
    if (!fit || !term) return;
    requestAnimationFrame(() => {
      try {
        fit.fit();
        invoke("resize_term", { tabId, cols: term.cols, rows: term.rows }).catch(() => {});
        term.scrollToBottom();
      } catch {}
    });
  }, [bottomPad, tabId]);

  const opts = { regex: false, wholeWord: false, caseSensitive: false,
    decorations: { matchBackground: "#ffd166", activeMatchBackground: "#ff6b6b",
      matchOverviewRuler: "#ffd166", activeMatchColorOverviewRuler: "#ff6b6b" } };
  const findNext = () => searchRef.current?.findNext(searchQuery, opts);
  const findPrev = () => searchRef.current?.findPrevious(searchQuery, opts);

  return (
    <div style={{ width: "100%", height: "100%", display: active ? "flex" : "none", flexDirection: "column", position: "relative" }}>
      <div ref={ref} style={{ flex: 1, minHeight: 0 }} />
      {/* Spacer below the terminal: physically shrinks the fit area so FitAddon
          computes fewer rows and the prompt floats above the window edge. */}
      <div style={{ height: bottomPad, flexShrink: 0 }} />
      {searchOpen && (
        <div style={{ position: "absolute", top: 4, right: 4, zIndex: 10,
          background: "#1a1b26", border: "1px solid #414868", borderRadius: 4,
          padding: 4, display: "flex", gap: 4, alignItems: "center", fontSize: 12 }}>
          <input
            autoFocus
            value={searchQuery}
            onChange={(e) => { setSearchQuery(e.target.value); searchRef.current?.findNext(e.target.value, opts); }}
            onKeyDown={(e) => {
              if (e.key === "Enter") (e.shiftKey ? findPrev() : findNext());
              if (e.key === "Escape") { setSearchOpen(false); searchRef.current?.clearDecorations(); }
            }}
            placeholder="Find in scrollback…"
            style={{ background: "#0a0e27", color: "#e0e6f0", border: "1px solid #4a5680", borderRadius: 3, padding: "3px 6px", outline: "none", width: 180 }}
          />
          <button onClick={findPrev} style={{ padding: "2px 8px", background: "#414868", color: "#e0e6f0", border: "none", borderRadius: 3, cursor: "pointer" }}>↑</button>
          <button onClick={findNext} style={{ padding: "2px 8px", background: "#414868", color: "#e0e6f0", border: "none", borderRadius: 3, cursor: "pointer" }}>↓</button>
          <button onClick={() => { setSearchOpen(false); searchRef.current?.clearDecorations(); }} style={{ padding: "2px 8px", background: "#3a3a4a", color: "#e0e6f0", border: "none", borderRadius: 3, cursor: "pointer" }}>✕</button>
        </div>
      )}
    </div>
  );
}
