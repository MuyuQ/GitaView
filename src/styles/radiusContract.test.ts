import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const projectRoot = resolve(import.meta.dirname, "../..");

function readStyle(path: string) {
  return readFileSync(resolve(projectRoot, path), "utf8");
}

function declarationBlock(css: string, selector: string) {
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return css.match(new RegExp(`${escapedSelector}\\s*{[^}]*}`))?.[0] ?? "";
}

describe("window radius style contract", () => {
  it("uses transparent outer chrome and one shared radius token for visible windows", () => {
    const tokens = readStyle("src/styles/tokens.css");
    const widget = readStyle("src/styles/widget.css");
    const settings = readStyle("src/styles/settings.css");

    expect(tokens).toContain("--gv-window-radius: 18px;");
    expect(tokens).toMatch(/body\s*{[^}]*background:\s*transparent;/);
    expect(widget).toMatch(/\.expanded-widget\s*{[\s\S]*border-radius:\s*var\(--gv-window-radius\)/);
    expect(settings).toMatch(/\.settings-window\s*{[\s\S]*border-radius:\s*var\(--gv-window-radius\)/);
  });

  it("does not cast a detached shadow below expanded widget surfaces", () => {
    const widget = readStyle("src/styles/widget.css");

    expect(widget).toMatch(/\.expanded-widget\s*{[^}]*box-shadow:\s*none;/);
  });

  it("does not cast clipped shadows around collapsed widget surfaces", () => {
    const widget = readStyle("src/styles/widget.css");

    expect(widget).toMatch(/\.collapsed-widget\s*{[^}]*box-shadow:\s*none;/);
  });

  it("keeps expanded content fully painted when switching from collapsed view", () => {
    const widget = readStyle("src/styles/widget.css");

    expect(widget).not.toMatch(/\.filter-row\s*{[^}]*animation:/);
    expect(widget).not.toMatch(/\.repo-results\s*{[^}]*animation:/);
    expect(widget).not.toMatch(/\.repo-row\s*{[^}]*animation:/);
  });

  it("keeps expanded widget surfaces the same size as the native window", () => {
    const widget = readStyle("src/styles/widget.css");
    const expandedWidget = declarationBlock(widget, ".expanded-widget");

    expect(expandedWidget).toContain("width: 100vw;");
    expect(expandedWidget).toContain("height: 100vh;");
    expect(expandedWidget).toContain("max-width: none;");
    expect(expandedWidget).toContain("max-height: none;");
  });

  it("prevents browser scrollbars while the native window is being resized", () => {
    const tokens = readStyle("src/styles/tokens.css");
    const rootBlock = tokens.match(/html,\s*body,\s*#root\s*{[^}]*}/)?.[0] ?? "";

    expect(rootBlock).toContain("height: 100%;");
    expect(rootBlock).toContain("overflow: hidden;");
  });

  it("keeps the settings window background neutral instead of using a green glow", () => {
    const settings = readStyle("src/styles/settings.css");
    const settingsWindow = declarationBlock(settings, ".settings-window");

    expect(settingsWindow).not.toContain("radial-gradient");
    expect(settingsWindow).not.toContain("47, 159, 103");
  });
});
