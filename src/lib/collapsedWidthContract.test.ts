import { describe, expect, it } from "vitest";
import { readProjectFile, readProjectJson } from "./sourceContract";

describe("collapsed width contract", () => {
  it("aligns native and CSS collapsed surfaces at 312px", () => {
    const hook = readProjectFile("src/lib/useWidgetView.ts");
    const styles = readProjectFile("src/styles/widget.css");
    const tauri = readProjectJson<{ app: { windows: Array<{ width: number }> } }>("src-tauri/tauri.conf.json");

    expect(tauri.app.windows[0].width).toBe(312);
    expect(hook).toContain("collapsed: { width: 312, height: 40 }");
    expect(styles).toMatch(/\.collapsed-widget\s*{[^}]*width:\s*312px;/);
    expect(styles).not.toContain("transition-capsule");
  });
});
