export type RemoteRelation =
  | "synced"
  | "local_ahead"
  | "remote_ahead"
  | "diverged"
  | "no_remote";

export type CollapsedBucket =
  | "synced"
  | "syncable"
  | "needs_attention"
  | "no_remote";

export interface RepoStatus {
  id: string;
  name: string;
  path: string;
  group: string;
  branch: string;
  relation: RemoteRelation;
  changeLabel: string;
  hint: string;
  remoteUrl: string | null;
}
