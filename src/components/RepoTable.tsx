import type { RepoStatus } from "../types";
import { RepoActions } from "./RepoActions";

const relationLabels: Record<RepoStatus["relation"], string> = {
  synced: "已同步",
  local_ahead: "本地领先",
  remote_ahead: "远程领先",
  diverged: "分叉",
  no_remote: "无远端",
};

const statusDotClass: Record<RepoStatus["relation"], string> = {
  synced: "green",
  local_ahead: "amber",
  remote_ahead: "amber",
  diverged: "red",
  no_remote: "slate",
};

export function RepoTable({ repos, selectedRepoId, onSelect }: { repos: RepoStatus[]; selectedRepoId: string | null; onSelect: (id: string | null) => void }) {
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
          {repos.map((repo) => (
            <>
              <tr
                key={repo.id}
                className={`repo-row ${selectedRepoId === repo.id ? "selected" : ""}`}
                onClick={() => onSelect(selectedRepoId === repo.id ? null : repo.id)}
              >
                <td className="col-status"><span className={`status-dot ${statusDotClass[repo.relation]}`} /></td>
                <td className="col-name">{repo.name}</td>
                <td className="col-group">{repo.group}</td>
                <td className="col-branch mono-light">{repo.branch}</td>
                <td className="col-relation mono">{relationLabels[repo.relation]}</td>
                <td className="col-changes mono">{repo.changeLabel}</td>
                <td className="col-hint">{repo.hint}</td>
              </tr>
              {selectedRepoId === repo.id && (
                <tr key={`${repo.id}-actions`}>
                  <td colSpan={7}>
                    <RepoActions repo={repo} />
                  </td>
                </tr>
              )}
            </>
          ))}
        </tbody>
      </table>
    </div>
  );
}
