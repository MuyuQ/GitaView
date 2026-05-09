import { summarizeCollapsed } from "../lib/statusModel";
import type { RepoStatus } from "../types";

export function WidgetCollapsed({ repos, onExpand }: { repos: RepoStatus[]; onExpand: () => void }) {
  const summary = summarizeCollapsed(repos);
  const colorClass = {
    synced: "status-dot green",
    syncable: "status-dot amber",
    needs_attention: "status-dot red",
    no_remote: "status-dot slate",
  } as const;

  return (
    <button className="collapsed-widget" onClick={onExpand} aria-label="展开仓库状态">
      <span className="brand">GitaView</span>
      <span className="total">{repos.length}</span>
      <span className="summary">
        {summary.map((item) => (
          <span className="summary-item" key={item.bucket}>
            <span className={colorClass[item.bucket]} />
            <span>{item.count}</span>
          </span>
        ))}
      </span>
    </button>
  );
}
