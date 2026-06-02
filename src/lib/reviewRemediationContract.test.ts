import { describe, expect, it } from "vitest";
import { readProjectFile, readProjectJson } from "./sourceContract";

describe("review remediation contracts", () => {
  it("keeps mounted settings cards subscribed and labels the sidebar action truthfully", () => {
    const refresh = readProjectFile("src/components/settings/RefreshSettings.tsx");
    const appearance = readProjectFile("src/components/settings/AppearanceSettings.tsx");
    const safety = readProjectFile("src/components/settings/SafetySettings.tsx");
    const shell = readProjectFile("src/components/settings/SettingsShell.tsx");

    expect(refresh).toContain("subscribeToSettingsUpdates");
    expect(appearance).toContain("subscribeToSettingsUpdates");
    expect(safety).not.toContain('type="checkbox"');
    expect(shell).toContain(">关闭</button>");
  });

  it("saves settings cards through serialized latest-state patches", () => {
    const refresh = readProjectFile("src/components/settings/RefreshSettings.tsx");
    const appearance = readProjectFile("src/components/settings/AppearanceSettings.tsx");
    const repositories = readProjectFile("src/components/settings/RepositorySettings.tsx");
    const groups = readProjectFile("src/components/settings/GroupSettings.tsx");

    expect(refresh).toContain("queueSettingsUpdate");
    expect(appearance).toContain("queueSettingsUpdate");
    expect(repositories).toContain("queueSettingsUpdate");
    expect(groups).toContain("queueSettingsUpdate");
  });

  it("always presents Pull and Push as explicit confirmations", () => {
    const actions = readProjectFile("src/components/RepoActions.tsx");

    expect(actions).not.toContain("requiresPullConfirm");
    expect(actions).not.toContain("requiresPushConfirm");
    expect(actions).toContain("confirmPull");
    expect(actions).toContain("confirmPush");
  });

  it("waits for initial loading before applying empty-state window sizing", () => {
    const app = readProjectFile("src/App.tsx");

    expect(app).toContain("if (initialLoading) return;");
    expect(app).toContain("resolveRefreshCompletion");
  });

  it("aligns release versions and executes quality gates before packaging", () => {
    const packageJson = readProjectJson<{ version: string }>("package.json");
    const tauriConfig = readProjectJson<{ version: string }>("src-tauri/tauri.conf.json");
    const cargo = readProjectFile("src-tauri/Cargo.toml");
    const workflow = readProjectFile(".github/workflows/release.yml");
    const releaseVersionScript = readProjectFile("scripts/validate-release-version.cjs");

    expect(packageJson.version).toBe("0.3.1");
    expect(tauriConfig.version).toBe("0.3.1");
    expect(cargo).toContain('version = "0.3.1"');
    expect(workflow).toContain("npm test");
    expect(workflow).toContain("cargo test --manifest-path src-tauri/Cargo.toml");
    expect(workflow).toContain("cargo fmt --manifest-path src-tauri/Cargo.toml -- --check");
    expect(workflow).toContain("cargo clippy --manifest-path src-tauri/Cargo.toml --target ${{ matrix.target }} --all-targets -- -D warnings");
    expect(workflow).toContain("Validate release version");
    expect(workflow).toContain("node scripts/validate-release-version.cjs");
    expect(releaseVersionScript).toContain("GITHUB_REF_NAME");
  });

  it("exposes collapsed status names and removes unstable hover movement", () => {
    const collapsed = readProjectFile("src/components/WidgetCollapsed.tsx");
    const transition = readProjectFile("src/components/TransitionSurface.tsx");
    const styles = readProjectFile("src/styles/widget.css");
    const index = readProjectFile("index.html");

    expect(collapsed).toContain("summarizeActionableCollapsed");
    expect(collapsed).toContain('className="repo-word">仓库</span>');
    expect(collapsed).toContain("{item.label}");
    expect(collapsed).not.toContain("summaryShortLabel");
    expect(styles).not.toContain(".summary-short-label");
    expect(transition).toContain("summarizeActionableCollapsed");
    expect(transition).toContain('className="repo-word">仓库</span>');
    expect(transition).toContain("{item.label}");
    expect(styles).not.toContain("var(--gv-blue)");
    expect(styles).not.toContain("translateY(-1px)");
    expect(styles).not.toContain("translateX(2px)");
    expect(index).not.toContain("/vite.svg");
  });

  it("associates accessible names with settings form controls", () => {
    const refresh = readProjectFile("src/components/settings/RefreshSettings.tsx");
    const repositories = readProjectFile("src/components/settings/RepositorySettings.tsx");
    const groups = readProjectFile("src/components/settings/GroupSettings.tsx");

    expect(refresh).toContain('htmlFor="refresh-interval-minutes"');
    expect(refresh).toContain('id="refresh-interval-minutes"');
    expect(repositories).toContain('aria-label={`${repo.name} 的仓库分组`}');
    expect(repositories).toContain('aria-label="仓库目录"');
    expect(groups).toContain('aria-label="新分组名称"');
  });

  it("redacts diagnostic filesystem paths at call sites", () => {
    const appSettings = readProjectFile("src-tauri/src/app_settings.rs");
    const appCommands = readProjectFile("src-tauri/src/app_commands.rs");
    const repoStatus = readProjectFile("src-tauri/src/repo_status.rs");
    const lib = readProjectFile("src-tauri/src/lib.rs");

    expect(appSettings).not.toContain("path.display()");
    expect(appCommands).toContain("diagnostics::redact_path");
    expect(repoStatus).toContain("diagnostics::redact_path");
    expect(lib).toContain("diagnostics::redact_path");
  });

  it("documents the v1 Push and origin-only remote contract consistently", () => {
    const readme = readProjectFile("README.md");
    const spec = readProjectFile("DESIGN_AND_BUILD_SPEC.md");

    expect(readme).toContain("Push");
    expect(readme).not.toContain("v1 不支持 Push");
    expect(readme).toContain("origin");
    expect(spec).not.toContain("Do not implement `push` in v1.");
    expect(spec).toContain("`push_repo`");
    expect(spec).toContain("`origin`");
  });

  it("broadcasts repository additions and removals to mounted settings cards", () => {
    const repositories = readProjectFile("src/components/settings/RepositorySettings.tsx");

    expect(repositories.match(/const nextSettings = await reload\(\);/g) ?? []).toHaveLength(2);
    expect(repositories.match(/notifySettingsUpdated\(nextSettings\);/g) ?? []).toHaveLength(2);
  });
});
