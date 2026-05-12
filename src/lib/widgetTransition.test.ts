import { describe, expect, it } from "vitest";
import {
  getTransitionMode,
  getTransitionWindowTarget,
  isWidgetTransitionView,
  transitionDurations,
} from "./widgetTransition";

describe("widget transition state helpers", () => {
  it("keeps native window resize out of the visible closing animation", () => {
    expect(getTransitionWindowTarget("opening")).toBe("expanded");
    expect(getTransitionWindowTarget("closing")).toBe("expanded");
    expect(getTransitionWindowTarget("collapsed")).toBe("collapsed");
  });

  it("maps transient render states to lightweight ghost modes", () => {
    expect(getTransitionMode("opening")).toBe("open");
    expect(getTransitionMode("closing")).toBe("close");
    expect(getTransitionMode("expanded")).toBeNull();
  });

  it("keeps widget morphs intentionally short", () => {
    expect(isWidgetTransitionView("opening")).toBe(true);
    expect(isWidgetTransitionView("settings")).toBe(false);
    expect(transitionDurations.openingMs).toBeLessThanOrEqual(60);
    expect(transitionDurations.closingMs).toBeLessThanOrEqual(60);
  });
});
