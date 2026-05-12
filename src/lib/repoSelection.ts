export function nextSelectedRepoId(currentRepoId: string | null, clickedRepoId: string): string | null {
  return currentRepoId === clickedRepoId ? null : clickedRepoId;
}
