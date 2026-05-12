import { describe, expect, it } from "vitest";
import { interpolateWindowSize, resolveAnchoredWindowPosition } from "./windowMotion";

describe("interpolateWindowSize", () => {
  it("starts and ends at the requested sizes", () => {
    const from = { width: 218, height: 44 };
    const to = { width: 720, height: 560 };

    expect(interpolateWindowSize(from, to, 0)).toEqual(from);
    expect(interpolateWindowSize(from, to, 1)).toEqual(to);
  });

  it("eases toward the target size", () => {
    expect(interpolateWindowSize({ width: 100, height: 100 }, { width: 200, height: 300 }, 0.5)).toEqual({
      width: 188,
      height: 275,
    });
  });
});

describe("resolveAnchoredWindowPosition", () => {
  const workArea = { x: 0, y: 0, width: 1920, height: 1040 };

  it("keeps the right edge anchored when expanding from the right edge", () => {
    const current = { x: 1702, y: 80, width: 218, height: 40 };
    const target = { width: 720, height: 560 };

    expect(resolveAnchoredWindowPosition(current, target, workArea)).toEqual({ x: 1200, y: 80 });
  });

  it("keeps the current top-left position when the target fits", () => {
    const current = { x: 320, y: 120, width: 218, height: 40 };
    const target = { width: 720, height: 560 };

    expect(resolveAnchoredWindowPosition(current, target, workArea)).toEqual({ x: 320, y: 120 });
  });

  it("keeps the bottom edge anchored when expanding from the bottom edge", () => {
    const current = { x: 80, y: 1000, width: 218, height: 40 };
    const target = { width: 720, height: 560 };

    expect(resolveAnchoredWindowPosition(current, target, workArea)).toEqual({ x: 80, y: 480 });
  });

  it("keeps a collapsed target on the right edge when shrinking from an expanded right edge", () => {
    const current = { x: 1200, y: 80, width: 720, height: 560 };
    const target = { width: 218, height: 40 };

    expect(resolveAnchoredWindowPosition(current, target, workArea)).toEqual({ x: 1702, y: 80 });
  });

  it("clamps correctly on a negative-coordinate monitor work area", () => {
    const negativeWorkArea = { x: -1920, y: 0, width: 1920, height: 1040 };
    const current = { x: -218, y: 1000, width: 218, height: 40 };
    const target = { width: 720, height: 560 };

    expect(resolveAnchoredWindowPosition(current, target, negativeWorkArea)).toEqual({ x: -720, y: 480 });
  });
});
