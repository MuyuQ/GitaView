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

  useEffect(() => {
    listRepoStatuses()
      .then((data) => { setRepos(data); setLoading(false); })
      .catch((err) => { setError(String(err)); setLoading(false); });
  }, []);

  if (loading) return <main className="app-shell">正在刷新仓库状态...</main>;
  if (error) return <main className="app-shell" role="alert">加载仓库失败：{error}</main>;
  if (view === "settings") return <SettingsShell />;
  if (repos.length === 0) return <main className="app-shell">还没有添加仓库</main>;

  return view === "expanded" ? (
    <WidgetExpanded repos={repos} />
  ) : (
    <WidgetCollapsed repos={repos} onExpand={() => setView("expanded")} />
  );
}
