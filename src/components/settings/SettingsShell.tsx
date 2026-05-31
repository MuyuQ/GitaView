import { useRef, useState } from "react";
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
          {sections.map((section) => (
            <button
              key={section}
              className={section === active ? "settings-nav active" : "settings-nav"}
              onClick={() => setActive(section)}
            >
              <span>{section}</span>
            </button>
          ))}
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
