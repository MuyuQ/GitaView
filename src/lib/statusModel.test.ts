import { describe, expect, it } from "vitest";
import {
  buildGroupOptions,
  getRepoActionAvailability,
  shouldShowSettingsView,
  summarizeCollapsed,
  sortRepos,
  filterRepos,
  reconcileRelationFilter,
  toCollapsedBucket,
} from "./statusModel";
import type { RepoStatus } from "../types";

const base = (name: string, relation: RepoStatus["relation"]): RepoStatus => ({
  id: name,
  name,
  path: `E:/repos/${name}`,
  group: "全部分组",
  branch: "main",
  relation,
  changeLabel: "-",
  hint: "",
  hasRemote: false,
  remoteUrl: null,
});

const withGroup = (name: string, relation: RepoStatus["relation"], group: string): RepoStatus => ({
  ...base(name, relation),
  group,
});

describe("statusModel", () => {
  it("summarizes collapsed buckets with no_remote last", () => {
    const summary = summarizeCollapsed([
      base("a", "synced"),
      base("b", "local_ahead"),
      base("c", "remote_ahead"),
      base("d", "diverged"),
      base("e", "no_remote"),
    ]);
    expect(summary).toEqual([
      { bucket: "synced", count: 1 },
      { bucket: "syncable", count: 2 },
      { bucket: "needs_attention", count: 1 },
      { bucket: "no_remote", count: 1 },
    ]);
  });

  it("sorts error first and no_remote last in expanded table", () => {
    expect(sortRepos([
      base("no", "no_remote"),
      base("ok", "synced"),
      base("bad", "diverged"),
      base("err", "error"),
    ]).map((repo) => repo.name)).toEqual(["err", "bad", "ok", "no"]);
  });

  it("toCollapsedBucket maps error to needs_attention", () => {
    expect(toCollapsedBucket("error")).toBe("needs_attention");
    expect(toCollapsedBucket("diverged")).toBe("needs_attention");
    expect(toCollapsedBucket("no_remote")).toBe("no_remote");
  });

  it("summarizes collapsed buckets includes error in needs_attention", () => {
    const summary = summarizeCollapsed([
      base("a", "error"),
      base("b", "diverged"),
      base("c", "synced"),
    ]);
    expect(summary).toEqual([
      { bucket: "synced", count: 1 },
      { bucket: "syncable", count: 0 },
      { bucket: "needs_attention", count: 2 },
      { bucket: "no_remote", count: 0 },
    ]);
  });

  it("filterRepos with 全部分组 includes all repos", () => {
    const repos = [
      withGroup("a", "synced", "后端"),
      withGroup("b", "diverged", "Web"),
      withGroup("c", "no_remote", "后端"),
    ];
    const result = filterRepos(repos, "全部分组", "all");
    expect(result).toHaveLength(3);
  });

  it("filterRepos with group selection filters correctly", () => {
    const repos = [
      withGroup("a", "synced", "后端"),
      withGroup("b", "diverged", "Web"),
      withGroup("c", "no_remote", "后端"),
    ];
    const result = filterRepos(repos, "后端", "all");
    expect(result).toHaveLength(2);
    expect(result.map((r) => r.name)).toContain("a");
    expect(result.map((r) => r.name)).toContain("c");
  });

  it("filterRepos with relation filter keeps no_remote last when all selected", () => {
    const repos = [
      withGroup("a", "synced", "后端"),
      withGroup("b", "no_remote", "后端"),
      withGroup("c", "diverged", "后端"),
    ];
    const result = filterRepos(repos, "后端", "all");
    expect(result[result.length - 1].relation).toBe("no_remote");
  });

  it("buildGroupOptions keeps 全部分组 first without duplicating it", () => {
    const repos = [
      withGroup("a", "synced", "全部分组"),
      withGroup("b", "synced", "后端"),
      withGroup("c", "synced", "后端"),
    ];
    expect(buildGroupOptions(repos).map((group) => group.name)).toEqual(["全部分组", "后端"]);
    expect(buildGroupOptions(repos).map((group) => group.count)).toEqual([3, 2]);
  });

  it("getRepoActionAvailability only enables meaningful git actions", () => {
    expect(getRepoActionAvailability({ ...base("ok", "synced"), hasRemote: true, remoteUrl: "https://example.com/repo" })).toMatchObject({
      canFetch: true,
      showPull: false,
      showPush: false,
    });
    expect(getRepoActionAvailability({ ...base("down", "remote_ahead"), hasRemote: true, remoteUrl: "https://example.com/repo" })).toMatchObject({
      canFetch: true,
      showPull: true,
      showPush: false,
    });
    expect(getRepoActionAvailability({ ...base("up", "local_ahead"), hasRemote: true, remoteUrl: "https://example.com/repo" })).toMatchObject({
      canFetch: true,
      showPull: false,
      showPush: true,
    });
    expect(getRepoActionAvailability(base("broken", "error"))).toMatchObject({
      canFetch: false,
      showPull: false,
      showPush: false,
    });
    expect(getRepoActionAvailability(base("local", "no_remote"))).toMatchObject({
      canFetch: false,
      showPull: false,
      showPush: false,
    });
  });

  it("shouldShowSettingsView allows closing the initial empty settings screen", () => {
    expect(shouldShowSettingsView("collapsed", 0, null, false)).toBe(true);
    expect(shouldShowSettingsView("collapsed", 0, null, true)).toBe(false);
    expect(shouldShowSettingsView("settings", 0, null, true)).toBe(true);
  });

  it("shouldShowSettingsView does not auto-open settings when the initial load failed", () => {
    expect(shouldShowSettingsView("collapsed", 0, "读取失败", false)).toBe(false);
  });

  it("reconcileRelationFilter resets an unavailable relation after switching groups", () => {
    const repos = [
      withGroup("split", "diverged", "业务"),
      withGroup("behind", "remote_ahead", "基础设施"),
    ];

    expect(reconcileRelationFilter(repos, "基础设施", "diverged")).toBe("all");
    expect(reconcileRelationFilter(repos, "业务", "diverged")).toBe("diverged");
  });
});
