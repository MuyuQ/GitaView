import { describe, expect, it } from "vitest";
import { summarizeActionableCollapsed } from "./collapsedSummary";
import type { RepoStatus } from "../types";

const repo = (name: string, relation: RepoStatus["relation"]): RepoStatus => ({
  id: name,
  name,
  path: `E:/repos/${name}`,
  group: "全部分组",
  branch: "main",
  relation,
  changeLabel: "-",
  hint: "",
  hasRemote: relation !== "no_remote",
  remoteUrl: relation === "no_remote" ? null : "https://example.com/repo",
});

describe("summarizeActionableCollapsed", () => {
  it("omits synced repositories and keeps action labels in stable order", () => {
    expect(summarizeActionableCollapsed([
      repo("clean", "synced"),
      repo("up", "local_ahead"),
      repo("down", "remote_ahead"),
      repo("split", "diverged"),
      repo("broken", "error"),
      repo("local", "no_remote"),
    ])).toEqual([
      { bucket: "syncable", label: "待同步", count: 2 },
      { bucket: "needs_attention", label: "需关注", count: 2 },
      { bucket: "no_remote", label: "无远端", count: 1 },
    ]);
  });

  it("keeps zero-count action buckets visible", () => {
    expect(summarizeActionableCollapsed([repo("clean", "synced")])).toEqual([
      { bucket: "syncable", label: "待同步", count: 0 },
      { bucket: "needs_attention", label: "需关注", count: 0 },
      { bucket: "no_remote", label: "无远端", count: 0 },
    ]);
  });
});
