import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { addRepository, getSettings, openRepoDirectory, removeRepository, scanDirectory } from "../../lib/commands";
import { getRepositoryDisplayText, nextRevealedRepoId } from "../../lib/repositorySettingsView";
import { notifySettingsUpdated, subscribeToSettingsUpdates } from "../../lib/settingsEvents";
import { queueSettingsUpdate } from "../../lib/settingsMutations";
import type { AppSettings, GroupRecord, RepoRecord } from "../../types";

export function RepositorySettings() {
  const [repos, setRepos] = useState<RepoRecord[]>([]);
  const [groups, setGroups] = useState<GroupRecord[]>([]);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [path, setPath] = useState("");
  const [scanResults, setScanResults] = useState<string[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [scanBusy, setScanBusy] = useState(false);
  const [addBusy, setAddBusy] = useState(false);
  const [repoActionBusyId, setRepoActionBusyId] = useState<string | null>(null);
  const [revealedRepoId, setRevealedRepoId] = useState<string | null>(null);

  function applySettings(nextSettings: AppSettings) {
    setSettings(nextSettings);
    setRepos(nextSettings.repos);
    setGroups(nextSettings.groups);
  }

  async function reload() {
    const nextSettings = await getSettings();
    applySettings(nextSettings);
    return nextSettings;
  }

  async function handlePickDirectory() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "选择仓库或扫描根目录",
    });
    if (typeof selected === "string") {
      setPath(selected);
    }
  }

  useEffect(() => {
    reload().catch((err) => setMessage(`加载设置失败：${err}`));
    return subscribeToSettingsUpdates(applySettings);
  }, []);

  async function handleScan() {
    if (!path.trim()) {
      setMessage("请输入要扫描的目录路径");
      return;
    }
    setScanBusy(true);
    setMessage(null);
    try {
      const results = await scanDirectory(path.trim());
      setScanResults(results);
      setMessage(`发现 ${results.length} 个仓库`);
    } catch (err) {
      setMessage(`扫描失败：${err}`);
    } finally {
      setScanBusy(false);
    }
  }

  async function handleAdd(targetPath = path.trim()) {
    if (!targetPath) {
      setMessage("请输入仓库路径");
      return;
    }
    setAddBusy(true);
    setMessage(null);
    try {
      await addRepository(targetPath);
      setPath("");
      setScanResults((prev) => prev.filter((p) => p !== targetPath));
      const nextSettings = await reload();
      notifySettingsUpdated(nextSettings);
      setMessage("仓库已添加");
    } catch (err) {
      setMessage(`添加失败：${err}`);
    } finally {
      setAddBusy(false);
    }
  }

  async function handleRemove(repoId: string) {
    if (repoActionBusyId) return;
    setRepoActionBusyId(repoId);
    setMessage(null);
    try {
      await removeRepository(repoId);
      setRevealedRepoId((current) => (current === repoId ? null : current));
      const nextSettings = await reload();
      notifySettingsUpdated(nextSettings);
      setMessage("仓库已移除");
    } catch (err) {
      setMessage(`移除失败：${err}`);
    } finally {
      setRepoActionBusyId(null);
    }
  }

  async function handleOpenDirectory(repoId: string) {
    if (repoActionBusyId) return;
    setRepoActionBusyId(repoId);
    setMessage(null);
    try {
      await openRepoDirectory(repoId);
      setMessage("已打开目录");
    } catch (err) {
      setMessage(`打开失败：${err}`);
    } finally {
      setRepoActionBusyId(null);
    }
  }

  async function handleGroupChange(repoId: string, groupName: string) {
    if (!settings) return;
    if (repoActionBusyId) return;
    setRepoActionBusyId(repoId);
    setMessage(null);
    try {
      const savedSettings = await queueSettingsUpdate((currentSettings) => {
        const nextRepos = currentSettings.repos.map((repo) =>
          repo.id === repoId ? { ...repo, group: groupName } : repo,
        );
        const nextGroups = currentSettings.groups.map((group) => ({
          ...group,
          repoIds:
            group.name === groupName
              ? Array.from(new Set([...group.repoIds, repoId]))
              : group.repoIds.filter((id) => id !== repoId),
        }));
        return { ...currentSettings, repos: nextRepos, groups: nextGroups };
      });
      applySettings(savedSettings);
      notifySettingsUpdated(savedSettings);
      setMessage("分组已更新");
    } catch (err) {
      setMessage(`更新分组失败：${err}`);
    } finally {
      setRepoActionBusyId(null);
    }
  }

  return (
    <>
      <header className="settings-header">
        <div>
          <h2>仓库管理</h2>
          <p>扫描目录、手动添加仓库，并为仓库分配业务分组。</p>
        </div>
      </header>
      <section className="settings-card">
        <h3>已管理仓库</h3>
        {repos.length === 0 && (
          <div className="settings-empty-state">
            <strong>还没有添加仓库</strong>
            <p>选择一个目录扫描，或者直接输入 Git 仓库路径。添加后这里会显示分组和管理操作。</p>
          </div>
        )}
        {repos.map((repo) => {
          const displayText = getRepositoryDisplayText(repo, revealedRepoId);
          const isPathRevealed = revealedRepoId === repo.id;
          const repoActionsBusy = repoActionBusyId !== null;
          return (
            <article className="settings-repo" key={repo.id}>
              <button
                className={`settings-repo-info ${isPathRevealed ? "is-path" : ""}`}
                type="button"
                title={repo.path}
                aria-label={isPathRevealed ? `${repo.name} 的仓库路径` : `显示 ${repo.name} 的仓库路径`}
                onClick={() => setRevealedRepoId((current) => nextRevealedRepoId(current, repo.id))}
              >
                <span>{displayText}</span>
              </button>
              <select
                value={repo.group}
                onChange={(event) => handleGroupChange(repo.id, event.target.value)}
                disabled={repoActionsBusy}
                aria-label={`${repo.name} 的仓库分组`}
              >
                {groups.map((group) => (
                  <option key={group.name} value={group.name}>
                    {group.name}
                  </option>
                ))}
              </select>
              <div className="settings-repo-actions">
                <button
                  type="button"
                  onClick={() => handleOpenDirectory(repo.id)}
                  disabled={repoActionsBusy}
                  title="用资源管理器或访达打开仓库目录"
                  aria-label={`用资源管理器或访达打开 ${repo.name} 的仓库目录`}
                >
                  打开
                </button>
                <button type="button" onClick={() => handleRemove(repo.id)} disabled={repoActionsBusy}>移除</button>
              </div>
            </article>
          );
        })}
      </section>
      <section className="settings-card settings-card-compact">
        <h3>添加仓库</h3>
        <div className="settings-add-repo">
          <input
            value={path}
            onChange={(event) => setPath(event.target.value)}
            placeholder="输入或选择仓库目录"
            aria-label="仓库目录"
          />
          <div className="settings-add-actions">
            <button className="pick-dir-btn" onClick={handlePickDirectory} disabled={scanBusy}>选择目录</button>
            <button onClick={handleScan} disabled={scanBusy}>扫描目录</button>
            <button className="primary" onClick={() => handleAdd()} disabled={addBusy}>添加仓库</button>
          </div>
        </div>
        {message && <p className="settings-message" role="status">{message}</p>}
        {scanResults.length > 0 && (
          <div className="settings-scan-results">
            {scanResults.map((result) => (
              <button key={result} onClick={() => handleAdd(result)} disabled={addBusy}>
                添加 {result}
              </button>
            ))}
          </div>
        )}
      </section>
    </>
  );
}
