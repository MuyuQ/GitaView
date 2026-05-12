import { describe, expect, it } from "vitest";
import { getRepositoryDisplayText, nextRevealedRepoId } from "./repositorySettingsView";
import type { RepoRecord } from "../types";

const repo: RepoRecord = {
  id: "aegisota",
  name: "AegisOTA",
  path: "E:/Git_Repositories/AegisOTA",
  group: "全部分组",
};

describe("nextRevealedRepoId", () => {
  it("reveals the clicked repository path when another repo or none is revealed", () => {
    expect(nextRevealedRepoId(null, "aegisota")).toBe("aegisota");
    expect(nextRevealedRepoId("bookmarkhub", "aegisota")).toBe("aegisota");
  });

  it("hides the path when the same repository is clicked again", () => {
    expect(nextRevealedRepoId("aegisota", "aegisota")).toBeNull();
  });
});

describe("getRepositoryDisplayText", () => {
  it("shows the repository name by default", () => {
    expect(getRepositoryDisplayText(repo, null)).toBe("AegisOTA");
  });

  it("shows the repository path when the repository is revealed", () => {
    expect(getRepositoryDisplayText(repo, "aegisota")).toBe("E:/Git_Repositories/AegisOTA");
  });
});
