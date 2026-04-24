import { useEffect, useRef, useState } from "react";

export interface Cmd {
  id: string;
  title: string;
  hint?: string;
  shortcut?: string;
  run: () => void | Promise<void>;
}

interface Props {
  commands: Cmd[];
  onClose: () => void;
}

export default function CommandPalette({ commands, onClose }: Props) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
    const onEsc = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", onEsc);
    return () => window.removeEventListener("keydown", onEsc);
  }, []);

  // Дуже простий fuzzy: перевіряє чи всі літери query йдуть в title по порядку.
  const score = (text: string, q: string): number => {
    if (!q) return 1;
    const t = text.toLowerCase(); const Q = q.toLowerCase();
    let i = 0, hits = 0, lastPos = -10;
    for (const ch of t) {
      if (i < Q.length && ch === Q[i]) {
        hits += (lastPos + 1 === i) ? 3 : 1;  // bonus за послідовні
        i++; lastPos = i - 1;
      }
    }
    return i === Q.length ? hits : 0;
  };

  const filtered = commands
    .map(c => ({ c, s: score(c.title + " " + (c.hint || ""), query) }))
    .filter(x => x.s > 0)
    .sort((a, b) => b.s - a.s)
    .map(x => x.c);

  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") { setSelected(s => Math.min(filtered.length - 1, s + 1)); e.preventDefault(); }
    else if (e.key === "ArrowUp") { setSelected(s => Math.max(0, s - 1)); e.preventDefault(); }
    else if (e.key === "Enter") {
      const cmd = filtered[selected];
      if (cmd) { onClose(); setTimeout(() => cmd.run(), 0); }
      e.preventDefault();
    }
  };

  return (
    <div className="settings-overlay" onClick={onClose}>
      <div
        className="settings-modal"
        style={{ minWidth: 520, padding: 0, overflow: "hidden" }}
        onClick={(e) => e.stopPropagation()}
      >
        <input
          ref={inputRef}
          value={query}
          onChange={(e) => { setQuery(e.target.value); setSelected(0); }}
          onKeyDown={onKey}
          placeholder="Type a command..."
          style={{
            width: "100%",
            padding: "16px 20px",
            background: "transparent",
            border: "none",
            borderBottom: "1px solid #1a1f3a",
            color: "#e0e6f0",
            fontSize: 15,
            fontFamily: "inherit",
            outline: "none",
          }}
        />
        <div style={{ maxHeight: 400, overflowY: "auto" }}>
          {filtered.length === 0 ? (
            <div style={{ padding: "24px 20px", color: "#4a5680", textAlign: "center", fontSize: 13 }}>
              No commands match.
            </div>
          ) : filtered.map((cmd, idx) => (
            <div
              key={cmd.id}
              onClick={() => { onClose(); setTimeout(() => cmd.run(), 0); }}
              onMouseEnter={() => setSelected(idx)}
              style={{
                display: "flex",
                alignItems: "center",
                padding: "10px 20px",
                cursor: "pointer",
                background: idx === selected ? "#1a1f3a" : "transparent",
                borderLeft: idx === selected ? "3px solid #00ff88" : "3px solid transparent",
              }}
            >
              <div style={{ flex: 1 }}>
                <div style={{ color: idx === selected ? "#00ff88" : "#e0e6f0", fontSize: 14 }}>{cmd.title}</div>
                {cmd.hint && <div style={{ color: "#4a5680", fontSize: 12, marginTop: 2 }}>{cmd.hint}</div>}
              </div>
              {cmd.shortcut && (
                <span style={{ color: "#4a5680", fontFamily: "monospace", fontSize: 11 }}>{cmd.shortcut}</span>
              )}
            </div>
          ))}
        </div>
        <div style={{ padding: "8px 20px", borderTop: "1px solid #1a1f3a", color: "#4a5680", fontSize: 11 }}>
          ↑↓ navigate · Enter run · Esc close
        </div>
      </div>
    </div>
  );
}
