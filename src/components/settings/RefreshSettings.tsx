import { useEffect, useState } from "react";
import { getSettings, saveSettings } from "../../lib/commands";
import type { AppSettings } from "../../types";

export function RefreshSettings() {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [enabled, setEnabled] = useState(true);
  const [intervalMinutes, setIntervalMinutes] = useState(5);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    getSettings()
      .then((nextSettings) => {
        setSettings(nextSettings);
        setEnabled(nextSettings.refresh.lightweightRefreshEnabled);
        setIntervalMinutes(nextSettings.refresh.intervalMinutes);
      })
      .catch((err) => setMessage(`加载刷新设置失败：${err}`));
  }, []);

  async function handleSave() {
    if (!settings) return;
    const safeInterval = Math.min(Math.max(intervalMinutes, 1), 60);
    setBusy(true);
    setMessage(null);
    try {
      const nextSettings = {
        ...settings,
        refresh: {
          lightweightRefreshEnabled: enabled,
          intervalMinutes: safeInterval,
        },
      };
      await saveSettings(nextSettings);
      setSettings(nextSettings);
      setIntervalMinutes(safeInterval);
      setMessage("刷新设置已保存");
    } catch (err) {
      setMessage(`保存失败：${err}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="settings-card">
      <h3>刷新设置</h3>
      <div className="settings-row">
        <label>
          <input type="checkbox" checked={enabled} onChange={(event) => setEnabled(event.target.checked)} /> 启用轻量定时刷新
        </label>
      </div>
      <div className="settings-row">
        <label>刷新间隔（分钟）</label>
        <input
          type="number"
          value={intervalMinutes}
          min={1}
          max={60}
          onChange={(event) => setIntervalMinutes(Number(event.target.value))}
        />
      </div>
      <button className="settings-save" onClick={handleSave} disabled={busy || !settings}>保存刷新设置</button>
      {message && <p className="settings-message" role="status">{message}</p>}
    </section>
  );
}
