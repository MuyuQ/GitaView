import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { RepoActions } from "./RepoActions";
import type { RepoStatus } from "../types";

vi.mock("../lib/commands", () => ({
  fetchRepo: vi.fn(),
  pullRepo: vi.fn(),
  pushRepo: vi.fn(),
  openRepoDirectory: vi.fn(),
  openRepoRemote: vi.fn(),
}));

import { pullRepo, pushRepo } from "../lib/commands";

const mockedPull = vi.mocked(pullRepo);
const mockedPush = vi.mocked(pushRepo);

const pushableRepo: RepoStatus = {
  id: "needs-push",
  name: "needs-push",
  path: "~/projects/needs-push",
  group: "Core",
  branch: "main",
  relation: "local_ahead",
  changeLabel: "↑ 1",
  hint: "需要推送",
  hasRemote: true,
  remoteUrl: "https://github.com/example/needs-push",
};

const pullableRepo: RepoStatus = {
  ...pushableRepo,
  id: "needs-pull",
  name: "needs-pull",
  relation: "remote_ahead",
  changeLabel: "↓ 2",
  hint: "远程有更新",
};

function renderActions(repo: RepoStatus = pushableRepo) {
  const onRefresh = vi.fn();
  render(<RepoActions repo={repo} onRefresh={onRefresh} />);
  return { onRefresh };
}

beforeEach(() => {
  vi.clearAllMocks();
});

afterEach(() => {
  cleanup();
});

describe("RepoActions pull/push confirmation flow", () => {
  it("requires a second click on Pull before invoking, then reports success", async () => {
    const user = userEvent.setup();
    const { onRefresh } = renderActions(pullableRepo);
    mockedPull.mockResolvedValueOnce("已拉取 origin/main");

    await user.click(screen.getByRole("button", { name: "Pull" }));
    // 第一次点击只进入确认态：不调用后端，并展示风险提示
    expect(mockedPull).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "确认 Pull" })).toBeTruthy();
    expect(screen.getByText("Pull 会修改当前仓库工作区，是否继续？")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "确认 Pull" }));
    expect(mockedPull).toHaveBeenCalledWith("needs-pull", true);
    await screen.findByText("已拉取 origin/main");
    expect(onRefresh).toHaveBeenCalled();
  });

  it("reports pull failures distinctly from success", async () => {
    const user = userEvent.setup();
    renderActions(pullableRepo);
    mockedPull.mockRejectedValueOnce(new Error("git pull failed: conflict"));

    await user.click(screen.getByRole("button", { name: "Pull" }));
    await user.click(screen.getByRole("button", { name: "确认 Pull" }));

    await screen.findByText("Pull 失败：Error: git pull failed: conflict");
  });

  it("requires confirmation for Push and reports backend failures", async () => {
    const user = userEvent.setup();
    renderActions();
    mockedPush.mockRejectedValueOnce(new Error("rejected (non-fast-forward)"));

    await user.click(screen.getByRole("button", { name: "Push" }));
    expect(mockedPush).not.toHaveBeenCalled();
    expect(screen.getByText("Push 会更新远端分支，是否继续？")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "确认 Push" }));
    await screen.findByText("Push 失败：Error: rejected (non-fast-forward)");
    expect(mockedPush).toHaveBeenCalledWith("needs-push", true);
  });
});
