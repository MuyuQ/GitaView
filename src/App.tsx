import { useState } from "react";
import { fixtureRepos } from "./fixtures";
import type { RepoStatus } from "./types";
import { WidgetCollapsed } from "./components/WidgetCollapsed";
import { WidgetExpanded } from "./components/WidgetExpanded";

export default function App() {
  const [expanded, setExpanded] = useState(false);
  const repos: RepoStatus[] = fixtureRepos;

  return expanded ? (
    <WidgetExpanded repos={repos} />
  ) : (
    <WidgetCollapsed repos={repos} onExpand={() => setExpanded(true)} />
  );
}
