import { describe, expect, it, vi } from "vitest";
import { notifySettingsUpdated, subscribeToSettingsUpdates } from "./settingsEvents";
import type { AppSettings } from "../types";

const settings: AppSettings = {
  repos: [],
  groups: [{ name: "全部分组", repoIds: [] }],
  defaultGroup: "全部分组",
  refresh: { lightweightRefreshEnabled: true, intervalMinutes: 5 },
  safety: { confirmPull: true, confirmPush: true },
  appearance: { compactMode: false, allowWidgetDrag: true },
};

describe("settings update events", () => {
  it("notifies subscribers with updated settings", () => {
    const target = new EventTarget();
    const handler = vi.fn();
    const unsubscribe = subscribeToSettingsUpdates(handler, target);

    notifySettingsUpdated(settings, target);

    expect(handler).toHaveBeenCalledWith(settings);
    unsubscribe();
  });

  it("stops notifying after unsubscribe", () => {
    const target = new EventTarget();
    const handler = vi.fn();
    const unsubscribe = subscribeToSettingsUpdates(handler, target);
    unsubscribe();

    notifySettingsUpdated(settings, target);

    expect(handler).not.toHaveBeenCalled();
  });
});
