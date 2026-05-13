import { useEffect, useState } from "react";
import type { RepoStatus } from "../types";
import { fetchRepo, getSettings, pullRepo, pushRepo, openRepoDirectory, openRepoRemote } from "../lib/commands";
import { getRepoActionAvailability } from "../lib/statusModel";

export function RepoActions({ repo, onRefresh }: { repo: RepoStatus; onRefresh: () => void }) {
  const [loading, setLoading] = useState<string | null>(null);
  const [result, setResult] = useState<string | null>(null);
  const [confirmPull, setConfirmPull] = useState(false);
  const [confirmPush, setConfirmPush] = useState(false);
  const [requiresPullConfirm, setRequiresPullConfirm] = useState(true);
  const [requiresPushConfirm, setRequiresPushConfirm] = useState(true);

  useEffect(() => {
    getSettings()
      .then((settings) => {
        setRequiresPullConfirm(settings.safety.confirmPull);
        setRequiresPushConfirm(settings.safety.confirmPush);
      })
      .catch(() => {
        setRequiresPullConfirm(true);
        setRequiresPushConfirm(true);
      });
  }, []);

  async function runAction(action: string, fn: () => Promise<string | void>) {
    setLoading(action);
    setResult(null);
    try {
      const res = await fn();
      setResult(typeof res === "string" ? res : `${action} 已完成`);
      const shouldRefresh = action === "Fetch" || action === "Pull" || action === "Push";
      if (shouldRefresh) onRefresh();
    } catch (err) {
      setResult(`${action} 失败：${err}`);
    } finally {
      setLoading(null);
    }
  }

  function handlePull() {
    if (!requiresPullConfirm || confirmPull) {
      runAction("Pull", () => pullRepo(repo.id, true));
      setConfirmPull(false);
    } else {
      setConfirmPull(true);
    }
  }

  function handlePush() {
    if (!requiresPushConfirm || confirmPush) {
      runAction("Push", () => pushRepo(repo.id, true));
      setConfirmPush(false);
    } else {
      setConfirmPush(true);
    }
  }

  const actions = getRepoActionAvailability(repo);

  return (
    <div className="repo-actions" onClick={(event) => event.stopPropagation()}>
      <button
        className="action-btn"
        onClick={() => runAction("目录", () => openRepoDirectory(repo.id))}
        disabled={loading !== null || !actions.canOpenDirectory}
        title="在文件管理器中打开仓库目录"
      >
        {loading === "目录" ? "加载中..." : "目录"}
      </button>
      <button
        className="action-btn"
        onClick={() => runAction("远端", () => openRepoRemote(repo.id))}
        disabled={loading !== null || !actions.canOpenRemote}
        title={repo.remoteUrl ? "在浏览器中打开远端仓库页面" : "未配置远端仓库"}
      >
        {loading === "远端" ? "加载中..." : "远端"}
      </button>
      <button
        className="action-btn"
        onClick={() => runAction("Fetch", () => fetchRepo(repo.id))}
        disabled={loading !== null || !actions.canFetch}
        title="从远端获取最新分支信息"
      >
        {loading === "Fetch" ? "加载中..." : "Fetch"}
      </button>
      {actions.showPush && (
        <button
          className={`action-btn push-btn ${confirmPush ? "confirm" : ""}`}
          onClick={handlePush}
          disabled={loading !== null}
          title="将本地提交推送到远端"
        >
          {loading === "Push" ? "加载中..." : confirmPush ? "确认 Push" : "Push"}
        </button>
      )}
      {actions.showPull && (
        <button
          className={`action-btn pull-btn ${confirmPull ? "confirm" : ""}`}
          onClick={handlePull}
          disabled={loading !== null}
          title="从远端拉取更新并合并到本地"
        >
          {loading === "Pull" ? "加载中..." : confirmPull ? "确认 Pull" : "Pull"}
        </button>
      )}
      {requiresPullConfirm && confirmPull && (
        <span className="action-warning">Pull 会修改当前仓库工作区，是否继续？</span>
      )}
      {requiresPushConfirm && confirmPush && (
        <span className="action-warning">Push 会更新远端分支，是否继续？</span>
      )}
      {result && <span className="action-result">{result}</span>}
    </div>
  );
}
