import { describe, expect, it } from "vitest";
import {
  shouldPromoteCollapsedDrag,
  shouldStartCollapsedDrag,
  shouldStartExpandedDrag,
  shouldStartWindowDrag,
} from "./windowDrag";

describe("shouldStartWindowDrag", () => {
  it("allows dragging from plain layout regions", () => {
    const region = { closest: () => null } as unknown as EventTarget;

    expect(shouldStartWindowDrag(region)).toBe(true);
  });

  it("does not start dragging from interactive controls", () => {
    const button = { closest: () => ({ tagName: "BUTTON" }) } as unknown as EventTarget;
    const input = { closest: () => ({ tagName: "INPUT" }) } as unknown as EventTarget;

    expect(shouldStartWindowDrag(button)).toBe(false);
    expect(shouldStartWindowDrag(input)).toBe(false);
  });
});

describe("shouldStartCollapsedDrag", () => {
  it("starts only for enabled primary-button drags", () => {
    expect(shouldStartCollapsedDrag(true, 0)).toBe(true);
    expect(shouldStartCollapsedDrag(true, 2)).toBe(false);
    expect(shouldStartCollapsedDrag(false, 0)).toBe(false);
  });
});

describe("shouldStartExpandedDrag", () => {
  it("starts from non-interactive expanded regions when widget dragging is enabled", () => {
    const toolbar = { closest: () => null } as unknown as EventTarget;

    expect(shouldStartExpandedDrag(true, 0, toolbar)).toBe(true);
  });

  it("does not start from controls, secondary buttons, or when disabled", () => {
    const button = { closest: () => ({ tagName: "BUTTON" }) } as unknown as EventTarget;
    const toolbar = { closest: () => null } as unknown as EventTarget;

    expect(shouldStartExpandedDrag(true, 0, button)).toBe(false);
    expect(shouldStartExpandedDrag(true, 2, toolbar)).toBe(false);
    expect(shouldStartExpandedDrag(false, 0, toolbar)).toBe(false);
  });
});

describe("shouldPromoteCollapsedDrag", () => {
  it("promotes a press to drag only after the pointer moves beyond threshold", () => {
    expect(shouldPromoteCollapsedDrag(true, { x: 10, y: 10 }, { x: 12, y: 11 })).toBe(false);
    expect(shouldPromoteCollapsedDrag(true, { x: 10, y: 10 }, { x: 14, y: 10 })).toBe(true);
  });

  it("does not promote when dragging is disabled", () => {
    expect(shouldPromoteCollapsedDrag(false, { x: 10, y: 10 }, { x: 30, y: 30 })).toBe(false);
  });
});
