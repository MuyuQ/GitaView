import type { RepoStatus } from "../types";

export interface StatusSnapshot {
  generation: number;
  completedAt: number;
  repos: RepoStatus[];
  error: string | null;
}

// Events and command responses can arrive in either order.
export function createSnapshotReceiver(apply: (snapshot: StatusSnapshot) => void) {
  let latestGeneration = 0;
  return (snapshot: StatusSnapshot) => {
    if (snapshot.generation <= latestGeneration) return;
    latestGeneration = snapshot.generation;
    apply(snapshot);
  };
}
