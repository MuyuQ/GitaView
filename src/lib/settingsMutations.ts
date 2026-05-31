import { getSettings, saveSettings } from "./commands";
import type { AppSettings } from "../types";

type SettingsPatch = (settings: AppSettings) => AppSettings;
type LoadSettings = () => Promise<AppSettings>;
type SaveSettings = (settings: AppSettings) => Promise<AppSettings>;

export function createSettingsUpdateQueue(loadSettings: LoadSettings, persistSettings: SaveSettings) {
  let pending: Promise<void> = Promise.resolve();

  return (patch: SettingsPatch): Promise<AppSettings> => {
    const update = pending.then(async () => persistSettings(patch(await loadSettings())));
    pending = update.then(() => undefined, () => undefined);
    return update;
  };
}

export const queueSettingsUpdate = createSettingsUpdateQueue(getSettings, saveSettings);
