import { describe, expect, it } from "vitest";
import { readProjectFile, readProjectFileCompact } from "./sourceContract";

describe("cross-platform desktop hardening contract", () => {
  it("routes frame updates through the native desktop widget boundary", () => {
    const hook = readProjectFileCompact("src/lib/useWidgetView.ts");
    const commands = readProjectFileCompact("src/lib/commands.ts");
    const rustCommands = readProjectFileCompact("src-tauri/src/app_commands.rs");
    const desktopWidget = readProjectFileCompact("src-tauri/src/desktop_widget/mod.rs");

    expect(hook).toContain("syncDesktopWidgetFrame");
    expect(commands).toContain('invoke<void>("sync_desktop_widget_frame"');
    expect(rustCommands).toContain("pub async fn sync_desktop_widget_frame(");
    expect(rustCommands).toContain("crate::desktop_widget::sync_desktop_widget_frame(");
    expect(desktopWidget).toContain("pub fn sync_desktop_widget_frame(");
  });

  it("converts Windows screen coordinates and applies child styles before parenting", () => {
    const windowsWidget = readProjectFileCompact("src-tauri/src/desktop_widget/windows.rs");

    expect(windowsWidget).toContain("ScreenToClient");
    expect(windowsWidget).toContain("GetParent");
    expect(windowsWidget).toMatch(/adjust_window_styles\(gitaview_hwnd\)\?;[\s\S]*SetParent\(gitaview_hwnd/);
  });

  it("rolls back partial Windows desktop attachment failures", () => {
    const windowsWidget = readProjectFileCompact("src-tauri/src/desktop_widget/windows.rs");

    expect(windowsWidget).toContain("struct WindowAttachmentSnapshot");
    expect(windowsWidget).toContain("capture_window_attachment(");
    expect(windowsWidget).toContain("rollback_window_attachment(");
    expect(windowsWidget).toContain("SWP_FRAMECHANGED");
  });

  it("starts Windows host recovery and reapplies the layer when showing the tray window", () => {
    const lib = readProjectFileCompact("src-tauri/src/lib.rs");
    const desktopWidget = readProjectFileCompact("src-tauri/src/desktop_widget/mod.rs");
    const windowsWidget = readProjectFileCompact("src-tauri/src/desktop_widget/windows.rs");

    expect(lib).toContain("desktop_widget::start_desktop_widget_watchdog(app.handle().clone())");
    expect(lib).toMatch(/TRAY_SHOW_ID[\s\S]*desktop_widget::reapply_desktop_widget_layer\(app\)/);
    expect(desktopWidget).toContain("pub fn start_desktop_widget_watchdog(");
    expect(windowsWidget).toContain("parent_matches_desktop_host(");
    expect(windowsWidget).toMatch(/ensure_desktop_widget_layer[\s\S]*find_desktop_icon_host\(progman\)\?/);
  });

  it("uses macOS accessory activation and a template left-click menu-bar icon", () => {
    const lib = readProjectFileCompact("src-tauri/src/lib.rs");

    expect(lib).toContain("set_activation_policy(tauri::ActivationPolicy::Accessory)");
    expect(lib).toContain('include_image!("./icons/tray-template.png")');
    expect(lib).toContain(".icon_as_template(true)");
    expect(lib).toContain(".show_menu_on_left_click(true)");
    expect(lib).toContain(".show_menu_on_left_click(false)");
  });

  it("hides every Windows Git child process", () => {
    const gitCommands = readProjectFileCompact("src-tauri/src/git/commands.rs");

    expect(gitCommands).toContain("fn configure_git_child_process(command: &mut Command)");
    expect(gitCommands).toContain("command.creation_flags(CREATE_NO_WINDOW);");
    expect(gitCommands).toMatch(/configure_git_child_process\(&mut command\);[\s\S]*run_command_with_timeout/);
  });

  it("keeps the Tauri content security policy free of inline style exceptions", () => {
    const tauriConfig = readProjectFile("src-tauri/tauri.conf.json");

    expect(tauriConfig).toContain("style-src 'self'");
    expect(tauriConfig).not.toContain("'unsafe-inline'");
  });

  it("documents manual desktop-widget acceptance for every supported desktop platform", () => {
    const checklist = readProjectFile("docs/platform-acceptance-checklist.md");

    expect(checklist).toContain("Windows");
    expect(checklist).toContain("macOS Apple Silicon");
    expect(checklist).toContain("macOS Intel");
    expect(checklist).toContain("Explorer restart");
    expect(checklist).toContain("Mission Control");
    expect(checklist).toContain("v0.3.1-unsigned");
  });
});

describe("cross-platform CI and release trust contract", () => {
  it("checks every supported target before merge", () => {
    const ci = readProjectFile(".github/workflows/ci.yml");

    expect(ci).toContain("pull_request:");
    expect(ci).not.toContain("macos-latest");
    expect(ci).not.toContain("windows-latest");
    expect(ci).toContain("platform: macos-15");
    expect(ci).toContain("platform: macos-15-intel");
    expect(ci).toContain("platform: windows-2025");
    expect(ci).toContain("cache: npm");
    expect(ci).toContain("cache-dependency-path: package-lock.json");
    expect(ci).toContain("x86_64-pc-windows-msvc");
    expect(ci).toContain("aarch64-apple-darwin");
    expect(ci).toContain("x86_64-apple-darwin");
    expect(ci).toContain("cargo check --manifest-path src-tauri/Cargo.toml --target ${{ matrix.target }}");
  });

  it("blocks unsigned releases and configures platform signing", () => {
    const release = readProjectFile(".github/workflows/release.yml");
    const signingValidator = readProjectFile("scripts/validate-release-signing.cjs");
    const windowsSigning = readProjectFile("scripts/configure-windows-signing.ps1");

    expect(release).not.toContain("macos-latest");
    expect(release).not.toContain("windows-latest");
    expect(release).toContain("platform: 'macos-15'");
    expect(release).toContain("platform: 'macos-15-intel'");
    expect(release).toContain("platform: 'windows-2025'");
    expect(release).toContain("cache: npm");
    expect(release).toContain("cache-dependency-path: package-lock.json");
    expect(release).toContain("node scripts/validate-release-signing.cjs");
    expect(release).toContain("scripts/configure-windows-signing.ps1");
    expect(release).toContain("APPLE_CERTIFICATE:");
    expect(release).toContain("WINDOWS_CERTIFICATE:");
    expect(release).toContain("cargo check --manifest-path src-tauri/Cargo.toml --target ${{ matrix.target }}");
    expect(signingValidator).toContain("WINDOWS_CERTIFICATE_THUMBPRINT");
    expect(signingValidator).toContain("APPLE_SIGNING_IDENTITY");
    expect(windowsSigning).toContain("Import-PfxCertificate");
    expect(windowsSigning).toContain("certificateThumbprint");
  });

  it("allows only explicit unsigned tags to bypass signing as draft prereleases", () => {
    const release = readProjectFile(".github/workflows/release.yml");
    const releaseVersion = readProjectFile("scripts/validate-release-version.cjs");
    const unsignedBuild = release.match(
      /- name: Build unsigned Tauri app([\s\S]*?)- name: Verify Windows installer signatures/,
    )?.[1];

    expect(releaseVersion).toContain("const unsignedTag = `${expectedTag}-unsigned`;");
    expect(releaseVersion).toContain("fs.appendFileSync(process.env.GITHUB_OUTPUT, `unsigned=${unsigned}\\n`);");
    expect(release).toContain("id: release_mode");
    expect(release).toContain("if: steps.release_mode.outputs.unsigned != 'true'");
    expect(release).toContain("if: startsWith(matrix.platform, 'windows') && steps.release_mode.outputs.unsigned != 'true'");
    expect(release).toContain("prerelease: ${{ steps.release_mode.outputs.unsigned == 'true' }}");
    expect(release).toContain("Unsigned test build");
    expect(unsignedBuild).toBeDefined();
    expect(unsignedBuild).not.toContain("APPLE_CERTIFICATE:");
  });
});
