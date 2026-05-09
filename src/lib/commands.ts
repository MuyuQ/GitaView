import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, RepoRecord, RepoStatus } from "../types";

export function getSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("get_settings");
}

export function saveSettings(settings: AppSettings): Promise<AppSettings> {
  return invoke<AppSettings>("save_settings", { settings });
}

export function scanDirectory(path: string): Promise<string[]> {
  return invoke<string[]>("scan_directory", { path });
}

export function addRepository(path: string): Promise<RepoRecord> {
  return invoke<RepoRecord>("add_repository", { path });
}

export function removeRepository(repoId: string): Promise<void> {
  return invoke<void>("remove_repository", { repoId });
}

export function listRepoStatuses(): Promise<RepoStatus[]> {
  return invoke<RepoStatus[]>("list_repo_statuses");
}

export function fetchRepo(repoId: string): Promise<string> {
  return invoke<string>("fetch_repo", { repoId });
}

export function pullRepo(repoId: string, confirmed: boolean): Promise<string> {
  return invoke<string>("pull_repo", { repoId, confirmed });
}

export function pushRepo(repoId: string, confirmed: boolean): Promise<string> {
  return invoke<string>("push_repo", { repoId, confirmed });
}

export function openRepoDirectory(repoId: string): Promise<void> {
  return invoke<void>("open_repo_directory", { repoId });
}

export function openRepoRemote(repoId: string): Promise<void> {
  return invoke<void>("open_repo_remote", { repoId });
}
