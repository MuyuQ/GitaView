import { useRef, useState } from "react";
import type { ReactElement } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { RepositorySettings } from "./RepositorySettings";
import { GroupSettings } from "./GroupSettings";
import { RefreshSettings } from "./RefreshSettings";
import { SafetySettings } from "./SafetySettings";
import { AppearanceSettings } from "./AppearanceSettings";
import { hasTauriRuntime } from "../../lib/runtime";
import { shouldStartWindowDrag, shouldPromoteExpandedDrag } from "../../lib/windowDrag";

const sections = ["仓库设置", "常规设置"] as const;
type Section = (typeof sections)[number];

function RepositoryIcon() {
  return (
    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden="true">
      <path d="M6 3h12a1 1 0 0 1 1 1v16l-3.5-2.5L12 20l-3.5-2.5L5 20V4a1 1 0 0 1 1-1Z" />
      <path d="M9 8h6" />
    </svg>
  );
}

function GeneralIcon() {
  return (
    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden="true">
      <path d="M4 7h9M17 7h3M4 17h3M11 17h9" />
      <circle cx="15" cy="7" r="2" />
      <circle cx="9" cy="17" r="2" />
    </svg>
  );
}

const sectionIcons: Record<Section, () => ReactElement> = {
  仓库设置: RepositoryIcon,
  常规设置: GeneralIcon,
};

export function SettingsShell({ onClose }: { onClose?: () => void }) {
  const [active, setActive] = useState<Section>("仓库设置");
  const dragStart = useRef<{ x: number; y: number } | null>(null);

  function handleMouseDown(event: React.MouseEvent<HTMLElement>) {
    if (!hasTauriRuntime() || !shouldStartWindowDrag(event.target)) return;
    dragStart.current = { x: event.clientX, y: event.clientY };
  }

  function handleMouseMove(event: React.MouseEvent<HTMLElement>) {
    if (!dragStart.current) return;
    const current = { x: event.clientX, y: event.clientY };
    if (!shouldPromoteExpandedDrag(true, dragStart.current, current)) return;
    dragStart.current = null;
    getCurrentWindow().startDragging().catch((err) => {
      console.error("启动窗口拖动失败", err);
    });
  }

  function clearDragStart() {
    dragStart.current = null;
  }

  return (
    <section
      className="settings-window"
      onMouseDown={handleMouseDown}
      onMouseMove={handleMouseMove}
      onMouseUp={clearDragStart}
      onMouseLeave={clearDragStart}
    >
      <aside className="settings-sidebar" aria-label="设置导航">
        <div className="settings-brand">
          <span className="settings-brand-kicker">GitaView</span>
          <h1>设置</h1>
        </div>
        <nav className="settings-nav-list">
          {sections.map((section) => {
            const Icon = sectionIcons[section];
            return (
              <button
                key={section}
                className={section === active ? "settings-nav active" : "settings-nav"}
                onClick={() => setActive(section)}
                aria-current={section === active ? "page" : undefined}
              >
                <span className="settings-nav-icon" aria-hidden="true"><Icon /></span>
                <span>{section}</span>
              </button>
            );
          })}
        </nav>
        <div className="settings-sidebar-footer">
          <button className="settings-save-btn" onClick={onClose}>关闭</button>
        </div>
      </aside>
      <main className="settings-main">
        {active === "仓库设置" && (
          <>
            <RepositorySettings />
            <GroupSettings />
          </>
        )}
        {active === "常规设置" && (
          <>
            <RefreshSettings />
            <SafetySettings />
            <AppearanceSettings />
          </>
        )}
      </main>
    </section>
  );
}
