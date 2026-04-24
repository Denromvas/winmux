import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getLang, setLang } from "./i18n";

interface Cfg {
  ram: string;
  smp: number;
  accel: string;
  ssh_port: number;
  qmp_port: number;
  agent_port: number;
}

export default function Settings({ onClose }: { onClose: () => void }) {
  const [cfg, setCfg] = useState<Cfg | null>(null);
  const [error, setError] = useState<string>("");
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    invoke<Cfg>("read_settings").then(setCfg).catch((e) => setError(String(e)));
  }, []);

  if (!cfg) {
    return (
      <div className="settings-overlay">
        <div className="settings-modal">
          <h2>Settings</h2>
          {error ? <p style={{ color: "#ff6b6b" }}>{error}</p> : <p>Loading...</p>}
          <button onClick={onClose}>Close</button>
        </div>
      </div>
    );
  }

  const save = async () => {
    try {
      await invoke("write_settings", { cfg });
      setSaved(true);
      setTimeout(() => setSaved(false), 2500);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="settings-overlay" onClick={onClose}>
      <div className="settings-modal" onClick={(e) => e.stopPropagation()}>
        <h2>Settings</h2>
        <p className="hint">Зміни вступлять у силу після наступного Stop+Start VM.</p>

        <div className="form-row">
          <label>RAM (e.g. 1G, 2G, 4G)</label>
          <input value={cfg.ram} onChange={(e) => setCfg({ ...cfg, ram: e.target.value })} />
        </div>

        <div className="form-row">
          <label>vCPUs (1–16)</label>
          <input
            type="number" min={1} max={16}
            value={cfg.smp}
            onChange={(e) => setCfg({ ...cfg, smp: parseInt(e.target.value) || 4 })}
          />
        </div>

        <div className="form-row">
          <label>Accelerator</label>
          <select value={cfg.accel} onChange={(e) => setCfg({ ...cfg, accel: e.target.value })}>
            <option value="tcg">TCG (software, working everywhere)</option>
            <option value="auto">Auto (WHPX with TCG fallback)</option>
            <option value="whpx">WHPX (Hyper-V Platform, fastest)</option>
          </select>
        </div>

        <div className="form-row">
          <label>SSH port (host)</label>
          <input
            type="number" min={1024} max={65535}
            value={cfg.ssh_port}
            onChange={(e) => setCfg({ ...cfg, ssh_port: parseInt(e.target.value) || 2223 })}
          />
        </div>

        <div className="form-row">
          <label>QMP port</label>
          <input
            type="number" min={1024} max={65535}
            value={cfg.qmp_port}
            onChange={(e) => setCfg({ ...cfg, qmp_port: parseInt(e.target.value) || 4444 })}
          />
        </div>

        <div className="form-row">
          <label>Agent port</label>
          <input
            type="number" min={1024} max={65535}
            value={cfg.agent_port}
            onChange={(e) => setCfg({ ...cfg, agent_port: parseInt(e.target.value) || 4445 })}
          />
        </div>

        <div className="form-row">
          <label>UI language / Мова</label>
          <select value={getLang()} onChange={(e) => setLang(e.target.value as any)}>
            <option value="uk">Українська</option>
            <option value="en">English</option>
          </select>
        </div>

        {error && <p style={{ color: "#ff6b6b" }}>{error}</p>}
        {saved && <p style={{ color: "#00ff88" }}>✓ Saved. Restart VM to apply.</p>}

        <div className="form-row" style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
          <button onClick={onClose} className="btn-warn">Cancel</button>
          <button onClick={save} className="btn-start">Save</button>
        </div>
      </div>
    </div>
  );
}
