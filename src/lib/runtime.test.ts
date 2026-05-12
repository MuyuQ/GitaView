import { describe, expect, it } from "vitest";
import { hasTauriRuntime } from "./runtime";

describe("hasTauriRuntime", () => {
  it("returns false when the browser window has no Tauri internals", () => {
    expect(hasTauriRuntime({ window: {} })).toBe(false);
  });

  it("returns true when Tauri internals are present", () => {
    expect(hasTauriRuntime({ window: { __TAURI_INTERNALS__: {} } })).toBe(true);
  });
});
