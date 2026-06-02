import { useRef } from "react";
import { showCollapsedNativeContextMenu } from "../lib/collapsedContextMenu";
import { summarizeActionableCollapsed } from "../lib/collapsedSummary";
import { shouldPromoteCollapsedDrag, shouldStartCollapsedDrag } from "../lib/windowDrag";
import type { RepoStatus } from "../types";

export function WidgetCollapsed({
  repos,
  allowDrag,
  onExpand,
  onStartDrag,
  onRefresh,
  onExit,
}: {
  repos: RepoStatus[];
  allowDrag: boolean;
  onExpand: () => void;
  onStartDrag: () => void;
  onRefresh: () => void;
  onExit: () => void;
}) {
  const dragStart = useRef<{ x: number; y: number } | null>(null);
  const suppressNextClick = useRef(false);
  const summary = summarizeActionableCollapsed(repos);
  const colorClass = {
    syncable: "status-dot amber",
    needs_attention: "status-dot red",
    no_remote: "status-dot slate",
  } as const;

  function handlePointerDown(event: React.PointerEvent<HTMLButtonElement>) {
    if (!shouldStartCollapsedDrag(allowDrag, event.button)) return;
    dragStart.current = { x: event.clientX, y: event.clientY };
    suppressNextClick.current = false;
  }

  function clearDragStart() {
    dragStart.current = null;
  }

  function handlePointerMove(event: React.PointerEvent<HTMLButtonElement>) {
    const current = { x: event.clientX, y: event.clientY };
    if (!shouldPromoteCollapsedDrag(allowDrag, dragStart.current, current)) return;
    suppressNextClick.current = true;
    dragStart.current = null;
    onStartDrag();
  }

  function handleClick(event: React.MouseEvent<HTMLButtonElement>) {
    if (suppressNextClick.current) {
      event.preventDefault();
      event.stopPropagation();
      suppressNextClick.current = false;
      return;
    }
    onExpand();
  }

  function handleContextMenu(event: React.MouseEvent<HTMLButtonElement>) {
    event.preventDefault();
    void showCollapsedNativeContextMenu(
      { x: event.clientX, y: event.clientY },
      { onRefresh, onExit },
    ).catch((err) => {
      console.error("打开收缩态右键菜单失败", err);
    });
  }

  return (
    <div className="collapsed-widget-wrap">
      <button
        className="collapsed-widget"
        onClick={handleClick}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={clearDragStart}
        onPointerCancel={clearDragStart}
        onContextMenu={handleContextMenu}
        aria-label="展开仓库状态"
        title={allowDrag ? "点击展开，拖动移动，右键菜单" : "点击展开，右键菜单"}
      >
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
      </button>
    </div>
  );
}
