import { describe, expect, it } from "vitest";
import { createSettingsUpdateQueue } from "./settingsMutations";
import type { AppSettings } from "../types";

const initialSettings: AppSettings = {
  version: 1,
  repos: [],
  groups: [{ name: "全部分组", repoIds: [] }],
  defaultGroup: "全部分组",
  refresh: { lightweightRefreshEnabled: true, intervalMinutes: 5 },
  appearance: { allowWidgetDrag: true },
};

describe("createSettingsUpdateQueue", () => {
  it("reapplies a patch after a concurrent repository addition", async () => {
    let persisted = structuredClone(initialSettings);
    let saves = 0;
    const update = createSettingsUpdateQueue(
      async () => structuredClone(persisted),
      async (next, expected) => {
        if (++saves === 1) {
          persisted.repos.push({ id: "new", name: "new", path: "/new", group: "全部分组" });
        }
        if (JSON.stringify(expected) !== JSON.stringify(persisted)) throw "SETTINGS_CONFLICT";
        persisted = next;
        return structuredClone(persisted);
      },
    );
    await update((settings) => ({ ...settings, appearance: { allowWidgetDrag: false } }));
    expect(persisted.repos.map((repo) => repo.id)).toEqual(["new"]);
    expect(persisted.appearance.allowWidgetDrag).toBe(false);
    expect(saves).toBe(2);
  });

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
