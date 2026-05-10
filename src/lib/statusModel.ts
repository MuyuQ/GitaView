import type { CollapsedBucket, RemoteRelation, RepoStatus } from "../types";

const expandedRank: Record<RemoteRelation, number> = {
  error: 0,
  diverged: 1,
  remote_ahead: 2,
  local_ahead: 3,
  synced: 4,
  no_remote: 5,
};

const bucketRank: CollapsedBucket[] = [
  "synced",
  "syncable",
  "needs_attention",
  "no_remote",
];

export function toCollapsedBucket(relation: RemoteRelation): CollapsedBucket {
  if (relation === "error" || relation === "diverged") return "needs_attention";
  if (relation === "synced") return "synced";
  if (relation === "no_remote") return "no_remote";
  return "syncable";
}

export function summarizeCollapsed(repos: RepoStatus[]) {
  return bucketRank.map((bucket) => ({
    bucket,
    count: repos.filter((repo) => toCollapsedBucket(repo.relation) === bucket).length,
  }));
}

export function sortRepos(repos: RepoStatus[]) {
  return [...repos].sort((a, b) => {
    const rankDiff = expandedRank[a.relation] - expandedRank[b.relation];
    return rankDiff === 0 ? a.name.localeCompare(b.name, "zh-Hans-CN") : rankDiff;
  });
}

export function filterRepos(
  repos: RepoStatus[],
  group: string,
  relation: RemoteRelation | "all",
  query: string,
): RepoStatus[] {
  const groupRepos = group === "全部分组" ? repos : repos.filter((repo) => repo.group === group);
  const relationRepos = relation === "all" ? groupRepos : groupRepos.filter((repo) => repo.relation === relation);
  const normalizedQuery = query.trim().toLowerCase();
  const searchedRepos = normalizedQuery
    ? relationRepos.filter((repo) =>
        [repo.name, repo.path, repo.branch, repo.group].join(" ").toLowerCase().includes(normalizedQuery),
      )
    : relationRepos;
  return sortRepos(searchedRepos);
}
