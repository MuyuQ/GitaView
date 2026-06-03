import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { RepoActions } from "./RepoActions";
import { SettingsShell } from "./settings/SettingsShell";
import { WidgetCollapsed } from "./WidgetCollapsed";
import { WidgetExpanded } from "./WidgetExpanded";
import type { RepoStatus } from "../types";

const sampleRepos: RepoStatus[] = [
  {
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
  },
  {
    id: "broken",
    name: "broken",
    path: "~/projects/broken",
    group: "Core",
    branch: "未知",
    relation: "error",
    changeLabel: "错误",
    hint: "读取失败",
    hasRemote: false,
    remoteUrl: null,
  },
];

describe("component render smoke tests", () => {
  it("renders collapsed status labels and counts without relying on colors alone", () => {
    const html = renderToStaticMarkup(
      <WidgetCollapsed
        repos={sampleRepos}
        allowDrag={false}
        onExpand={() => undefined}
        onStartDrag={() => undefined}
        onRefresh={() => undefined}
        onExit={() => undefined}
      />,
    );

    expect(html).toContain("GitaView");
    expect(html).toContain("仓库");
    expect(html).toContain("需关注");
    expect(html).toContain("待同步");
  });

  it("renders expanded refresh state, search, filters, and repository rows", () => {
    const html = renderToStaticMarkup(
      <WidgetExpanded
        repos={sampleRepos}
        lastRefreshAt={new Date("2026-06-03T12:00:00Z")}
        refreshing={true}
        refreshError="network failed"
        onRefresh={() => undefined}
        onCollapse={() => undefined}
        onOpenSettings={() => undefined}
        allowDrag={false}
        onStartDrag={() => undefined}
      />,
    );

    expect(html).toContain("仓库状态");
    expect(html).toContain("刷新中");
    expect(html).toContain("搜索或分组");
    expect(html).toContain("刷新失败：network failed");
    expect(html).toContain("needs-push");
  });

  it("renders action availability for safe and unsafe repository states", () => {
    const pushable = renderToStaticMarkup(
      <RepoActions repo={sampleRepos[0]} onRefresh={() => undefined} />,
    );
    const broken = renderToStaticMarkup(
      <RepoActions repo={sampleRepos[1]} onRefresh={() => undefined} />,
    );

    expect(pushable).toContain(">Push<");
    expect(pushable).toContain(">Fetch<");
    expect(broken).not.toContain(">Push<");
    expect(broken).not.toContain(">Pull<");
    expect(broken).toContain("disabled=\"\"");
  });

  it("renders the settings shell navigation and default repository section", () => {
    const html = renderToStaticMarkup(<SettingsShell onClose={() => undefined} />);

    expect(html).toContain("设置导航");
    expect(html).toContain("仓库设置");
    expect(html).toContain("常规设置");
    expect(html).toContain("仓库管理");
    expect(html).toContain("关闭");
  });
});
