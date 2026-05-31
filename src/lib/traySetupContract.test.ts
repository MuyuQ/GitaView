import { describe, expect, it } from "vitest";
import { readProjectFileCompact } from "./sourceContract";

describe("native tray setup contract", () => {
  it("sets a visible icon and a right-click menu with handlers", () => {
    const lib = readProjectFileCompact("src-tauri/src/lib.rs");

    expect(lib).toContain("include_image!(\"./icons/icon.png\")");
    expect(lib).toContain("TrayIconBuilder::with_id(tray_status::MAIN_TRAY_ID)");
    expect(lib).toContain(".icon(");
    expect(lib).toContain(".menu(&tray_menu)");
    expect(lib).toContain(".show_menu_on_left_click(false)");
    expect(lib).toContain(".on_menu_event(");
  });

  it("reapplies the native desktop widget layer during startup", () => {
    const lib = readProjectFileCompact("src-tauri/src/lib.rs");

    expect(lib).toContain("desktop_widget::reapply_desktop_widget_layer(app.handle())");
  });

  it("reserves tray generations before repository status collection begins", () => {
    const commands = readProjectFileCompact("src-tauri/src/app_commands.rs");

    expect(commands).toMatch(/begin_tray_menu_update\(\)[\s\S]*spawn_blocking[\s\S]*set_status_menu_if_current/);
  });
});
