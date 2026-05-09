import { describe, expect, it } from "vitest";
import { summarizeCollapsed, sortRepos } from "./statusModel";
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
});
