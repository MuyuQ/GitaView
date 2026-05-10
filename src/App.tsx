import { useEffect, useState } from "react";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { listRepoStatuses, getSettings } from "./lib/commands";
import type { AppSettings, RepoStatus } from "./types";
import { WidgetCollapsed } from "./components/WidgetCollapsed";
import { WidgetExpanded } from "./components/WidgetExpanded";
import { SettingsShell } from "./components/settings/SettingsShell";

const windowSizes = {
  collapsed: new LogicalSize(300, 72),
  expanded: new LogicalSize(720, 560),
  settings: new LogicalSize(760, 540),
} as const;

export default function App() {
  const [view, setView] = useState<"collapsed" | "expanded" | "settings">("collapsed");
  const [repos, setRepos] = useState<RepoStatus[]>([]);
  const [initialLoading, setInitialLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [initialError, setInitialError] = useState<string | null>(null);
  const [refreshError, setRefreshError] = useState<string | null>(null);
  const [lastRefreshAt, setLastRefreshAt] = useState<Date | null>(null);
  const [refreshSettings, setRefreshSettings] = useState<AppSettings["refresh"] | null>(null);

  function refreshRepos() {
    setRefreshing(true);
    setRefreshError(null);
    listRepoStatuses()
      .then((data) => {
        setRepos(data);
        setLastRefreshAt(new Date());
        setInitialError(null);
      })
      .catch((err) => {
        const message = String(err);
        if (initialLoading && repos.length === 0) {
          setInitialError(message);
        } else {
          setRefreshError(message);
        }
      })
      .finally(() => {
        setInitialLoading(false);
        setRefreshing(false);
      });
  }

  useEffect(() => {
    refreshRepos();
  }, []);

  useEffect(() => {
    const appWindow = getCurrentWindow();
    const targetView = view === "settings" || repos.length === 0 ? "settings" : view;
    const size = windowSizes[targetView];
    appWindow.setSize(size).catch((err) => {
      console.error("调整窗口尺寸失败", err);
    });
  }, [view, repos.length]);

  useEffect(() => {
    getSettings()
      .then((settings) => setRefreshSettings(settings.refresh))
      .catch(() => {});
  }, []);

  useEffect(() => {
    if (!refreshSettings?.lightweightRefreshEnabled) return;
    const intervalMinutes = Math.min(Math.max(refreshSettings.intervalMinutes, 1), 60);
    const id = window.setInterval(refreshRepos, intervalMinutes * 60_000);
    return () => window.clearInterval(id);
  }, [refreshSettings]);

  if (initialLoading) return <main className="app-shell">正在刷新仓库状态...</main>;
  if (initialError && repos.length === 0) {
    return <main className="app-shell" role="alert">加载仓库失败：{initialError}</main>;
  }
  if (view === "settings" || repos.length === 0) {
    return (
      <SettingsShell
        onClose={() => {
          refreshRepos();
          setView("collapsed");
          getSettings().then((settings) => setRefreshSettings(settings.refresh)).catch(() => {});
        }}
      />
    );
  }

  return view === "expanded" ? (
    <WidgetExpanded
      repos={repos}
      lastRefreshAt={lastRefreshAt}
      refreshing={refreshing}
      refreshError={refreshError}
      onRefresh={refreshRepos}
      onCollapse={() => setView("collapsed")}
      onOpenSettings={() => setView("settings")}
    />
  ) : (
    <WidgetCollapsed
      repos={repos}
      onExpand={() => setView("expanded")}
      onOpenSettings={() => setView("settings")}
    />
  );
}
