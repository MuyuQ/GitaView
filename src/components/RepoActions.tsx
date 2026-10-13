import { useState } from "react";
import type { RepoStatus } from "../types";
import { fetchRepo, pullRepo, pushRepo, openRepoDirectory, openRepoRemote } from "../lib/commands";
import { getRepoActionAvailability } from "../lib/statusModel";
import { uiStrings } from "../lib/strings";
import {
  actionResultClassName,
  actionResultRole,
  formatActionResult,
  type ActionResult,
} from "../lib/actionResults";

export function RepoActions({ repo, onRefresh }: { repo: RepoStatus; onRefresh: () => void }) {
  const [loading, setLoading] = useState<string | null>(null);
  const [result, setResult] = useState<ActionResult | null>(null);
  const [confirmPull, setConfirmPull] = useState(false);
  const [confirmPush, setConfirmPush] = useState(false);

  async function runAction(action: string, fn: () => Promise<string | void>) {
    setLoading(action);
    setResult(null);
    try {
      const res = await fn();
      setResult(formatActionResult(action, { ok: true, message: typeof res === "string" ? res : undefined }));
      const shouldRefresh = action === "Fetch" || action === "Pull" || action === "Push";
      if (shouldRefresh) onRefresh();
    } catch (err) {
      setResult(formatActionResult(action, { ok: false, error: err }));
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

  function handlePush() {
    if (confirmPush) {
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
        onClick={() => runAction(uiStrings.actions.directory, () => openRepoDirectory(repo.id))}
        disabled={loading !== null || !actions.canOpenDirectory}
        title={uiStrings.actions.directoryTitle}
      >
        {loading === uiStrings.actions.directory ? uiStrings.actions.loading : uiStrings.actions.directory}
      </button>
      <button
        className="action-btn"
        onClick={() => runAction(uiStrings.actions.remote, () => openRepoRemote(repo.id))}
        disabled={loading !== null || !actions.canOpenRemote}
        title={repo.remoteUrl ? uiStrings.actions.remoteTitle : uiStrings.actions.remoteMissingTitle}
      >
        {loading === uiStrings.actions.remote ? uiStrings.actions.loading : uiStrings.actions.remote}
      </button>
      <button
        className="action-btn"
        onClick={() => runAction(uiStrings.actions.fetch, () => fetchRepo(repo.id))}
        disabled={loading !== null || !actions.canFetch}
        title={uiStrings.actions.fetchTitle}
      >
        {loading === uiStrings.actions.fetch ? uiStrings.actions.loading : uiStrings.actions.fetch}
      </button>
      {actions.showPush && (
        <button
          className={`action-btn push-btn ${confirmPush ? "confirm" : ""}`}
          onClick={handlePush}
          disabled={loading !== null}
          title={uiStrings.actions.pushTitle}
        >
          {loading === uiStrings.actions.push ? uiStrings.actions.loading : confirmPush ? uiStrings.actions.confirmPush : uiStrings.actions.push}
        </button>
      )}
      {actions.showPull && (
        <button
          className={`action-btn pull-btn ${confirmPull ? "confirm" : ""}`}
          onClick={handlePull}
          disabled={loading !== null}
          title={uiStrings.actions.pullTitle}
        >
          {loading === uiStrings.actions.pull ? uiStrings.actions.loading : confirmPull ? uiStrings.actions.confirmPull : uiStrings.actions.pull}
        </button>
      )}
      {confirmPull && (
        <span className="action-warning">{uiStrings.actions.pullWarning}</span>
      )}
      {confirmPush && (
        <span className="action-warning">{uiStrings.actions.pushWarning}</span>
      )}
      {result && (
        <span className={actionResultClassName(result.kind)} role={actionResultRole(result.kind)}>
          {result.text}
        </span>
      )}
    </div>
  );
}
