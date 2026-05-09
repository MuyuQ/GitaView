import { describe, expect, it } from "vitest";
import { summarizeCollapsed, sortRepos, filterRepos } from "./statusModel";
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

  it("sorts no_remote last in expanded table", () => {
    expect(sortRepos([
      base("no", "no_remote"),
      base("ok", "synced"),
      base("bad", "diverged"),
    ]).map((repo) => repo.name)).toEqual(["bad", "ok", "no"]);
  });

  it("filterRepos with 全部分组 includes all repos", () => {
    const repos = [
      withGroup("a", "synced", "后端"),
      withGroup("b", "diverged", "Web"),
      withGroup("c", "no_remote", "后端"),
    ];
    const result = filterRepos(repos, "全部分组", "all", "");
    expect(result).toHaveLength(3);
  });

  it("filterRepos with group selection filters correctly", () => {
    const repos = [
      withGroup("a", "synced", "后端"),
      withGroup("b", "diverged", "Web"),
      withGroup("c", "no_remote", "后端"),
    ];
    const result = filterRepos(repos, "后端", "all", "");
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
    const result = filterRepos(repos, "后端", "all", "");
    expect(result[result.length - 1].relation).toBe("no_remote");
  });

  it("filterRepos with query matches name and branch", () => {
    const repos = [
      { ...base("api", "synced"), branch: "dev" },
      { ...base("web", "synced"), branch: "main" },
      { ...base("docs", "synced"), branch: "feature" },
    ];
    const byName = filterRepos(repos, "全部分组", "all", "api");
    expect(byName).toHaveLength(1);
    expect(byName[0].name).toBe("api");

    const byBranch = filterRepos(repos, "全部分组", "all", "feature");
    expect(byBranch).toHaveLength(1);
    expect(byBranch[0].branch).toBe("feature");
  });

  it("filterRepos with empty query returns all filtered repos", () => {
    const repos = [
      withGroup("a", "synced", "Web"),
      withGroup("b", "diverged", "Web"),
    ];
    const result = filterRepos(repos, "Web", "all", "");
    expect(result).toHaveLength(2);
  });
});
