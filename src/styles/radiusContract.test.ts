import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const projectRoot = resolve(import.meta.dirname, "../..");

function readStyle(path: string) {
  return readFileSync(resolve(projectRoot, path), "utf8");
}

describe("window radius style contract", () => {
  it("uses transparent outer chrome and one shared radius token for visible windows", () => {
    const tokens = readStyle("src/styles/tokens.css");
    const widget = readStyle("src/styles/widget.css");
    const settings = readStyle("src/styles/settings.css");

    expect(tokens).toContain("--gv-window-radius: 18px;");
    expect(tokens).toMatch(/body\s*{[^}]*background:\s*transparent;/);
    expect(widget).toMatch(/\.expanded-widget\s*{[\s\S]*border-radius:\s*var\(--gv-window-radius\)/);
    expect(widget).toMatch(/\.transition-surface\s*{[\s\S]*border-radius:\s*var\(--gv-window-radius\)/);
    expect(settings).toMatch(/\.settings-window\s*{[\s\S]*border-radius:\s*var\(--gv-window-radius\)/);
  });

  it("does not cast a detached shadow below expanded widget surfaces", () => {
    const widget = readStyle("src/styles/widget.css");

    expect(widget).toMatch(/\.expanded-widget\s*{[^}]*box-shadow:\s*none;/);
    expect(widget).toMatch(/\.transition-surface\s*{[^}]*box-shadow:\s*none;/);
  });

  it("does not cast clipped shadows around collapsed widget surfaces", () => {
    const widget = readStyle("src/styles/widget.css");

    expect(widget).toMatch(/\.collapsed-widget\s*{[^}]*box-shadow:\s*none;/);
    expect(widget).toMatch(/\.transition-capsule\s*{[^}]*box-shadow:\s*none;/);
  });

  it("does not render a blank expanded surface during transient widget states", () => {
    const widget = readStyle("src/styles/widget.css");

    expect(widget).toMatch(/\.transition-surface\s*{[^}]*display:\s*none;/);
  });

  it("keeps the transient collapsed capsule visible instead of opacity-animating it", () => {
    const widget = readStyle("src/styles/widget.css");

    expect(widget).not.toMatch(/\.transition-capsule--(?:open|close)\s*{[^}]*animation:/);
    expect(widget).not.toContain("@keyframes capsule-open");
    expect(widget).not.toContain("@keyframes capsule-close");
  });
});
