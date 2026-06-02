import { summarizeActionableCollapsed } from "../lib/collapsedSummary";
import type { WidgetTransitionMode } from "../lib/widgetTransition";
import type { RepoStatus } from "../types";

export function TransitionSurface({ mode, repos }: { mode: WidgetTransitionMode; repos: RepoStatus[] }) {
  const summary = summarizeActionableCollapsed(repos);
  const colorClass = {
    syncable: "status-dot amber",
    needs_attention: "status-dot red",
    no_remote: "status-dot slate",
  } as const;

  return (
    <div className={`transition-stage transition-stage--${mode}`}>
      <div className={`transition-surface transition-surface--${mode}`} />
      <div className={`transition-capsule transition-capsule--${mode}`}>
        <span className="brand">GitaView</span>
        <span className="repo-word">仓库</span>
        <span className="total">{repos.length}</span>
        <span className="summary">
          {summary.map((item) => (
            <span className="summary-item" key={item.bucket}>
              <span className={colorClass[item.bucket]} aria-hidden="true" />
              <span>{item.label}</span>
              <span>{item.count}</span>
            </span>
          ))}
        </span>
      </div>
    </div>
  );
}
