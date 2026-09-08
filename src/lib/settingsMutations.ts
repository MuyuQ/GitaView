import { getSettings, saveSettings } from "./commands";
import type { AppSettings } from "../types";

type SettingsPatch = (settings: AppSettings) => AppSettings;
type LoadSettings = () => Promise<AppSettings>;
type SaveSettings = (settings: AppSettings, expected: AppSettings) => Promise<AppSettings>;

export function createSettingsUpdateQueue(loadSettings: LoadSettings, persistSettings: SaveSettings) {
  let pending: Promise<void> = Promise.resolve();

  return (patch: SettingsPatch): Promise<AppSettings> => {
    const update = pending.then(async () => {
      for (let attempt = 0; ; attempt++) {
        const expected = await loadSettings();
        try {
          return await persistSettings(patch(structuredClone(expected)), expected);
        } catch (error) {
          if (String(error) !== "SETTINGS_CONFLICT" || attempt >= 3) throw error;
        }
      }
    });
    pending = update.then(() => undefined, () => undefined);
    return update;
  };
}

export const queueSettingsUpdate = createSettingsUpdateQueue(getSettings, saveSettings);
