import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const projectRoot = resolve(import.meta.dirname, "../..");

describe("widget view switch ordering", () => {
  it("commits the React view before resizing the native window", () => {
    const app = readFileSync(resolve(projectRoot, "src/App.tsx"), "utf8");
    const showView = app.match(/function showView[\s\S]*?\n  }/);

    expect(app).toContain('import { flushSync } from "react-dom";');
    expect(showView?.[0]).toMatch(/flushSync\(\(\) => \{[\s\S]*setView\(nextView\);[\s\S]*setWindowView\(nextView\);[\s\S]*\}\);[\s\S]*syncNativeWindowFrame\(nextView\);/);
  });

  it("uses a native resize background guard before exposing transparent window area", () => {
    const app = readFileSync(resolve(projectRoot, "src/App.tsx"), "utf8");
    const syncNativeWindowFrame = app.match(/const syncNativeWindowFrame[\s\S]*?\n  }, \[\]\);/);

    expect(app).toContain("const resizeGuardBackground");
    expect(app).toContain("const transparentWindowBackground");
    expect(syncNativeWindowFrame?.[0]).toMatch(/setBackgroundColor\(resizeGuardBackground\)[\s\S]*setSize\(targetLogicalSize\)/);
    expect(syncNativeWindowFrame?.[0]).toMatch(/setTimeout\([\s\S]*setBackgroundColor\(transparentWindowBackground\)/);
  });

  it("grants the native background color permission used by the resize guard", () => {
    const capability = readFileSync(resolve(projectRoot, "src-tauri/capabilities/default.json"), "utf8");

    expect(capability).toContain('"core:window:allow-set-background-color"');
  });
});
