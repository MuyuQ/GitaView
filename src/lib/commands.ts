import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, RepoRecord, RepoStatus } from "../types";
import { hasTauriRuntime } from "./runtime";

const previewSettings: AppSettings = {
  repos: [
    {
      id: "gitaview",
      name: "GitaView",
      path: "E:/Git_Repositories/GitaView",
      group: "产品",
    },
    {
      id: "website",
      name: "website",
      path: "E:/Git_Repositories/website",
      group: "业务",
    },
    {
      id: "tooling",
      name: "tooling",
      path: "E:/Git_Repositories/tooling",
      group: "基础设施",
    },
    {
      id: "archive",
      name: "archive",
      path: "E:/Git_Repositories/archive",
      group: "归档",
    },
  ],
  groups: [
    { name: "全部分组", repoIds: ["gitaview", "website", "tooling", "archive"] },
    { name: "产品", repoIds: ["gitaview"] },
    { name: "业务", repoIds: ["website"] },
    { name: "基础设施", repoIds: ["tooling"] },
    { name: "归档", repoIds: ["archive"] },
  ],
  defaultGroup: "全部分组",
  refresh: {
    lightweightRefreshEnabled: true,
    intervalMinutes: 5,
  },
  safety: {
    confirmPull: true,
    confirmPush: true,
  },
  appearance: {
    compactMode: false,
    allowWidgetDrag: true,
  },
};

const previewStatuses: RepoStatus[] = [
  {
    id: "website",
    name: "website",
    path: "E:/Git_Repositories/website",
    group: "业务",
    branch: "main",
    relation: "diverged",
    changeLabel: "↑ 2 ↓ 1",
    hint: "本地与远端分叉",
    hasRemote: true,
    remoteUrl: "https://github.com/example/website",
  },
  {
    id: "tooling",
    name: "tooling",
    path: "E:/Git_Repositories/tooling",
    group: "基础设施",
    branch: "release",
    relation: "remote_ahead",
    changeLabel: "↓ 3",
    hint: "远端有更新",
    hasRemote: true,
    remoteUrl: "https://github.com/example/tooling",
  },
  {
    id: "gitaview",
    name: "GitaView",
    path: "E:/Git_Repositories/GitaView",
    group: "产品",
    branch: "main",
    relation: "synced",
    changeLabel: "✓",
    hint: "本地与远端一致",
    hasRemote: true,
    remoteUrl: "https://github.com/example/gitaview",
  },
  {
    id: "archive",
    name: "archive",
    path: "E:/Git_Repositories/archive",
    group: "归档",
    branch: "main",
    relation: "no_remote",
    changeLabel: "无远端",
    hint: "未配置远端",
    hasRemote: false,
    remoteUrl: null,
  },
];

function previewResult<T>(value: T): Promise<T> {
  return Promise.resolve(structuredClone(value));
}

export function getSettings(): Promise<AppSettings> {
  if (!hasTauriRuntime()) return previewResult(previewSettings);
  return invoke<AppSettings>("get_settings");
}

export function saveSettings(settings: AppSettings): Promise<AppSettings> {
  if (!hasTauriRuntime()) return previewResult(settings);
  return invoke<AppSettings>("save_settings", { settings });
}

export function scanDirectory(path: string): Promise<string[]> {
  if (!hasTauriRuntime()) return previewResult(previewSettings.repos.map((repo) => repo.path));
  return invoke<string[]>("scan_directory", { path });
}

export function addRepository(path: string): Promise<RepoRecord> {
  if (!hasTauriRuntime()) {
    const name = path.split(/[\\/]/).filter(Boolean).at(-1) ?? "repo";
    return previewResult({ id: name.toLowerCase(), name, path, group: "全部分组" });
  }
  return invoke<RepoRecord>("add_repository", { path });
}

export function removeRepository(repoId: string): Promise<void> {
  if (!hasTauriRuntime()) return Promise.resolve();
  return invoke<void>("remove_repository", { repoId });
}

export function listRepoStatuses(): Promise<RepoStatus[]> {
  if (!hasTauriRuntime()) return previewResult(previewStatuses);
  return invoke<RepoStatus[]>("list_repo_statuses");
}

export function fetchRepo(repoId: string): Promise<string> {
  if (!hasTauriRuntime()) return Promise.resolve("Fetch 已完成");
  return invoke<string>("fetch_repo", { repoId });
}

export function pullRepo(repoId: string, confirmed: boolean): Promise<string> {
  if (!hasTauriRuntime()) return Promise.resolve(confirmed ? "Pull 已完成" : "Pull 需要确认");
  return invoke<string>("pull_repo", { repoId, confirmed });
}

export function pushRepo(repoId: string, confirmed: boolean): Promise<string> {
  if (!hasTauriRuntime()) return Promise.resolve(confirmed ? "Push 已完成" : "Push 需要确认");
  return invoke<string>("push_repo", { repoId, confirmed });
}

export function openRepoDirectory(repoId: string): Promise<void> {
  if (!hasTauriRuntime()) return Promise.resolve();
  return invoke<void>("open_repo_directory", { repoId });
}

export function openRepoRemote(repoId: string): Promise<void> {
  if (!hasTauriRuntime()) return Promise.resolve();
  return invoke<void>("open_repo_remote", { repoId });
}

export function exitApp(): Promise<void> {
  if (!hasTauriRuntime()) return Promise.resolve();
  return invoke<void>("exit_app");
}
