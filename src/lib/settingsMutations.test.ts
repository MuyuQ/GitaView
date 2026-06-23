import { describe, expect, it } from "vitest";
import { createSettingsUpdateQueue } from "./settingsMutations";
import type { AppSettings } from "../types";

const initialSettings: AppSettings = {
  version: 1,
  repos: [],
  groups: [{ name: "全部分组", repoIds: [] }],
  defaultGroup: "全部分组",
  refresh: { lightweightRefreshEnabled: true, intervalMinutes: 5 },
  safety: { confirmPull: true, confirmPush: true },
  appearance: { allowWidgetDrag: true },
};

describe("createSettingsUpdateQueue", () => {
  it("serializes patches against the latest persisted settings", async () => {
    let persisted = structuredClone(initialSettings);
    const updateSettings = createSettingsUpdateQueue(
      async () => structuredClone(persisted),
      async (nextSettings) => {
        await Promise.resolve();
        persisted = structuredClone(nextSettings);
        return structuredClone(persisted);
      },
    );

    await Promise.all([
      updateSettings((settings) => ({
        ...settings,
        refresh: { ...settings.refresh, intervalMinutes: 15 },
      })),
      updateSettings((settings) => ({
        ...settings,
        appearance: { allowWidgetDrag: false },
      })),
    ]);

    expect(persisted.refresh.intervalMinutes).toBe(15);
    expect(persisted.appearance.allowWidgetDrag).toBe(false);
  });
});
