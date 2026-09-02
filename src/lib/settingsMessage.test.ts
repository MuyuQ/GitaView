import { describe, expect, it } from "vitest";
import { errorMessage, errorMessageText, okMessage } from "./settingsMessage";

describe("settingsMessage", () => {
  it("tags success and failure messages with distinct kinds", () => {
    expect(okMessage("仓库已添加")).toEqual({ kind: "ok", text: "仓库已添加" });
    expect(errorMessage("添加失败", "disk full")).toEqual({
      kind: "error",
      text: "添加失败：disk full",
    });
    expect(errorMessageText("请输入仓库路径")).toEqual({
      kind: "error",
      text: "请输入仓库路径",
    });
  });
});
