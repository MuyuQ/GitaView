import { describe, expect, it } from "vitest";
import { readProjectFile, readProjectFileCompact } from "./sourceContract";

describe("tray status menu contract", () => {
  it("uses a stable tray id, keeps right-click behavior, and refreshes after setup", () => {
    const lib = readProjectFileCompact("src-tauri/src/lib.rs");

    expect(lib).toContain("TrayIconBuilder::with_id(tray_status::MAIN_TRAY_ID)");
    expect(lib).toContain(".show_menu_on_left_click(false)");
    expect(lib).toContain("tray_status::loading_tray_menu(app)?");
    expect(lib).toContain("tray_status::refresh_tray_menu_async(app.handle().clone())");
    expect(lib).toContain("tray_status::TRAY_REFRESH_ID");
  });

  it("builds display rows as disabled menu items and updates the existing tray", () => {
    const trayStatus = readProjectFileCompact("src-tauri/src/tray_status.rs");
    const trayRows = readProjectFile("src-tauri/src/tray_menu_rows.rs");

    expect(trayStatus).toContain("MenuItem::with_id(");
    expect(trayStatus).toMatch(/false,\s*None::<&str>/);
    expect(trayStatus).toContain("tray_by_id(MAIN_TRAY_ID)");
    expect(trayStatus).toContain("tray.set_menu(Some(menu))");
    expect(trayRows).toContain("正在读取仓库状态...");
  });

  it("shares collected statuses with the tray menu after list_repo_statuses", () => {
    const commands = readProjectFileCompact("src-tauri/src/app_commands.rs");

    expect(commands).toContain("crate::repo_status::collect_repo_statuses");
    expect(commands).toContain(
      "crate::tray_status::set_status_menu_if_current(&app, tray_generation, &statuses)",
    );
  });

  it("keeps tray status rendering platform neutral", () => {
    const trayStatus = readProjectFile("src-tauri/src/tray_status.rs");

    expect(trayStatus).not.toContain("target_os");
    expect(trayStatus).not.toContain("Command::new");
    expect(trayStatus).not.toContain("explorer");
    expect(trayStatus).not.toContain("xdg-open");
  });

  it("keeps pure tray row formatting separate from Tauri runtime menu updates", () => {
    const lib = readProjectFileCompact("src-tauri/src/lib.rs");
    const trayStatus = readProjectFileCompact("src-tauri/src/tray_status.rs");
    const trayRows = readProjectFile("src-tauri/src/tray_menu_rows.rs");

    expect(lib).toContain("pub mod tray_menu_rows;");
    expect(trayStatus).toContain("crate::tray_menu_rows");
    expect(trayStatus).toContain("pub use crate::tray_menu_rows::{");
    expect(trayRows).toContain("正在读取仓库状态...");
    expect(trayRows).toContain("GitaView · 共");
  });
});
