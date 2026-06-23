import { useWidgetView } from "./lib/useWidgetView";
import { shouldShowSettingsView } from "./lib/statusModel";
import { WidgetCollapsed } from "./components/WidgetCollapsed";
import { WidgetExpanded } from "./components/WidgetExpanded";
import { SettingsShell } from "./components/settings/SettingsShell";

export default function App() {
  const widget = useWidgetView();

  if (widget.initialLoading) return <main className="app-shell">正在刷新仓库状态...</main>;

  const shouldRenderSettings = shouldShowSettingsView(
    widget.view,
    widget.repos.length,
    widget.initialError,
    widget.emptySettingsDismissed,
  );

  if (shouldRenderSettings) {
    return (
      <main className="app-shell settings-shell">
        <SettingsShell onClose={widget.dismissEmptySettings} />
      </main>
    );
  }

  if (widget.initialError && widget.repos.length === 0) {
    return (
      <main className="app-shell error-shell" role="alert">
        <p>加载仓库失败：{widget.initialError}</p>
        <button onClick={() => widget.refreshRepos({ initial: true })}>重试</button>
        <button onClick={widget.navigateToSettings}>
          打开设置
        </button>
      </main>
    );
  }

  return widget.view === "expanded" ? (
    <WidgetExpanded
      repos={widget.repos}
      lastRefreshAt={widget.lastRefreshAt}
      refreshing={widget.refreshing}
      refreshError={widget.refreshError}
      onRefresh={() => widget.refreshRepos({ initial: false })}
      onCollapse={widget.collapseExpandedView}
      onOpenSettings={() => widget.showView("settings")}
      allowDrag={widget.allowWidgetDrag}
      onStartDrag={widget.startDrag}
    />
  ) : (
    <main className="app-shell collapsed-shell">
      <WidgetCollapsed
        repos={widget.repos}
        allowDrag={widget.allowWidgetDrag}
        onExpand={widget.expandCollapsedView}
        onStartDrag={widget.startDrag}
        onRefresh={() => widget.refreshRepos({ initial: false })}
        onExit={widget.handleExit}
      />
    </main>
  );
}
