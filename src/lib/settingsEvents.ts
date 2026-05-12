import type { AppSettings } from "../types";

export const settingsUpdatedEventName = "gitaview:settings-updated";

type SettingsEventTarget = Pick<EventTarget, "addEventListener" | "removeEventListener" | "dispatchEvent">;

function getDefaultTarget(): SettingsEventTarget {
  return window;
}

function createSettingsEvent(settings: AppSettings): Event {
  const event = new Event(settingsUpdatedEventName);
  Object.defineProperty(event, "detail", { value: settings });
  return event;
}

export function notifySettingsUpdated(settings: AppSettings, target = getDefaultTarget()): void {
  target.dispatchEvent(createSettingsEvent(settings));
}

export function subscribeToSettingsUpdates(
  handler: (settings: AppSettings) => void,
  target = getDefaultTarget(),
): () => void {
  const listener = (event: Event) => {
    handler((event as CustomEvent<AppSettings>).detail);
  };
  target.addEventListener(settingsUpdatedEventName, listener);
  return () => target.removeEventListener(settingsUpdatedEventName, listener);
}
