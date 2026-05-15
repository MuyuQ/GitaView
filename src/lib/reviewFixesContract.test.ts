import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { getSettings, listRepoStatuses } from "./commands";

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

  it("logs settings and exit failures instead of swallowing them silently", () => {
    const app = readFileSync(resolve(projectRoot, "src/App.tsx"), "utf8");
    const repoActions = readFileSync(resolve(projectRoot, "src/components/RepoActions.tsx"), "utf8");

    expect(app).not.toContain(".catch(() => {})");
    expect(app).toContain("console.error(\"加载设置失败\"");
    expect(app).toContain("console.error(\"退出应用失败\"");
    expect(repoActions).toContain("console.error(\"加载安全操作设置失败\"");
  });

  it("documents error as an app read-failure state rather than a sixth Git relation", () => {
    const readme = readFileSync(resolve(projectRoot, "README.md"), "utf8");

    expect(readme).toContain("error");
    expect(readme).toContain("读取失败");
    expect(readme).toContain("应用层状态");
  });
});
