import type { RemoteRelation, RepoStatus } from "../types";

const statusLabels: Record<RemoteRelation, string> = {
  synced: "同步",
  local_ahead: "本地领先",
  remote_ahead: "远程领先",
  diverged: "分叉",
  no_remote: "无远端",
};

export function StatusFilters({ repos, selected, onSelect }: { repos: RepoStatus[]; selected: RemoteRelation | "all"; onSelect: (r: RemoteRelation | "all") => void }) {
  const statuses: (RemoteRelation | "all")[] = ["all", "synced", "local_ahead", "remote_ahead", "diverged", "no_remote"];
  return (
    <div className="filter-row status-filters">
      {statuses.map((s) => {
        const label = s === "all" ? "全部" : statusLabels[s];
        const count = s === "all" ? repos.length : repos.filter((r) => r.relation === s).length;
        return (
          <button
            key={s}
            className={`filter-btn ${s === selected ? "active" : ""}`}
            onClick={() => onSelect(s)}
          >
            {label} {count}
          </button>
        );
      })}
    </div>
  );
}
