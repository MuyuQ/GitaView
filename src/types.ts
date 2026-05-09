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

export interface RepoRecord {
  id: string;
  name: string;
  path: string;
  group: string;
}

export interface GroupRecord {
  name: string;
  repoIds: string[];
}

export interface AppSettings {
  repos: RepoRecord[];
  groups: GroupRecord[];
  defaultGroup: string;
  refresh: {
    lightweightRefreshEnabled: boolean;
    intervalMinutes: number;
  };
  safety: {
    confirmPull: boolean;
    confirmPush: boolean;
  };
  appearance: {
    compactMode: boolean;
  };
}
