import { describe, expect, it } from "vitest";
import {
  getTransitionMode,
  getTransitionWindowTarget,
  isWidgetTransitionView,
  transitionDurations,
} from "./widgetTransition";

describe("widget transition state helpers", () => {
  it("does not route user toggles through a visible transient window target", () => {
    expect(getTransitionWindowTarget("opening")).toBe("collapsed");
    expect(getTransitionWindowTarget("closing")).toBe("collapsed");
    expect(getTransitionWindowTarget("expanded")).toBe("expanded");
  });

  it("keeps transient render states invisible to avoid switch ghosts", () => {
    expect(getTransitionMode("opening")).toBeNull();
    expect(getTransitionMode("closing")).toBeNull();
    expect(getTransitionMode("expanded")).toBeNull();
  });

  it("keeps widget toggles instant at the app-state layer", () => {
    expect(isWidgetTransitionView("opening")).toBe(false);
    expect(isWidgetTransitionView("closing")).toBe(false);
    expect(isWidgetTransitionView("settings")).toBe(false);
    expect(transitionDurations.openingMs).toBe(0);
    expect(transitionDurations.closingMs).toBe(0);
  });
});
