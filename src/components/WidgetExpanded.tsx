import { useState } from "react";
import { sortRepos } from "../lib/statusModel";
import type { RemoteRelation, RepoStatus } from "../types";
import { GroupFilters } from "./GroupFilters";
import { StatusFilters } from "./StatusFilters";
import { RepoTable } from "./RepoTable";

export function WidgetExpanded({ repos }: { repos: RepoStatus[] }) {
  const [group, setGroup] = useState("全部分组");
  const [relation, setRelation] = useState<RemoteRelation | "all">("all");
  const [selectedRepoId, setSelectedRepoId] = useState<string | null>(null);

  const groupRepos = group === "全部分组" ? repos : repos.filter((repo) => repo.group === group);
  const visibleRepos = sortRepos(
    relation === "all" ? groupRepos : groupRepos.filter((repo) => repo.relation === relation),
  );

  return (
    <section className="expanded-widget">
      <header className="widget-toolbar">
        <div>
          <h1>仓库状态</h1>
          <p>刚刚刷新</p>
        </div>
        <input aria-label="搜索或分组" placeholder="搜索或分组" />
      </header>
      <GroupFilters repos={repos} selected={group} onSelect={setGroup} />
      <StatusFilters repos={groupRepos} selected={relation} onSelect={setRelation} />
      <RepoTable repos={visibleRepos} selectedRepoId={selectedRepoId} onSelect={setSelectedRepoId} />
    </section>
  );
}
