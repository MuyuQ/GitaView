import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { getSettings, listRepoStatuses, saveSettings } from "./commands";

const projectRoot = resolve(__dirname, "../..");

describe("review follow-up contracts", () => {
  it("uses portable preview fixture paths outside the Tauri runtime", async () => {
    const settings = await getSettings();
    const statuses = await listRepoStatuses();
    const paths = [
      ...settings.repos.map((repo) => repo.path),
      ...statuses.map((repo) => repo.path),
    ];

    expect(paths).not.toHaveLength(0);
    expect(paths.every((path) => path.startsWith("~/projects/"))).toBe(true);
    expect(paths.every((path) => !path.includes("E:/Git_Repositories"))).toBe(true);
  });

  it("logs app settings and exit failures instead of swallowing them silently", () => {
    const app = readFileSync(resolve(projectRoot, "src/App.tsx"), "utf8");
    const hook = readFileSync(resolve(projectRoot, "src/lib/useWidgetView.ts"), "utf8");
    const repoActions = readFileSync(resolve(projectRoot, "src/components/RepoActions.tsx"), "utf8");

    expect(hook).not.toContain(".catch(() => {})");
    expect(hook).toContain("console.error(\"加载设置失败\"");
    expect(hook).toContain("console.error(\"退出应用失败\"");
    expect(app).not.toContain(".catch(() => {})");
    expect(repoActions).not.toContain("getSettings");
  });

  it("guards lightweight refreshes from overlapping repository scans", () => {
    const hook = readFileSync(resolve(projectRoot, "src/lib/useWidgetView.ts"), "utf8");

    expect(hook).toContain("refreshInFlightRef");
    expect(hook).toContain("if (refreshInFlightRef.current) return;");
    expect(hook).toContain("refreshInFlightRef.current = true;");
    expect(hook).toContain("refreshInFlightRef.current = false;");
  });

  it("splits repository settings busy state by operation area", () => {
    const repositorySettings = readFileSync(
      resolve(projectRoot, "src/components/settings/RepositorySettings.tsx"),
      "utf8",
    );

    expect(repositorySettings).not.toContain("const [busy, setBusy]");
    expect(repositorySettings).not.toContain("disabled={busy}");
    expect(repositorySettings).toContain("scanBusy");
    expect(repositorySettings).toContain("addBusy");
    expect(repositorySettings).toContain("repoActionBusyId");
  });

  it("documents error as an app read-failure state rather than a sixth Git relation", () => {
    const readme = readFileSync(resolve(projectRoot, "README.md"), "utf8");

    expect(readme).toContain("error");
    expect(readme).toContain("读取失败");
    expect(readme).toContain("应用层状态");
  });

  it("persists settings changes in browser preview mode", async () => {
    const settings = await getSettings();
    const original = settings.refresh.intervalMinutes;
    const updated = original === 60 ? 59 : original + 1;

    await saveSettings({
      ...settings,
      refresh: { ...settings.refresh, intervalMinutes: updated },
    });

    expect((await getSettings()).refresh.intervalMinutes).toBe(updated);

    await saveSettings(settings);
  });
});
