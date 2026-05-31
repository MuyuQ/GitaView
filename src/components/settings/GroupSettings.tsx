import { useEffect, useState } from "react";
import { getSettings } from "../../lib/commands";
import { notifySettingsUpdated, subscribeToSettingsUpdates } from "../../lib/settingsEvents";
import { queueSettingsUpdate } from "../../lib/settingsMutations";
import type { AppSettings } from "../../types";

export function GroupSettings() {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [name, setName] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    getSettings()
      .then(setSettings)
      .catch((err) => setMessage(`加载分组失败：${err}`));
    return subscribeToSettingsUpdates(setSettings);
  }, []);

  async function persist(patch: (settings: AppSettings) => AppSettings, successMessage: string) {
    setBusy(true);
    setMessage(null);
    try {
      const savedSettings = await queueSettingsUpdate(patch);
      setSettings(savedSettings);
      notifySettingsUpdated(savedSettings);
      setMessage(successMessage);
    } catch (err) {
      setMessage(`保存失败：${err}`);
    } finally {
      setBusy(false);
    }
  }

  async function handleCreate() {
    if (!settings) return;
    const groupName = name.trim();
    if (!groupName) {
      setMessage("请输入分组名称");
      return;
    }
    if (settings.groups.some((group) => group.name === groupName)) {
      setMessage("这个分组已经存在");
      return;
    }
    await persist(
      (currentSettings) => ({
        ...currentSettings,
        groups: [...currentSettings.groups, { name: groupName, repoIds: [] }],
      }),
      "分组已创建",
    );
    setName("");
  }

  async function handleRemove(groupName: string) {
    if (!settings || groupName === settings.defaultGroup) return;
    await persist(
      (currentSettings) => ({
        ...currentSettings,
        groups: currentSettings.groups.filter((group) => group.name !== groupName),
        repos: currentSettings.repos.map((repo) =>
          repo.group === groupName ? { ...repo, group: currentSettings.defaultGroup } : repo,
        ),
      }),
      "分组已删除，相关仓库已移回默认分组",
    );
  }

  return (
    <section className="settings-card">
      <h3>分组管理</h3>
      <p>创建分组后，可以在上方仓库列表里把仓库分配进去。</p>
      <div className="settings-group-create">
        <input
          value={name}
          onChange={(event) => setName(event.target.value)}
          placeholder="例如：工作、开源、实验"
          aria-label="新分组名称"
        />
        <button className="primary" onClick={handleCreate} disabled={busy || !settings}>新增分组</button>
      </div>
      {message && <p className="settings-message" role="status">{message}</p>}
      <div className="settings-list">
        {settings?.groups.map((group) => (
          <article className="settings-group" key={group.name}>
            <strong>{group.name}</strong>
            <span>{settings.repos.filter((repo) => repo.group === group.name).length} 个仓库</span>
            <button onClick={() => handleRemove(group.name)} disabled={busy || group.name === settings.defaultGroup}>
              {group.name === settings.defaultGroup ? "默认" : "删除"}
            </button>
          </article>
        ))}
      </div>
    </section>
  );
}
