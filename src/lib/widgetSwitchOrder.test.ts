import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const projectRoot = resolve(import.meta.dirname, "../..");

describe("widget view switch ordering", () => {
  it("commits the React view before resizing the native window", () => {
    const hook = readFileSync(resolve(projectRoot, "src/lib/useWidgetView.ts"), "utf8");
    const showView = hook.match(/const showView = useCallback[\s\S]*?\[syncNativeWindowFrame\]\);/);

    expect(hook).toContain('import { flushSync } from "react-dom";');
    expect(showView?.[0]).toMatch(/flushSync\(\(\) => \{[\s\S]*setView\(nextView\);[\s\S]*setWindowView\(nextView\);[\s\S]*\}\);[\s\S]*syncNativeWindowFrame\(nextView\);/);
  });

  it("uses a native resize background guard before exposing transparent window area", () => {
    const hook = readFileSync(resolve(projectRoot, "src/lib/useWidgetView.ts"), "utf8");
    const syncNativeWindowFrame = hook.match(/const syncNativeWindowFrame = useCallback[\s\S]*?\[\]\);/);

    expect(hook).toContain("const resizeGuardBackground");
    expect(hook).toContain("const transparentWindowBackground");
    expect(syncNativeWindowFrame?.[0]).toMatch(/setBackgroundColor\(resizeGuardBackground\)[\s\S]*syncDesktopWidgetFrame\(/);
    expect(syncNativeWindowFrame?.[0]).toMatch(/setTimeout\([\s\S]*setBackgroundColor\(transparentWindowBackground\)/);
  });

  it("grants the native background color permission used by the resize guard", () => {
    const capability = readFileSync(resolve(projectRoot, "src-tauri/capabilities/default.json"), "utf8");

    expect(capability).toContain('"core:window:allow-set-background-color"');
  });

  it("lets the native desktop layer own z-order instead of forcing settings always-on-top", () => {
    const hook = readFileSync(resolve(projectRoot, "src/lib/useWidgetView.ts"), "utf8");
    const capability = readFileSync(resolve(projectRoot, "src-tauri/capabilities/default.json"), "utf8");

    expect(hook).not.toContain("setAlwaysOnTop");
    expect(capability).not.toContain('"core:window:allow-set-always-on-top"');
  });
});
