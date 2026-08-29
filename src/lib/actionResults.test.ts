import { describe, expect, it } from "vitest";
import {
  actionResultClassName,
  actionResultRole,
  formatActionResult,
} from "./actionResults";

describe("formatActionResult", () => {
  it("keeps success messages distinguishable from failures", () => {
    const success = formatActionResult("Fetch", { ok: true, message: "Fetch 已完成" });
    expect(success).toEqual({ kind: "success", text: "Fetch 已完成" });

    const defaultSuccess = formatActionResult("目录", { ok: true });
    expect(defaultSuccess).toEqual({ kind: "success", text: "目录 已完成" });
  });

  it("marks failures with an error kind and preserves the backend message", () => {
    const failure = formatActionResult("Push", {
      ok: false,
      error: "git push failed: rejected",
    });

    expect(failure.kind).toBe("error");
    expect(failure.text).toBe("Push 失败：git push failed: rejected");
  });

  it("maps kinds to distinct presentation classes and roles", () => {
    expect(actionResultClassName("error")).toContain("action-result-error");
    expect(actionResultClassName("success")).not.toContain("action-result-error");
    expect(actionResultRole("error")).toBe("alert");
    expect(actionResultRole("success")).toBe("status");
  });
});
