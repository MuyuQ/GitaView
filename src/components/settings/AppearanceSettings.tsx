import { useEffect, useState } from "react";
import { getSettings, saveSettings } from "../../lib/commands";
import { notifySettingsUpdated } from "../../lib/settingsEvents";
import type { AppSettings } from "../../types";

export function AppearanceSettings() {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [allowWidgetDrag, setAllowWidgetDrag] = useState(true);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    getSettings()
      .then((nextSettings) => {
        setSettings(nextSettings);
        setAllowWidgetDrag(nextSettings.appearance.allowWidgetDrag);
      })
      .catch((err) => setMessage(`加载外观设置失败：${err}`));
  }, []);

  async function handleSave() {
    if (!settings) return;
    setBusy(true);
    setMessage(null);
    try {
      const nextSettings = {
        ...settings,
        appearance: { allowWidgetDrag },
      };
      const savedSettings = await saveSettings(nextSettings);
      setSettings(savedSettings);
      notifySettingsUpdated(savedSettings);
      setMessage("外观设置已保存");
    } catch (err) {
      setMessage(`保存失败：${err}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="settings-card">
      <h3>外观</h3>
      <div className="settings-row">
        <label>
          <input
            type="checkbox"
            checked={allowWidgetDrag}
            onChange={(event) => setAllowWidgetDrag(event.target.checked)}
          />
          允许拖动收起浮窗
        </label>
      </div>
      <button className="settings-save" onClick={handleSave} disabled={busy || !settings}>保存外观设置</button>
      {message && <p className="settings-message" role="status">{message}</p>}
    </section>
  );
}
