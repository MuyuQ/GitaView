import { useEffect, useState } from "react";
import { getSettings, saveSettings } from "../../lib/commands";
import type { AppSettings } from "../../types";

export function SafetySettings() {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [confirmPull, setConfirmPull] = useState(true);
  const [confirmPush, setConfirmPush] = useState(true);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    getSettings()
      .then((nextSettings) => {
        setSettings(nextSettings);
        setConfirmPull(nextSettings.safety.confirmPull);
        setConfirmPush(nextSettings.safety.confirmPush);
      })
      .catch((err) => setMessage(`加载安全设置失败：${err}`));
  }, []);

  async function handleSave() {
    if (!settings) return;
    setBusy(true);
    setMessage(null);
    try {
      const nextSettings = {
        ...settings,
        safety: { confirmPull, confirmPush },
      };
      await saveSettings(nextSettings);
      setSettings(nextSettings);
      setMessage("安全设置已保存");
    } catch (err) {
      setMessage(`保存失败：${err}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="settings-card">
      <h3>安全操作</h3>
      <div className="settings-row">
        <label>
          <input type="checkbox" checked={confirmPull} onChange={(event) => setConfirmPull(event.target.checked)} /> Pull 操作需要确认
        </label>
      </div>
      <div className="settings-row">
        <label>
          <input type="checkbox" checked={confirmPush} onChange={(event) => setConfirmPush(event.target.checked)} /> Push 操作需要确认
        </label>
      </div>
      <button className="settings-save" onClick={handleSave} disabled={busy || !settings}>保存安全设置</button>
      {message && <p className="settings-message" role="status">{message}</p>}
    </section>
  );
}
