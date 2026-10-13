import type { RepoStatus } from "../types";
import { RepoActions } from "./RepoActions";
import { Fragment, useState, useRef, useCallback, useEffect } from "react";
import type { CSSProperties } from "react";
import { nextSelectedRepoId } from "../lib/repoSelection";
import { uiStrings } from "../lib/strings";

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

  // 键盘调宽：左右方向键步进 16px，与鼠标拖拽等效
  const handleKeyDown = useCallback((columnKey: string, event: React.KeyboardEvent) => {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    const step = event.key === "ArrowLeft" ? -16 : 16;
    setColumnWidths((prev) => ({ ...prev, [columnKey]: Math.max(24, prev[columnKey] + step) }));
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

  const columns: Array<{ key: string; label: string }> = [
    { key: "status", label: uiStrings.table.status },
    { key: "name", label: uiStrings.table.name },
    { key: "group", label: uiStrings.table.group },
    { key: "branch", label: uiStrings.table.branch },
    { key: "relation", label: uiStrings.table.relation },
    { key: "changes", label: uiStrings.table.changes },
    { key: "hint", label: uiStrings.table.hint },
  ];

  const renderResizeHandle = (columnKey: string, label: string) => (
    <div
      className="col-resize-handle"
      role="separator"
      aria-orientation="vertical"
      aria-label={uiStrings.table.resizeHint(label)}
      tabIndex={0}
      onMouseDown={(event) => handleMouseDown(columnKey, event)}
      onKeyDown={(event) => handleKeyDown(columnKey, event)}
    />
  );

  return (
    <div className="repo-table">
      <table>
        <thead>
          <tr>
            {columns.map((column) => (
              <th key={column.key} scope="col" className={`col-${column.key}`} style={getColumnStyle(column.key)}>
                <div className="col-header">
                  <span>{column.label}</span>
                  {column.key !== "status" && renderResizeHandle(column.key, column.label)}
                </div>
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {repos.map((repo) => {
            const isExpanded = selectedRepoId === repo.id;
            const actionsPanelId = `repo-actions-${repo.id}`;
            const handleToggle = () => onSelect(nextSelectedRepoId(selectedRepoId, repo.id));

            return (
              <Fragment key={repo.id}>
                <tr
                  className={`repo-row ${isExpanded ? "selected" : ""}`}
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
                  <td className="col-status">
                    <span className={`status-dot ${statusDotClass[repo.relation]}`} aria-hidden="true" />
                    <span className="sr-only">{uiStrings.relation[repo.relation]}</span>
                  </td>
                  <td className="col-name">
                    <span className="repo-name-trigger">
                      <span className="repo-expand-indicator" aria-hidden="true">›</span>
                      <span className="repo-name-text">{repo.name}</span>
                    </span>
                  </td>
                  <td className="col-group">{repo.group}</td>
                  <td className="col-branch mono-light">{repo.branch}</td>
                  <td className="col-relation">
                    <span className={`relation-pill relation-${repo.relation}`}>
                      <span className={`status-dot ${statusDotClass[repo.relation]}`} aria-hidden="true" />
                      {uiStrings.relation[repo.relation]}
                    </span>
                  </td>
                  <td className="col-changes mono">{repo.changeLabel}</td>
                  <td className="col-hint">{repo.hint}</td>
                </tr>
                {isExpanded && (
                  <tr className="repo-actions-row">
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
