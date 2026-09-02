import { describe, expect, it } from "vitest";
import { readProjectFileCompact } from "./sourceContract";

describe("desktop widget product wording contract", () => {
  it("matches the wired-in native desktop layer: README claims a resident desktop widget", () => {
    const readme = readProjectFileCompact("README.md");
    const lib = readProjectFileCompact("src-tauri/src/lib.rs");

    expect(readme).toContain("常驻桌面");
    expect(lib).toContain("desktop_widget::reapply_desktop_widget_layer(app.handle())");
    expect(lib).toContain("desktop_widget::start_desktop_widget_watchdog(app.handle().clone())");
  });
});
