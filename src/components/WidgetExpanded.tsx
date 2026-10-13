import { useDeferredValue, useEffect, useRef, useState } from "react";
import { shouldPromoteExpandedDrag, shouldStartExpandedDrag } from "../lib/windowDrag";
import { filterRepos, reconcileExpandedFilters, reconcileRelationFilter } from "../lib/statusModel";
import { uiStrings } from "../lib/strings";
import type { RemoteRelation, RepoStatus } from "../types";
import { GroupFilters } from "./GroupFilters";
import { StatusFilters } from "./StatusFilters";
import { RepoTable } from "./RepoTable";

function SearchIcon() {
  return (
    <svg className="search-icon" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
      <circle cx="11" cy="11" r="7" />
      <path d="m20 20-3.5-3.5" />
    </svg>
  );
}

function SettingsIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true">
      <path d="M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Z" />
      <path d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06A1.7 1.7 0 0 0 15 19.4a1.7 1.7 0 0 0-1 .6 1.7 1.7 0 0 0-.4 1.1V21a2 2 0 1 1-4 0v-.09A1.7 1.7 0 0 0 8.6 19.4a1.7 1.7 0 0 0-1.88.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-.6-1 1.7 1.7 0 0 0-1.1-.4H3a2 2 0 1 1 0-4h.09A1.7 1.7 0 0 0 4.6 8.6a1.7 1.7 0 0 0-.34-1.88l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1-.6 1.7 1.7 0 0 0 .4-1.1V3a2 2 0 1 1 4 0v.09A1.7 1.7 0 0 0 15.4 4.6a1.7 1.7 0 0 0 1.88-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 19.4 9c.2.33.6.6 1 .6h.1a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1 .4Z" />
    </svg>
  );
}

function CollapseIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
      <path d="M12 4L4 12M4 4l8 8" />
    </svg>
  );
}

function EmptyIcon() {
  return (
    <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
      <circle cx="11" cy="11" r="7" />
      <path d="m20 20-3.5-3.5" strokeLinecap="round" />
      <path d="M8.5 11h5" strokeLinecap="round" />
    </svg>
  );
}

export function WidgetExpanded({
  repos,
  lastRefreshAt,
  refreshing,
  refreshError,
  onRefresh,
  onCollapse,
  onOpenSettings,
  allowDrag,
  onStartDrag,
  focusRepoId = null,
  onFocusRepoHandled,
}: {
  repos: RepoStatus[];
  lastRefreshAt: Date | null;
  refreshing: boolean;
  refreshError: string | null;
  onRefresh: () => void;
  onCollapse: () => void;
  onOpenSettings: () => void;
  allowDrag: boolean;
  onStartDrag: () => void;
  /** deep link 请求聚焦的仓库 id；处理后回调 onFocusRepoHandled */
  focusRepoId?: string | null;
  onFocusRepoHandled?: () => void;
}) {
  const [group, setGroup] = useState("全部分组");
  const [relation, setRelation] = useState<RemoteRelation | "all">("all");
  const [selectedRepoId, setSelectedRepoId] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const dragStart = useRef<{ x: number; y: number } | null>(null);

  const deferredQuery = useDeferredValue(query);
  const groupRepos = group === "全部分组" ? repos : repos.filter((repo) => repo.group === group);
  const visibleRepos = filterRepos(repos, group, relation, deferredQuery);
  const resultMotionKey = `${group}:${relation}:${deferredQuery.trim()}`;

  useEffect(() => {
    const nextFilters = reconcileExpandedFilters(repos, group, relation);
    if (nextFilters.group !== group) setGroup(nextFilters.group);
    if (nextFilters.relation !== relation) setRelation(nextFilters.relation);
    setSelectedRepoId((current) => current && repos.some((repo) => repo.id === current) ? current : null);
  }, [repos, group, relation]);

  // deep link 聚焦：重置筛选避免目标仓库被过滤掉，然后选中并展开其操作面板
  useEffect(() => {
    if (!focusRepoId) return;
    setGroup("全部分组");
    setRelation("all");
    setSelectedRepoId(repos.some((repo) => repo.id === focusRepoId) ? focusRepoId : null);
    onFocusRepoHandled?.();
  }, [focusRepoId, repos, onFocusRepoHandled]);

  function handleMouseDown(event: React.MouseEvent<HTMLElement>) {
    if (!shouldStartExpandedDrag(allowDrag, event.button, event.target)) return;
    dragStart.current = { x: event.clientX, y: event.clientY };
  }

  function handleMouseMove(event: React.MouseEvent<HTMLElement>) {
    if (!dragStart.current) return;
    const current = { x: event.clientX, y: event.clientY };
    if (!shouldPromoteExpandedDrag(allowDrag, dragStart.current, current)) return;
    dragStart.current = null;
    onStartDrag();
  }

  function clearDragStart() {
    dragStart.current = null;
  }

  function handleGroupSelect(nextGroup: string) {
    setGroup(nextGroup);
    setRelation((currentRelation) => reconcileRelationFilter(repos, nextGroup, currentRelation));
  }

  return (
    <section
      className="expanded-widget"
      onMouseDown={handleMouseDown}
      onMouseMove={handleMouseMove}
      onMouseUp={clearDragStart}
      onMouseLeave={clearDragStart}
      title={allowDrag ? uiStrings.expanded.dragHint : undefined}
    >
      <header className="widget-toolbar">
        <div>
          <h1>{uiStrings.expanded.title}</h1>
          <p>
            {lastRefreshAt
              ? `${uiStrings.expanded.refreshTimePrefix} ${lastRefreshAt.toLocaleTimeString("zh-CN", { hour12: false })}`
              : uiStrings.expanded.notRefreshed}
          </p>
        </div>
        <div className="search-control">
          <SearchIcon />
          <input
            aria-label={uiStrings.expanded.searchLabel}
            placeholder={uiStrings.expanded.searchPlaceholder}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
        </div>
        <div className="widget-toolbar-actions">
          <button className="collapse-btn refresh-btn" onClick={onRefresh} disabled={refreshing} aria-label={uiStrings.expanded.refreshActionLabel}>
            {refreshing && <span className="refresh-spinner" aria-hidden="true" />}
            {refreshing ? uiStrings.expanded.refreshing : uiStrings.expanded.refresh}
          </button>
          <button className="collapse-btn" onClick={onOpenSettings} aria-label={uiStrings.expanded.openSettings}>
            <SettingsIcon />
          </button>
          <button className="collapse-btn" onClick={onCollapse} aria-label={uiStrings.expanded.collapse}>
            <CollapseIcon />
          </button>
        </div>
      </header>
      {refreshError && (
        <p className="refresh-warning" role="status">
          {uiStrings.expanded.refreshFailedPrefix}：{refreshError}
        </p>
      )}
      <GroupFilters repos={repos} selected={group} onSelect={handleGroupSelect} />
      <StatusFilters repos={groupRepos} selected={relation} onSelect={setRelation} />
      <div className="repo-results" key={resultMotionKey}>
        {visibleRepos.length === 0 ? (
          <p className="repo-empty">
            <EmptyIcon />
            <span>{uiStrings.expanded.empty}</span>
          </p>
        ) : (
          <RepoTable repos={visibleRepos} selectedRepoId={selectedRepoId} onSelect={setSelectedRepoId} onRefresh={onRefresh} />
        )}
      </div>
    </section>
  );
}
