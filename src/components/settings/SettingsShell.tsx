import { useState } from "react";
import { RepositorySettings } from "./RepositorySettings";
import { GroupSettings } from "./GroupSettings";
import { RefreshSettings } from "./RefreshSettings";
import { SafetySettings } from "./SafetySettings";
import { AppearanceSettings } from "./AppearanceSettings";

const sections = ["仓库", "分组", "刷新", "安全操作", "外观"] as const;
type Section = (typeof sections)[number];

export function SettingsShell({ onClose }: { onClose?: () => void }) {
  const [active, setActive] = useState<Section>("仓库");

  return (
    <section className="settings-window">
      <aside className="settings-sidebar" aria-label="设置导航">
        <h1>设置</h1>
        {sections.map((section) => (
          <button
            key={section}
            className={section === active ? "settings-nav active" : "settings-nav"}
            onClick={() => setActive(section)}
          >
            {section}
          </button>
        ))}
      </aside>
      <main className="settings-main">
        <div className="settings-topbar">
          <button className="settings-close" onClick={onClose}>完成</button>
        </div>
        {active === "仓库" && <RepositorySettings />}
        {active === "分组" && <GroupSettings />}
        {active === "刷新" && <RefreshSettings />}
        {active === "安全操作" && <SafetySettings />}
        {active === "外观" && <AppearanceSettings />}
      </main>
    </section>
  );
}
