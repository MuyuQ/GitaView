import { describe, expect, it } from "vitest";
import { readProjectFile } from "./sourceContract";

describe("desktop widget product wording contract", () => {
  it("describes the current implementation as a floating widget unless native desktop layer is wired in", () => {
    const readme = readProjectFile("README.md");
    const setupContract = readProjectFile("src/lib/traySetupContract.test.ts");
    const desktopModule = readProjectFile("src-tauri/src/desktop_widget/mod.rs");

    expect(setupContract).toContain("not.toContain(\"reapply_desktop_widget_layer\")");
    expect(readme).toContain("浮动 Widget");
    expect(readme).not.toContain("常驻桌面");
    expect(desktopModule).toContain("实验性");
  });
});
