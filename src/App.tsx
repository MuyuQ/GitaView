import { useWidgetView } from "./lib/useWidgetView";
import { shouldShowSettingsView } from "./lib/statusModel";
import { uiStrings } from "./lib/strings";
import { WidgetCollapsed } from "./components/WidgetCollapsed";
import { WidgetExpanded } from "./components/WidgetExpanded";
import { SettingsShell } from "./components/settings/SettingsShell";

export default function App() {
  const widget = useWidgetView();

  if (widget.initialLoading) return <main className="app-shell">{uiStrings.app.initialLoading}</main>;

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
        <p>{uiStrings.app.loadFailedPrefix}：{widget.initialError}</p>
        <button onClick={() => widget.refreshRepos({ initial: true })}>{uiStrings.app.retry}</button>
        <button onClick={widget.navigateToSettings}>
          {uiStrings.app.openSettings}
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
      focusRepoId={widget.focusRepoId}
      onFocusRepoHandled={widget.clearFocusRepo}
    />
  ) : (
    <main className="app-shell collapsed-shell">
      <WidgetCollapsed
        repos={widget.repos}
        allowDrag={widget.allowWidgetDrag}
        refreshError={widget.refreshError}
        lastRefreshAt={widget.lastRefreshAt}
        onExpand={widget.expandCollapsedView}
        onStartDrag={widget.startDrag}
        onRefresh={() => widget.refreshRepos({ initial: false })}
        onExit={widget.handleExit}
      />
    </main>
  );
}
