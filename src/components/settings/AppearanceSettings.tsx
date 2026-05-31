import { useEffect, useState } from "react";
import { getSettings } from "../../lib/commands";
import { notifySettingsUpdated, subscribeToSettingsUpdates } from "../../lib/settingsEvents";
import { queueSettingsUpdate } from "../../lib/settingsMutations";
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
    return subscribeToSettingsUpdates(setSettings);
  }, []);

  async function handleSave() {
    if (!settings) return;
    setBusy(true);
    setMessage(null);
    try {
      const savedSettings = await queueSettingsUpdate((currentSettings) => ({
        ...currentSettings,
        appearance: { allowWidgetDrag },
      }));
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
