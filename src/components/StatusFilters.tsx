import type { RemoteRelation, RepoStatus } from "../types";
import { uiStrings } from "../lib/strings";

const statusLabels: Record<RemoteRelation, string> = {
  error: uiStrings.relation.error,
  synced: uiStrings.relation.synced,
  local_ahead: uiStrings.relation.local_ahead,
  remote_ahead: uiStrings.relation.remote_ahead,
  diverged: uiStrings.relation.diverged,
  no_remote: uiStrings.relation.no_remote,
};

const statusDotClass: Record<RemoteRelation, string> = {
  error: "red",
  synced: "green",
  local_ahead: "amber",
  remote_ahead: "amber",
  diverged: "red",
  no_remote: "slate",
};

export function StatusFilters({ repos, selected, onSelect }: { repos: RepoStatus[]; selected: RemoteRelation | "all"; onSelect: (r: RemoteRelation | "all") => void }) {
  const statuses: RemoteRelation[] = ["error", "diverged", "remote_ahead", "local_ahead", "synced", "no_remote"];
  // 计算各状态的数量
  const counts: Record<RemoteRelation, number> = {
    error: repos.filter((r) => r.relation === "error").length,
    diverged: repos.filter((r) => r.relation === "diverged").length,
    remote_ahead: repos.filter((r) => r.relation === "remote_ahead").length,
    local_ahead: repos.filter((r) => r.relation === "local_ahead").length,
    synced: repos.filter((r) => r.relation === "synced").length,
    no_remote: repos.filter((r) => r.relation === "no_remote").length,
  };

  return (
    <div className="filter-row status-filters">
      {/* "全部" 按钮始终显示 */}
      <button
        className={`filter-btn ${selected === "all" ? "active" : ""}`}
        onClick={() => onSelect("all")}
        aria-pressed={selected === "all"}
      >
        {uiStrings.filters.all} <span className="filter-count">{repos.length}</span>
      </button>
      {/* 其他状态只在数量 > 0 时显示，圆点 + 文字 + 计数三重编码 */}
      {statuses.map((s) => {
        const count = counts[s];
        if (count === 0) return null;
        return (
          <button
            key={s}
            className={`filter-btn ${s === selected ? "active" : ""}`}
            onClick={() => onSelect(s)}
            aria-pressed={s === selected}
          >
            <span className={`status-dot ${statusDotClass[s]}`} aria-hidden="true" />
            {statusLabels[s]} <span className="filter-count">{count}</span>
          </button>
        );
      })}
    </div>
  );
}
