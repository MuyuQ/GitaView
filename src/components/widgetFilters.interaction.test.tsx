import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { WidgetExpanded } from "./WidgetExpanded";
import type { RepoStatus } from "../types";

vi.mock("../lib/commands", () => ({
  fetchRepo: vi.fn(),
  pullRepo: vi.fn(),
  pushRepo: vi.fn(),
  openRepoDirectory: vi.fn(),
  openRepoRemote: vi.fn(),
}));

const repos: RepoStatus[] = [
  {
    id: "frontend",
    name: "frontend",
    path: "~/projects/frontend",
    group: "产品",
    branch: "main",
    relation: "synced",
    changeLabel: "✓",
    hint: "本地与远端一致",
    hasRemote: true,
    remoteUrl: "https://github.com/example/frontend",
  },
  {
    id: "backend",
    name: "backend",
    path: "~/projects/backend",
    group: "后端",
    branch: "develop",
    relation: "diverged",
    changeLabel: "↑ 1 ↓ 2",
    hint: "本地与远端分叉",
    hasRemote: true,
    remoteUrl: "https://github.com/example/backend",
  },
  {
    id: "scratch",
    name: "scratch",
    path: "~/projects/scratch",
    group: "后端",
    branch: "main",
    relation: "no_remote",
    changeLabel: "无远端",
    hint: "未配置远端",
    hasRemote: false,
    remoteUrl: null,
  },
];

function renderExpanded() {
  render(
    <WidgetExpanded
      repos={repos}
      lastRefreshAt={new Date("2026-06-03T12:00:00Z")}
      refreshing={false}
      refreshError={null}
      onRefresh={() => undefined}
      onCollapse={() => undefined}
      onOpenSettings={() => undefined}
      allowDrag={false}
      onStartDrag={() => undefined}
    />,
  );
}

function visibleRepoNames(): string[] {
  return [...document.querySelectorAll(".repo-name-text")].map((el) => el.textContent ?? "");
}

afterEach(() => {
  cleanup();
});

describe("expanded widget filter interactions", () => {
  it("shows every repo before filtering", () => {
    renderExpanded();
    // statusModel 按严重度排序：diverged 先于 synced
    expect(visibleRepoNames()).toEqual(["backend", "frontend", "scratch"]);
  });

  it("narrows rows by group and keeps status counts consistent with the narrowed set", async () => {
    const user = userEvent.setup();
    renderExpanded();

    await user.click(screen.getByRole("button", { name: "后端 2" }));
    expect(visibleRepoNames()).toEqual(["backend", "scratch"]);

    // 状态筛选行应基于分组后的集合计数：无远端只剩 scratch
    const statusRow = document.querySelector(".filter-row.status-filters") as HTMLElement;
    expect(within(statusRow).getByRole("button", { name: /无远端/ }).textContent).toContain("1");
    // 分叉按钮在分组后仍显示 backend
    expect(within(statusRow).getByRole("button", { name: /分叉/ }).textContent).toContain("1");
    // 全部分组在分组筛选中仍存在且计数为全部
    expect(screen.getByRole("button", { name: "全部分组 3" })).toBeTruthy();
  });

  it("combines group and relation filters and restores rows when reset to 全部", async () => {
    const user = userEvent.setup();
    renderExpanded();

    await user.click(screen.getByRole("button", { name: "后端 2" }));
    // 仓库行也有 button role（整行可点击），状态筛选需限定在筛选行内
    const statusRow = document.querySelector(".filter-row.status-filters") as HTMLElement;
    await user.click(within(statusRow).getByRole("button", { name: /分叉/ }));
    expect(visibleRepoNames()).toEqual(["backend"]);

    await user.click(within(statusRow).getByRole("button", { name: "全部 2" }));
    expect(visibleRepoNames()).toEqual(["backend", "scratch"]);
  });

  it("filters rows by search query across name, group, and branch", async () => {
    const user = userEvent.setup();
    renderExpanded();

    await user.type(screen.getByRole("textbox", { name: "搜索仓库" }), "develop");
    expect(visibleRepoNames()).toEqual(["backend"]);

    await user.clear(screen.getByRole("textbox", { name: "搜索仓库" }));
    await user.type(screen.getByRole("textbox", { name: "搜索仓库" }), "产品");
    expect(visibleRepoNames()).toEqual(["frontend"]);
  });

  it("expands the action panel of the clicked repo row", async () => {
    const user = userEvent.setup();
    renderExpanded();

    await user.click(screen.getByText("frontend"));
    expect(screen.getByRole("button", { name: "Fetch" })).toBeTruthy();
    // 其他行没有操作面板
    const panels = document.querySelectorAll(".repo-actions-panel");
    expect(panels).toHaveLength(1);
  });
});
