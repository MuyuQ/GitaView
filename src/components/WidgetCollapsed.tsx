import { summarizeCollapsed } from "../lib/statusModel";
import type { RepoStatus } from "../types";

export function WidgetCollapsed({
  repos,
  onExpand,
  onOpenSettings,
}: {
  repos: RepoStatus[];
  onExpand: () => void;
  onOpenSettings: () => void;
}) {
  const summary = summarizeCollapsed(repos);
  const colorClass = {
    synced: "status-dot green",
    syncable: "status-dot amber",
    needs_attention: "status-dot red",
    no_remote: "status-dot slate",
  } as const;

  return (
    <div className="collapsed-widget-wrap">
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
      <button className="mini-icon-button" onClick={onOpenSettings} aria-label="打开设置">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <path d="M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Z" />
          <path d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06A1.7 1.7 0 0 0 15 19.4a1.7 1.7 0 0 0-1 .6 1.7 1.7 0 0 0-.4 1.1V21a2 2 0 1 1-4 0v-.09A1.7 1.7 0 0 0 8.6 19.4a1.7 1.7 0 0 0-1.88.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-.6-1 1.7 1.7 0 0 0-1.1-.4H3a2 2 0 1 1 0-4h.09A1.7 1.7 0 0 0 4.6 8.6a1.7 1.7 0 0 0-.34-1.88l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1-.6 1.7 1.7 0 0 0 .4-1.1V3a2 2 0 1 1 4 0v.09A1.7 1.7 0 0 0 15.4 4.6a1.7 1.7 0 0 0 1.88-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 19.4 9c.2.33.6.6 1 .6h.1a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1 .4Z" />
        </svg>
      </button>
    </div>
  );
}
