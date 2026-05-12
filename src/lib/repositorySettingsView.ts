import type { RepoRecord } from "../types";

export function nextRevealedRepoId(currentRepoId: string | null, clickedRepoId: string): string | null {
  return currentRepoId === clickedRepoId ? null : clickedRepoId;
}

export function getRepositoryDisplayText(repo: RepoRecord, revealedRepoId: string | null): string {
  return revealedRepoId === repo.id ? repo.path : repo.name;
}
