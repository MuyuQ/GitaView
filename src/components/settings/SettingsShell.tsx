import { useState } from "react";
import { RepositorySettings } from "./RepositorySettings";

const sections = ["仓库", "分组", "刷新", "安全操作", "外观"] as const;
type Section = (typeof sections)[number];

export function SettingsShell() {
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
        {active === "仓库" ? <RepositorySettings /> : <div className="settings-placeholder">{active}</div>}
      </main>
    </section>
  );
}
