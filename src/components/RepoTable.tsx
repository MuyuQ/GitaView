import type { RepoStatus } from "../types";
import { RepoActions } from "./RepoActions";
import { Fragment, useState, useRef, useCallback, useEffect } from "react";
import type { CSSProperties } from "react";
import { nextSelectedRepoId } from "../lib/repoSelection";

const relationLabels: Record<RepoStatus["relation"], string> = {
  error: "读取失败",
  synced: "已同步",
  local_ahead: "本地领先",
  remote_ahead: "远程领先",
  diverged: "分叉",
  no_remote: "无远端",
};

const statusDotClass: Record<RepoStatus["relation"], string> = {
  error: "red",
  synced: "green",
  local_ahead: "amber",
  remote_ahead: "amber",
  diverged: "red",
  no_remote: "slate",
};

const defaultColumnWidths: Record<string, number> = {
  status: 24,
  name: 140,
  group: 60,
  branch: 80,
  relation: 80,
  changes: 50,
  hint: 80,
};

export function RepoTable({ repos, selectedRepoId, onSelect, onRefresh }: { repos: RepoStatus[]; selectedRepoId: string | null; onSelect: (id: string | null) => void; onRefresh: () => void }) {
  const [columnWidths, setColumnWidths] = useState<Record<string, number>>(defaultColumnWidths);
  const [draggingColumn, setDraggingColumn] = useState<string | null>(null);
  const dragStartX = useRef<number>(0);
  const dragStartWidth = useRef<number>(0);

  const handleMouseDown = useCallback((columnKey: string, event: React.MouseEvent) => {
    event.preventDefault();
    setDraggingColumn(columnKey);
    dragStartX.current = event.clientX;
    dragStartWidth.current = columnWidths[columnKey];
  }, [columnWidths]);

  const handleMouseMove = useCallback((event: MouseEvent) => {
    if (!draggingColumn) return;
    const delta = event.clientX - dragStartX.current;
    const newWidth = Math.max(24, dragStartWidth.current + delta);
    setColumnWidths(prev => ({ ...prev, [draggingColumn]: newWidth }));
  }, [draggingColumn]);

  const handleMouseUp = useCallback(() => {
    setDraggingColumn(null);
  }, []);

  useEffect(() => {
    if (!draggingColumn) return;

    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);

    return () => {
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUp);
    };
  }, [draggingColumn, handleMouseMove, handleMouseUp]);

  const getColumnStyle = (key: string): CSSProperties => ({
    width: columnWidths[key],
    minWidth: key === "name" ? 60 : 24,
  });

  return (
    <div className="repo-table">
      <table>
        <thead>
          <tr>
            <th className="col-status" style={getColumnStyle("status")}>状态</th>
            <th className="col-name" style={getColumnStyle("name")}>
              <div className="col-header">
                <span>仓库</span>
                <div className="col-resize-handle" onMouseDown={(e) => handleMouseDown("name", e)} />
              </div>
            </th>
            <th className="col-group" style={getColumnStyle("group")}>
              <div className="col-header">
                <span>分类</span>
                <div className="col-resize-handle" onMouseDown={(e) => handleMouseDown("group", e)} />
              </div>
            </th>
            <th className="col-branch" style={getColumnStyle("branch")}>
              <div className="col-header">
                <span>分支</span>
                <div className="col-resize-handle" onMouseDown={(e) => handleMouseDown("branch", e)} />
              </div>
            </th>
            <th className="col-relation" style={getColumnStyle("relation")}>
              <div className="col-header">
                <span>关系</span>
                <div className="col-resize-handle" onMouseDown={(e) => handleMouseDown("relation", e)} />
              </div>
            </th>
            <th className="col-changes" style={getColumnStyle("changes")}>
              <div className="col-header">
                <span>变更</span>
                <div className="col-resize-handle" onMouseDown={(e) => handleMouseDown("changes", e)} />
              </div>
            </th>
            <th className="col-hint" style={getColumnStyle("hint")}>
              <div className="col-header">
                <span>提示</span>
                <div className="col-resize-handle" onMouseDown={(e) => handleMouseDown("hint", e)} />
              </div>
            </th>
          </tr>
        </thead>
        <tbody>
          {repos.map((repo, index) => {
            const isExpanded = selectedRepoId === repo.id;
            const actionsPanelId = `repo-actions-${repo.id}`;
            const handleToggle = () => onSelect(nextSelectedRepoId(selectedRepoId, repo.id));

            return (
              <Fragment key={repo.id}>
                <tr
                  className={`repo-row ${isExpanded ? "selected" : ""}`}
                  style={{ animationDelay: `${Math.min(index, 8) * 16}ms` } as CSSProperties}
                  onClick={handleToggle}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault();
                      handleToggle();
                    }
                  }}
                  role="button"
                  tabIndex={0}
                  aria-expanded={isExpanded}
                  aria-controls={actionsPanelId}
                >
                  <td className="col-status"><span className={`status-dot ${statusDotClass[repo.relation]}`} /></td>
                  <td className="col-name">
                    <span className="repo-name-trigger">
                      <span className="repo-expand-indicator" aria-hidden="true">›</span>
                      <span className="repo-name-text">{repo.name}</span>
                    </span>
                  </td>
                  <td className="col-group">{repo.group}</td>
                  <td className="col-branch mono-light">{repo.branch}</td>
                  <td className="col-relation mono">{relationLabels[repo.relation]}</td>
                  <td className="col-changes mono">{repo.changeLabel}</td>
                  <td className="col-hint">{repo.hint}</td>
                </tr>
                {isExpanded && (
                  <tr className="repo-actions-row" key={`${repo.id}-actions`}>
                    <td colSpan={7}>
                      <div id={actionsPanelId} className="repo-actions-panel" onClick={(event) => event.stopPropagation()}>
                        <RepoActions repo={repo} onRefresh={onRefresh} />
                      </div>
                    </td>
                  </tr>
                )}
              </Fragment>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
