import { describe, expect, it } from "vitest";
import { getTransitionWindowTarget } from "./widgetTransition";

describe("widget transition state helpers", () => {
  it("keeps widget views stable and removes transient render states", () => {
    expect(getTransitionWindowTarget("expanded")).toBe("expanded");
    expect(getTransitionWindowTarget("settings")).toBe("settings");
  });
});
