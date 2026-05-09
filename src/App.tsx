import { useState } from "react";
import { fixtureRepos } from "./fixtures";
import type { RepoStatus } from "./types";
import { WidgetCollapsed } from "./components/WidgetCollapsed";
import { WidgetExpanded } from "./components/WidgetExpanded";
import { SettingsShell } from "./components/settings/SettingsShell";

export default function App() {
  const [view, setView] = useState<"collapsed" | "expanded" | "settings">("collapsed");
  const repos: RepoStatus[] = fixtureRepos;

  if (view === "settings") return <SettingsShell />;

  return view === "expanded" ? (
    <WidgetExpanded repos={repos} />
  ) : (
    <WidgetCollapsed repos={repos} onExpand={() => setView("expanded")} />
  );
}
