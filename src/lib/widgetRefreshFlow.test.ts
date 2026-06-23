import { describe, expect, it } from "vitest";
import {
  summarizeCollapsed,
  sortRepos,
  filterRepos,
  toCollapsedBucket,
  shouldShowSettingsView,
  buildGroupOptions,
} from "./statusModel";
import { resolveRefreshCompletion } from "./refreshGeneration";
import { getTransitionWindowTarget } from "./widgetTransition";
import type { RepoStatus, RemoteRelation } from "../types";

const sampleRepo = (
  name: string,
  relation: RemoteRelation,
  group = "全部分组",
): RepoStatus => ({
  id: name,
  name,
  path: `/repos/${name}`,
  group,
  branch: "main",
  relation,
  changeLabel: relation === "synced" ? "✓" : relation === "diverged" ? "⇕ 3" : "↑ 2",
  hint: "",
  hasRemote: relation !== "no_remote",
  remoteUrl: relation !== "no_remote" ? `https://github.com/example/${name}` : null,
});

describe("widgetRefreshFlow (integration)", () => {
  it("full refresh pipeline: repos → collapsed summary → expand → filters → settings", () => {
    const repos: RepoStatus[] = [
      sampleRepo("app", "synced", "产品"),
      sampleRepo("api", "local_ahead", "业务"),
      sampleRepo("worker", "remote_ahead", "业务"),
      sampleRepo("admin", "diverged", "基础设施"),
      sampleRepo("archive", "no_remote", "归档"),
      sampleRepo("failed", "error", "产品"),
    ];

    const sorted = sortRepos(repos);
    expect(sorted.map((r) => r.name)).toEqual([
      "failed",
      "admin",
      "worker",
      "api",
      "app",
      "archive",
    ]);

    const summary = summarizeCollapsed(repos);
    expect(summary).toEqual([
      { bucket: "synced", count: 1 },
      { bucket: "syncable", count: 2 },
      { bucket: "needs_attention", count: 2 },
      { bucket: "no_remote", count: 1 },
    ]);

    const groups = buildGroupOptions(repos);
    expect(groups.map((g) => g.name)).toEqual([
      "全部分组",
      "产品",
      "业务",
      "基础设施",
      "归档",
    ]);

    const businessRepos = filterRepos(repos, "业务", "all");
    expect(businessRepos.map((r) => r.name)).toEqual(["worker", "api"]);

    const syncableOnly = filterRepos(repos, "全部分组", "all", "");
    expect(syncableOnly.map((r) => r.name)).toEqual([
      "failed",
      "admin",
      "worker",
      "api",
      "app",
      "archive",
    ]);

    const searched = filterRepos(repos, "全部分组", "all", "worker");
    expect(searched.map((r) => r.name)).toEqual(["worker"]);

    const emptySearch = filterRepos(repos, "全部分组", "all", "不存在");
    expect(emptySearch).toHaveLength(0);

    expect(shouldShowSettingsView("collapsed", 0, null, false)).toBe(true);
    expect(shouldShowSettingsView("collapsed", 5, null, false)).toBe(false);
  });

  it("refresh generation resolves stale vs current correctly", () => {
    expect(resolveRefreshCompletion(1, 1)).toBe(true);
    expect(resolveRefreshCompletion(1, 2)).toBe(false);
    expect(resolveRefreshCompletion(5, 5)).toBe(true);
    expect(resolveRefreshCompletion(5, 3)).toBe(false);
  });

  it("widget transition maps render view to stable view", () => {
    expect(getTransitionWindowTarget("collapsed")).toBe("collapsed");
    expect(getTransitionWindowTarget("expanded")).toBe("expanded");
    expect(getTransitionWindowTarget("settings")).toBe("settings");
  });

  it("toCollapsedBucket groups all relations correctly", () => {
    const relations: [RemoteRelation, ReturnType<typeof toCollapsedBucket>][] = [
      ["synced", "synced"],
      ["local_ahead", "syncable"],
      ["remote_ahead", "syncable"],
      ["diverged", "needs_attention"],
      ["error", "needs_attention"],
      ["no_remote", "no_remote"],
    ];

    for (const [relation, expected] of relations) {
      expect(toCollapsedBucket(relation)).toBe(expected);
    }
  });
});
