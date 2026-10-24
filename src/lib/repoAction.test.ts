import { describe, expect, it, vi } from "vitest";
import { executeRepoAction } from "./actionResults";

describe("repository action completion", () => {
  it.each(["Fetch", "Pull", "Push"])("refreshes after a failed %s and preserves its error", async (action) => {
    const refresh = vi.fn();
    const result = await executeRepoAction(action, async () => { throw "merge conflict"; }, refresh);
    expect(refresh).toHaveBeenCalledOnce();
    expect(result).toEqual({ kind: "error", text: `${action} 失败：merge conflict` });
  });

  it("refreshes after success but not after opening a directory", async () => {
    const refresh = vi.fn();
    await executeRepoAction("Fetch", async () => "done", refresh);
    await executeRepoAction("目录", async () => {}, refresh);
    expect(refresh).toHaveBeenCalledOnce();
  });
});
