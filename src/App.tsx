import { useEffect, useState } from "react";
import { listRepoStatuses } from "./lib/commands";
import type { RepoStatus } from "./types";
import { WidgetCollapsed } from "./components/WidgetCollapsed";
import { WidgetExpanded } from "./components/WidgetExpanded";
import { SettingsShell } from "./components/settings/SettingsShell";

export default function App() {
  const [view, setView] = useState<"collapsed" | "expanded" | "settings">("collapsed");
  const [repos, setRepos] = useState<RepoStatus[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [lastRefreshAt, setLastRefreshAt] = useState<Date | null>(null);

  function refreshRepos() {
    setLoading(true);
    setError(null);
    listRepoStatuses()
      .then((data) => {
        setRepos(data);
        setLastRefreshAt(new Date());
        setLoading(false);
      })
      .catch((err) => {
        setError(String(err));
        setLoading(false);
      });
  }

  useEffect(() => {
    refreshRepos();
  }, []);

  if (loading) return <main className="app-shell">正在刷新仓库状态...</main>;
  if (error) return <main className="app-shell" role="alert">加载仓库失败：{error}</main>;
  if (view === "settings" || repos.length === 0) {
    return (
      <SettingsShell
        onClose={() => {
          refreshRepos();
          setView("collapsed");
        }}
      />
    );
  }

  return view === "expanded" ? (
    <WidgetExpanded
      repos={repos}
      lastRefreshAt={lastRefreshAt}
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
