import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { addRepository, getSettings, removeRepository, saveSettings, scanDirectory } from "../../lib/commands";
import type { AppSettings, GroupRecord, RepoRecord } from "../../types";

export function RepositorySettings() {
  const [repos, setRepos] = useState<RepoRecord[]>([]);
  const [groups, setGroups] = useState<GroupRecord[]>([]);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [path, setPath] = useState("");
  const [scanResults, setScanResults] = useState<string[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function reload() {
    const nextSettings = await getSettings();
    setSettings(nextSettings);
    setRepos(nextSettings.repos);
    setGroups(nextSettings.groups);
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
  }, []);

  async function handleScan() {
    if (!path.trim()) {
      setMessage("请输入要扫描的目录路径");
      return;
    }
    setBusy(true);
    setMessage(null);
    try {
      const results = await scanDirectory(path.trim());
      setScanResults(results);
      setMessage(`发现 ${results.length} 个仓库`);
    } catch (err) {
      setMessage(`扫描失败：${err}`);
    } finally {
      setBusy(false);
    }
  }

  async function handleAdd(targetPath = path.trim()) {
    if (!targetPath) {
      setMessage("请输入仓库路径");
      return;
    }
    setBusy(true);
    setMessage(null);
    try {
      await addRepository(targetPath);
      setPath("");
      await reload();
      setMessage("仓库已添加");
    } catch (err) {
      setMessage(`添加失败：${err}`);
    } finally {
      setBusy(false);
    }
  }

  async function handleRemove(repoId: string) {
    setBusy(true);
    setMessage(null);
    try {
      await removeRepository(repoId);
      await reload();
      setMessage("仓库已移除");
    } catch (err) {
      setMessage(`移除失败：${err}`);
    } finally {
      setBusy(false);
    }
  }

  async function handleGroupChange(repoId: string, groupName: string) {
    if (!settings) return;
    setBusy(true);
    setMessage(null);
    try {
      const nextRepos = settings.repos.map((repo) =>
        repo.id === repoId ? { ...repo, group: groupName } : repo,
      );
      const nextGroups = settings.groups.map((group) => ({
        ...group,
        repoIds:
          group.name === groupName
            ? Array.from(new Set([...group.repoIds, repoId]))
            : group.repoIds.filter((id) => id !== repoId),
      }));
      const nextSettings = { ...settings, repos: nextRepos, groups: nextGroups };
      await saveSettings(nextSettings);
      setSettings(nextSettings);
      setRepos(nextRepos);
      setGroups(nextGroups);
      setMessage("分组已更新");
    } catch (err) {
      setMessage(`更新分组失败：${err}`);
    } finally {
      setBusy(false);
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
        <h3>添加仓库</h3>
        <div className="settings-path-row">
          <button className="pick-dir-btn" onClick={handlePickDirectory} disabled={busy}>选择目录</button>
          <input
            value={path}
            onChange={(event) => setPath(event.target.value)}
            placeholder="输入或选择仓库目录"
          />
          <button onClick={handleScan} disabled={busy}>扫描目录</button>
          <button className="primary" onClick={() => handleAdd()} disabled={busy}>添加仓库</button>
        </div>
        {message && <p className="settings-message" role="status">{message}</p>}
        {scanResults.length > 0 && (
          <div className="settings-scan-results">
            {scanResults.map((result) => (
              <button key={result} onClick={() => handleAdd(result)} disabled={busy}>
                添加 {result}
              </button>
            ))}
          </div>
        )}
      </section>
      <section className="settings-card">
        <h3>已管理仓库</h3>
        {repos.length === 0 && <p className="settings-empty">还没有添加仓库</p>}
        {repos.map((repo) => (
          <article className="settings-repo" key={repo.id}>
            <strong>{repo.name}</strong>
            <span>{repo.path}</span>
            <select value={repo.group} onChange={(event) => handleGroupChange(repo.id, event.target.value)} disabled={busy}>
              {groups.map((group) => (
                <option key={group.name} value={group.name}>
                  {group.name}
                </option>
              ))}
            </select>
            <button onClick={() => handleRemove(repo.id)} disabled={busy}>移除</button>
          </article>
        ))}
      </section>
    </>
  );
}
