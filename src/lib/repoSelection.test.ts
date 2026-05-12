import { describe, expect, it } from "vitest";
import { nextSelectedRepoId } from "./repoSelection";

describe("nextSelectedRepoId", () => {
  it("selects a repo when none is selected", () => {
    expect(nextSelectedRepoId(null, "repo-a")).toBe("repo-a");
  });

  it("clears the selection when clicking the selected repo again", () => {
    expect(nextSelectedRepoId("repo-a", "repo-a")).toBeNull();
  });

  it("switches selection when clicking a different repo", () => {
    expect(nextSelectedRepoId("repo-a", "repo-b")).toBe("repo-b");
  });
});
