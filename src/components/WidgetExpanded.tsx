import { useState } from "react";
import { filterRepos } from "../lib/statusModel";
import type { RemoteRelation, RepoStatus } from "../types";
import { GroupFilters } from "./GroupFilters";
import { StatusFilters } from "./StatusFilters";
import { RepoTable } from "./RepoTable";

export function WidgetExpanded({
  repos,
  lastRefreshAt,
  refreshing,
  onRefresh,
  onCollapse,
  onOpenSettings,
}: {
  repos: RepoStatus[];
  lastRefreshAt: Date | null;
  refreshing: boolean;
  onRefresh: () => void;
  onCollapse: () => void;
  onOpenSettings: () => void;
}) {
  const [group, setGroup] = useState("全部分组");
  const [relation, setRelation] = useState<RemoteRelation | "all">("all");
  const [selectedRepoId, setSelectedRepoId] = useState<string | null>(null);
  const [query, setQuery] = useState("");

  const groupRepos = group === "全部分组" ? repos : repos.filter((repo) => repo.group === group);
  const visibleRepos = filterRepos(repos, group, relation, query);

  return (
    <section className="expanded-widget">
      <header className="widget-toolbar">
        <div>
          <h1>仓库状态</h1>
          <p>{lastRefreshAt ? `刷新时间 ${lastRefreshAt.toLocaleTimeString("zh-CN", { hour12: false })}` : "尚未刷新"}</p>
        </div>
        <input
          aria-label="搜索仓库"
          placeholder="搜索仓库 / 分支"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
        <button className="collapse-btn refresh-btn" onClick={onRefresh} disabled={refreshing} aria-label="刷新">
          {refreshing ? "刷新中" : "刷新"}
        </button>
        <button className="collapse-btn" onClick={onOpenSettings} aria-label="打开设置">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
            <path d="M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Z" />
            <path d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06A1.7 1.7 0 0 0 15 19.4a1.7 1.7 0 0 0-1 .6 1.7 1.7 0 0 0-.4 1.1V21a2 2 0 1 1-4 0v-.09A1.7 1.7 0 0 0 8.6 19.4a1.7 1.7 0 0 0-1.88.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-.6-1 1.7 1.7 0 0 0-1.1-.4H3a2 2 0 1 1 0-4h.09A1.7 1.7 0 0 0 4.6 8.6a1.7 1.7 0 0 0-.34-1.88l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1-.6 1.7 1.7 0 0 0 .4-1.1V3a2 2 0 1 1 4 0v.09A1.7 1.7 0 0 0 15.4 4.6a1.7 1.7 0 0 0 1.88-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 19.4 9c.2.33.6.6 1 .6h.1a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1 .4Z" />
          </svg>
        </button>
        <button className="collapse-btn" onClick={onCollapse} aria-label="收起">
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5">
            <path d="M12 4L4 12M4 4l8 8" />
          </svg>
        </button>
      </header>
      <GroupFilters repos={repos} selected={group} onSelect={setGroup} />
      <StatusFilters repos={groupRepos} selected={relation} onSelect={setRelation} />
      {visibleRepos.length === 0 ? (
        <p className="repo-empty">没有匹配的仓库</p>
      ) : (
        <RepoTable repos={visibleRepos} selectedRepoId={selectedRepoId} onSelect={setSelectedRepoId} onRefresh={onRefresh} />
      )}
    </section>
  );
}
