import { useState } from "react";
import type { RepoStatus } from "../types";
import { fetchRepo, pullRepo, openRepoDirectory, openRepoRemote } from "../lib/commands";

export function RepoActions({ repo }: { repo: RepoStatus }) {
  const [loading, setLoading] = useState<string | null>(null);
  const [result, setResult] = useState<string | null>(null);
  const [confirmPull, setConfirmPull] = useState(false);

  async function runAction(action: string, fn: () => Promise<string | void>) {
    setLoading(action);
    setResult(null);
    try {
      const res = await fn();
      setResult(typeof res === "string" ? res : `${action} 已完成`);
    } catch (err) {
      setResult(`${action} 失败：${err}`);
    } finally {
      setLoading(null);
    }
  }

  function handlePull() {
    if (confirmPull) {
      runAction("Pull", () => pullRepo(repo.id, true));
      setConfirmPull(false);
    } else {
      setConfirmPull(true);
    }
  }

  return (
    <div className="repo-actions">
      <button className="action-btn" onClick={() => runAction("目录", () => openRepoDirectory(repo.id))} disabled={loading !== null}>
        {loading === "目录" ? "加载中..." : "目录"}
      </button>
      <button className="action-btn" onClick={() => runAction("远端", () => openRepoRemote(repo.id))} disabled={loading !== null || !repo.remoteUrl}>
        {loading === "远端" ? "加载中..." : "远端"}
      </button>
      <button className="action-btn" onClick={() => runAction("Fetch", () => fetchRepo(repo.id))} disabled={loading !== null}>
        {loading === "Fetch" ? "加载中..." : "Fetch"}
      </button>
      <button className={`action-btn pull-btn ${confirmPull ? "confirm" : ""}`} onClick={handlePull} disabled={loading !== null}>
        {loading === "Pull" ? "加载中..." : confirmPull ? "确认 Pull？" : "Pull"}
      </button>
      {result && <span className="action-result">{result}</span>}
    </div>
  );
}
