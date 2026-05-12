import type { RepoStatus } from "../types";
import { RepoActions } from "./RepoActions";
import { Fragment } from "react";
import type { CSSProperties } from "react";
import { nextSelectedRepoId } from "../lib/repoSelection";

const relationLabels: Record<RepoStatus["relation"], string> = {
  error: "读取失败",
  synced: "已同步",
  local_ahead: "本地领先",
  remote_ahead: "远程领先",
  diverged: "分叉",
  no_remote: "无远端",
};

const statusDotClass: Record<RepoStatus["relation"], string> = {
  error: "red",
  synced: "green",
  local_ahead: "amber",
  remote_ahead: "amber",
  diverged: "red",
  no_remote: "slate",
};

export function RepoTable({ repos, selectedRepoId, onSelect, onRefresh }: { repos: RepoStatus[]; selectedRepoId: string | null; onSelect: (id: string | null) => void; onRefresh: () => void }) {
  return (
    <div className="repo-table">
      <table>
        <thead>
          <tr>
            <th className="col-status">状态</th>
            <th className="col-name">仓库</th>
            <th className="col-group">分类</th>
            <th className="col-branch">分支</th>
            <th className="col-relation">关系</th>
            <th className="col-changes">变更</th>
            <th className="col-hint">提示</th>
          </tr>
        </thead>
        <tbody>
          {repos.map((repo, index) => {
            const isExpanded = selectedRepoId === repo.id;
            const actionsPanelId = `repo-actions-${repo.id}`;
            const handleToggle = () => onSelect(nextSelectedRepoId(selectedRepoId, repo.id));

            return (
              <Fragment key={repo.id}>
                <tr
                  className={`repo-row ${isExpanded ? "selected" : ""}`}
                  style={{ animationDelay: `${Math.min(index, 8) * 16}ms` } as CSSProperties}
                  onClick={handleToggle}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault();
                      handleToggle();
                    }
                  }}
                  role="button"
                  tabIndex={0}
                  aria-expanded={isExpanded}
                  aria-controls={actionsPanelId}
                >
                  <td className="col-status"><span className={`status-dot ${statusDotClass[repo.relation]}`} /></td>
                  <td className="col-name">
                    <span className="repo-name-trigger">
                      <span className="repo-expand-indicator" aria-hidden="true">›</span>
                      <span className="repo-name-text">{repo.name}</span>
                    </span>
                  </td>
                  <td className="col-group">{repo.group}</td>
                  <td className="col-branch mono-light">{repo.branch}</td>
                  <td className="col-relation mono">{relationLabels[repo.relation]}</td>
                  <td className="col-changes mono">{repo.changeLabel}</td>
                  <td className="col-hint">{repo.hint}</td>
                </tr>
                {isExpanded && (
                  <tr className="repo-actions-row" key={`${repo.id}-actions`}>
                    <td colSpan={7}>
                      <div id={actionsPanelId} className="repo-actions-panel" onClick={(event) => event.stopPropagation()}>
                        <RepoActions repo={repo} onRefresh={onRefresh} />
                      </div>
                    </td>
                  </tr>
                )}
              </Fragment>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
