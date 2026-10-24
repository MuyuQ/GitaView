import { describe, expect, it } from "vitest";
import { readProjectFile } from "./sourceContract";

describe("WidgetKit reload bridge", () => {
  it("uses the Swift WidgetCenter API with the Rust C symbol", () => {
    const bridge = readProjectFile("src-tauri/widget-bridge/WidgetReloader.swift");
    expect(bridge).toContain('@_cdecl("reload_widget_timelines")');
    expect(bridge).toContain("WidgetCenter.shared.reloadAllTimelines()");
    const build = readProjectFile("src-tauri/build.rs");
    expect(build).toContain('"OUT_DIR"');
    expect(build).toContain('"TARGET"');
    expect(build).toContain('"swiftc"');
  });

  it("requests a fallback timeline when a reload notification is missed", () => {
    const provider = readProjectFile("src-tauri/widget-extension/GitaViewWidget/Provider.swift");
    expect(provider).not.toContain("policy: .never");
    expect(provider).toContain("policy: .after(");
  });
});
