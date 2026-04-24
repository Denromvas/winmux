import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { WebglAddon } from "@xterm/addon-webgl";
import { CanvasAddon } from "@xterm/addon-canvas";
import "@xterm/xterm/css/xterm.css";

export interface TerminalProps {
  tabId: number;
  active: boolean;
  fontSize: number;
  theme: any;
}

export default function Terminal({ tabId, active, fontSize, theme }: TerminalProps) {
  const ref = useRef<HTMLDivElement>(null);
  const termRef = useRef<XTerm | null>(null);
  const fitRef = useRef<FitAddon | null>(null);

  useEffect(() => {
    if (!ref.current) return;
    const term = new XTerm({
      fontFamily: '"Cascadia Code", "JetBrains Mono", "Fira Code", "Consolas", monospace',
      fontSize,
      lineHeight: 1.2,
      cursorBlink: true,
      cursorStyle: "block",
      scrollback: 5000,
      allowProposedApi: true,
      theme,
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.loadAddon(new WebLinksAddon((_e, url) => {
      invoke("open_url", { url }).catch(console.error);
    }));
    term.open(ref.current);
    try {
      const webgl = new WebglAddon();
      webgl.onContextLoss(() => webgl.dispose());
      term.loadAddon(webgl);
    } catch {
      try { term.loadAddon(new CanvasAddon()); } catch {}
    }
    fit.fit();
    termRef.current = term;
    fitRef.current = fit;

    term.onData((data) => {
      invoke("send_input", { tabId, data }).catch(console.error);
    });
    term.onResize(({ cols, rows }) => {
      invoke("resize_term", { tabId, cols, rows }).catch(() => {});
    });

    const doFit = () => {
      requestAnimationFrame(() => { try { fit.fit(); } catch {} });
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

  return (
    <div
      ref={ref}
      style={{
        width: "100%",
        height: "100%",
        display: active ? "block" : "none",
      }}
    />
  );
}
