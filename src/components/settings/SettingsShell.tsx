import { useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { RepositorySettings } from "./RepositorySettings";
import { GroupSettings } from "./GroupSettings";
import { RefreshSettings } from "./RefreshSettings";
import { SafetySettings } from "./SafetySettings";
import { AppearanceSettings } from "./AppearanceSettings";
import { hasTauriRuntime } from "../../lib/runtime";
import { shouldStartWindowDrag } from "../../lib/windowDrag";

const sections = ["仓库设置", "常规设置"] as const;
type Section = (typeof sections)[number];

export function SettingsShell({ onClose }: { onClose?: () => void }) {
  const [active, setActive] = useState<Section>("仓库设置");

  function handleMouseDown(event: React.MouseEvent<HTMLElement>) {
    if (!hasTauriRuntime() || !shouldStartWindowDrag(event.target)) return;
    getCurrentWindow().startDragging().catch((err) => {
      console.error("启动窗口拖动失败", err);
    });
  }

  return (
    <section className="settings-window" onMouseDown={handleMouseDown}>
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
      </aside>
      <main className="settings-main">
        <div className="settings-topbar">
          <button className="settings-close" onClick={onClose}>完成</button>
        </div>
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
