import type { RemoteRelation, RepoStatus } from "../types";

const statusLabels: Record<RemoteRelation, string> = {
  error: "读取失败",
  synced: "同步",
  local_ahead: "本地领先",
  remote_ahead: "远程领先",
  diverged: "分叉",
  no_remote: "无远端",
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
      >
        全部 {repos.length}
      </button>
      {/* 其他状态只在数量 > 0 时显示 */}
      {statuses.map((s) => {
        const count = counts[s];
        if (count === 0) return null;
        return (
          <button
            key={s}
            className={`filter-btn ${s === selected ? "active" : ""}`}
            onClick={() => onSelect(s)}
          >
            {statusLabels[s]} {count}
          </button>
        );
      })}
    </div>
  );
}
