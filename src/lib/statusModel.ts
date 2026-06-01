import type { CollapsedBucket, RemoteRelation, RepoStatus } from "../types";

type AppView = "collapsed" | "expanded" | "settings";

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

export function buildGroupOptions(repos: RepoStatus[]) {
  const repoGroups = Array.from(new Set(repos.map((repo) => repo.group)))
    .filter((group) => group !== "全部分组");
  return ["全部分组", ...repoGroups].map((name) => ({
    name,
    count: name === "全部分组" ? repos.length : repos.filter((repo) => repo.group === name).length,
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
  query = "",
): RepoStatus[] {
  const groupRepos = group === "全部分组" ? repos : repos.filter((repo) => repo.group === group);
  const relationRepos = relation === "all" ? groupRepos : groupRepos.filter((repo) => repo.relation === relation);
  const normalizedQuery = query.trim().toLowerCase();
  const matchingRepos = normalizedQuery
    ? relationRepos.filter((repo) =>
      [repo.name, repo.path, repo.branch, repo.group]
        .join(" ")
        .toLowerCase()
        .includes(normalizedQuery),
    )
    : relationRepos;
  return sortRepos(matchingRepos);
}

export function reconcileRelationFilter(
  repos: RepoStatus[],
  group: string,
  relation: RemoteRelation | "all",
): RemoteRelation | "all" {
  if (relation === "all") return relation;
  const groupRepos = group === "全部分组" ? repos : repos.filter((repo) => repo.group === group);
  return groupRepos.some((repo) => repo.relation === relation) ? relation : "all";
}

export function reconcileExpandedFilters(
  repos: RepoStatus[],
  group: string,
  relation: RemoteRelation | "all",
) {
  const nextGroup = group === "全部分组" || repos.some((repo) => repo.group === group)
    ? group
    : "全部分组";
  return {
    group: nextGroup,
    relation: reconcileRelationFilter(repos, nextGroup, relation),
  };
}

export function getRepoActionAvailability(repo: RepoStatus) {
  return {
    canOpenDirectory: true,
    canOpenRemote: Boolean(repo.remoteUrl),
    canFetch: repo.hasRemote && repo.relation !== "error",
    showPull: repo.relation === "remote_ahead" || repo.relation === "diverged",
    showPush: repo.relation === "local_ahead" || repo.relation === "diverged",
  };
}

export function shouldShowSettingsView(
  view: AppView,
  repoCount: number,
  initialError: string | null,
  emptySettingsDismissed: boolean,
) {
  if (view === "settings") return true;
  return !initialError && repoCount === 0 && !emptySettingsDismissed;
}
