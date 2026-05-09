import type { CollapsedBucket, RemoteRelation, RepoStatus } from "../types";

const expandedRank: Record<RemoteRelation, number> = {
  diverged: 0,
  remote_ahead: 1,
  local_ahead: 2,
  synced: 3,
  no_remote: 4,
};

const bucketRank: CollapsedBucket[] = [
  "synced",
  "syncable",
  "needs_attention",
  "no_remote",
];

export function toCollapsedBucket(relation: RemoteRelation): CollapsedBucket {
  if (relation === "synced") return "synced";
  if (relation === "diverged") return "needs_attention";
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
