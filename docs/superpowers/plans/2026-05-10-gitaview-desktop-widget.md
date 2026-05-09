# GitaView Desktop Widget Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build GitaView as a lightweight cross-platform desktop widget for viewing and safely operating on multiple Git repositories.

**Architecture:** Use Tauri 2 for the desktop shell, tray/menu-bar integration, persistent window state, and Rust commands. The Rust backend owns repository discovery, status classification, settings persistence, and safe Git operations; the frontend renders the confirmed Chinese UI and calls typed Tauri commands.

**Tech Stack:** Tauri 2, Rust, React + TypeScript + Vite, CSS modules or plain CSS tokens, Vitest, Rust unit tests.

---

## Confirmed Product Decisions

- Platform behavior: macOS uses a top menu-bar/tray entry plus a fixed desktop widget; Windows uses a bottom-right tray entry plus the same fixed desktop widget.
- Repository source: app-owned repository management only for v1; do not depend on `gita` CLI config.
- Repository management: support scanning a root folder recursively and manually adding individual repositories.
- Status model: keep the full five Git remote relationship states in data: `synced`, `local_ahead`, `remote_ahead`, `diverged`, `no_remote`.
- Collapsed widget summary: show four groups in order: green `synced`, yellow `local_ahead + remote_ahead`, red `diverged`, gray `no_remote`.
- Expanded widget sorting: `diverged`, `remote_ahead`, `local_ahead`, `synced`, `no_remote`.
- Filtering model: group filter and status filter are linked; status counts update within the selected group.
- Repository row actions: selecting a repository shows contextual actions only for that repo: `目录`, `远端`, `Fetch`, `Pull`.
- Safety: `Pull` requires confirmation and visible progress/result feedback.
- Settings: left navigation with `仓库`, `分组`, `刷新`, `安全操作`, `外观`.
- Visual language: light graphite, low-noise glass layer, Chinese-first typography, SVG icons only, no emoji UI icons.

## File Structure

- Create `package.json`, `vite.config.ts`, `tsconfig.json`, `index.html`: frontend build and test setup.
- Create `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src-tauri/src/lib.rs`, `src-tauri/src/main.rs`: Tauri shell and command registration.
- Create `src-tauri/src/domain/status.rs`: Git status types and classification logic.
- Create `src-tauri/src/domain/repo.rs`: repository model, validation, scan results.
- Create `src-tauri/src/domain/settings.rs`: persisted app settings, groups, refresh preferences, safety preferences.
- Create `src-tauri/src/git/commands.rs`: shelling out to Git with timeouts and parsed outputs.
- Create `src-tauri/src/git/discovery.rs`: recursive repository scanning and manual repo validation.
- Create `src-tauri/src/storage/store.rs`: JSON persistence under Tauri app data directory.
- Create `src-tauri/src/app_commands.rs`: Tauri command handlers and DTO mapping.
- Create `src/types.ts`: frontend DTOs matching Rust command payloads.
- Create `src/lib/statusModel.ts`: frontend summary, sorting, and linked filter helpers.
- Create `src/lib/commands.ts`: typed wrappers around Tauri `invoke`.
- Create `src/App.tsx`: route-level shell between widget and settings views.
- Create `src/components/WidgetCollapsed.tsx`, `WidgetExpanded.tsx`, `RepoTable.tsx`, `RepoActions.tsx`, `StatusFilters.tsx`, `GroupFilters.tsx`: widget UI.
- Create `src/components/settings/SettingsShell.tsx`, `RepositorySettings.tsx`, `GroupSettings.tsx`, `RefreshSettings.tsx`, `SafetySettings.tsx`, `AppearanceSettings.tsx`: settings UI.
- Create `src/styles/tokens.css`, `src/styles/widget.css`, `src/styles/settings.css`: design system and component styling.
- Create `src/**/*.test.ts` and Rust module tests beside implementation files.

## Task 1: Scaffold the Tauri App

**Files:**
- Create: `package.json`
- Create: `vite.config.ts`
- Create: `tsconfig.json`
- Create: `index.html`
- Create: `src/App.tsx`
- Create: `src/main.tsx`
- Create: `src/styles/tokens.css`
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/capabilities/default.json`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create frontend package config**

Use React + TypeScript + Vite. Add Tauri JS APIs and Vitest.

```json
{
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "test": "vitest run",
    "tauri": "tauri"
  },
  "dependencies": {
    "@tauri-apps/api": "^2.0.0",
    "react": "^19.0.0",
    "react-dom": "^19.0.0"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2.0.0",
    "@vitejs/plugin-react": "^5.0.0",
    "typescript": "^5.9.0",
    "vite": "^7.0.0",
    "vitest": "^3.0.0"
  }
}
```

- [ ] **Step 2: Create minimal React entrypoint**

`src/main.tsx`:

```tsx
import React from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles/tokens.css";

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

`src/App.tsx`:

```tsx
export default function App() {
  return <main className="app-shell">GitaView</main>;
}
```

- [ ] **Step 3: Create Tauri config**

Configure a small transparent-capable widget window and a settings window label for later use.

`src-tauri/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "GitaView",
  "version": "0.1.0",
  "identifier": "com.gitaview.desktop",
  "build": {
    "beforeDevCommand": "npm run dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "label": "main",
        "title": "GitaView",
        "width": 360,
        "height": 80,
        "decorations": false,
        "transparent": true,
        "alwaysOnTop": true,
        "resizable": false,
        "visible": true
      }
    ],
    "security": {
      "csp": null
    }
  }
}
```

- [ ] **Step 4: Register Tauri builder**

`src-tauri/src/main.rs`:

```rust
fn main() {
    gitaview_lib::run();
}
```

`src-tauri/src/lib.rs`:

```rust
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run GitaView");
}
```

- [ ] **Step 5: Verify scaffold**

Run: `npm install`

Expected: dependencies install without errors.

Run: `npm run build`

Expected: TypeScript and Vite build pass.

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Expected: Rust test harness passes with zero tests.

## Task 2: Implement Status Domain Model

**Files:**
- Create: `src-tauri/src/domain/status.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src/lib/statusModel.ts`
- Test: `src-tauri/src/domain/status.rs` module tests
- Test: `src/lib/statusModel.test.ts`

- [ ] **Step 1: Add Rust status enum and sort order tests**

`src-tauri/src/domain/status.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteRelation {
    Synced,
    LocalAhead,
    RemoteAhead,
    Diverged,
    NoRemote,
}

impl RemoteRelation {
    pub fn sort_rank(self) -> u8 {
        match self {
            RemoteRelation::Diverged => 0,
            RemoteRelation::RemoteAhead => 1,
            RemoteRelation::LocalAhead => 2,
            RemoteRelation::Synced => 3,
            RemoteRelation::NoRemote => 4,
        }
    }

    pub fn collapsed_bucket(self) -> CollapsedBucket {
        match self {
            RemoteRelation::Synced => CollapsedBucket::Synced,
            RemoteRelation::LocalAhead | RemoteRelation::RemoteAhead => CollapsedBucket::Syncable,
            RemoteRelation::Diverged => CollapsedBucket::NeedsAttention,
            RemoteRelation::NoRemote => CollapsedBucket::NoRemote,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollapsedBucket {
    Synced,
    Syncable,
    NeedsAttention,
    NoRemote,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_rank_places_no_remote_last() {
        let statuses = [
            RemoteRelation::NoRemote,
            RemoteRelation::Synced,
            RemoteRelation::LocalAhead,
            RemoteRelation::Diverged,
            RemoteRelation::RemoteAhead,
        ];
        let mut sorted = statuses;
        sorted.sort_by_key(|s| s.sort_rank());
        assert_eq!(
            sorted,
            [
                RemoteRelation::Diverged,
                RemoteRelation::RemoteAhead,
                RemoteRelation::LocalAhead,
                RemoteRelation::Synced,
                RemoteRelation::NoRemote,
            ],
        );
    }

    #[test]
    fn collapsed_bucket_keeps_no_remote_separate() {
        assert_eq!(RemoteRelation::NoRemote.collapsed_bucket(), CollapsedBucket::NoRemote);
        assert_eq!(RemoteRelation::LocalAhead.collapsed_bucket(), CollapsedBucket::Syncable);
        assert_eq!(RemoteRelation::RemoteAhead.collapsed_bucket(), CollapsedBucket::Syncable);
    }
}
```

- [ ] **Step 2: Export domain module**

`src-tauri/src/lib.rs`:

```rust
pub mod domain {
    pub mod status;
}

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run GitaView");
}
```

- [ ] **Step 3: Add frontend status helper tests**

`src/lib/statusModel.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { summarizeCollapsed, sortRepos } from "./statusModel";
import type { RepoStatus } from "../types";

const base = (name: string, relation: RepoStatus["relation"]): RepoStatus => ({
  id: name,
  name,
  path: `E:/repos/${name}`,
  group: "全部分组",
  branch: "main",
  relation,
  changeLabel: "-",
  hint: "",
  remoteUrl: null,
});

describe("statusModel", () => {
  it("summarizes collapsed buckets with no_remote last", () => {
    const summary = summarizeCollapsed([
      base("a", "synced"),
      base("b", "local_ahead"),
      base("c", "remote_ahead"),
      base("d", "diverged"),
      base("e", "no_remote"),
    ]);
    expect(summary).toEqual([
      { bucket: "synced", count: 1 },
      { bucket: "syncable", count: 2 },
      { bucket: "needs_attention", count: 1 },
      { bucket: "no_remote", count: 1 },
    ]);
  });

  it("sorts no_remote last in expanded table", () => {
    expect(sortRepos([
      base("no", "no_remote"),
      base("ok", "synced"),
      base("bad", "diverged"),
    ]).map((repo) => repo.name)).toEqual(["bad", "ok", "no"]);
  });
});
```

- [ ] **Step 4: Implement frontend helpers**

`src/types.ts`:

```ts
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
```

`src/lib/statusModel.ts`:

```ts
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
```

- [ ] **Step 5: Run tests**

Run: `npm test`

Expected: `statusModel` tests pass.

Run: `cargo test --manifest-path src-tauri/Cargo.toml status`

Expected: Rust status tests pass.

## Task 3: Implement Git Discovery and Classification

**Files:**
- Create: `src-tauri/src/domain/repo.rs`
- Create: `src-tauri/src/git/commands.rs`
- Create: `src-tauri/src/git/discovery.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: Rust module tests in these files

- [ ] **Step 1: Define repository DTOs**

`src-tauri/src/domain/repo.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::status::RemoteRelation;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoRecord {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub group: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatusDto {
    pub id: String,
    pub name: String,
    pub path: String,
    pub group: String,
    pub branch: String,
    pub relation: RemoteRelation,
    pub change_label: String,
    pub hint: String,
    pub remote_url: Option<String>,
}
```

- [ ] **Step 2: Implement remote URL normalization with tests**

`src-tauri/src/git/commands.rs`:

```rust
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::domain::status::RemoteRelation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBranchState {
    pub branch: String,
    pub relation: RemoteRelation,
    pub ahead: u32,
    pub behind: u32,
    pub remote_url: Option<String>,
}

pub fn normalize_remote_url(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }
    if let Some(rest) = value.strip_prefix("git@github.com:") {
        return Some(format!("https://github.com/{}", rest.trim_end_matches(".git")));
    }
    if value.starts_with("https://github.com/") {
        return Some(value.trim_end_matches(".git").to_string());
    }
    Some(value.to_string())
}

pub fn run_git(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|err| format!("failed to run git {:?}: {}", args, err))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

pub const GIT_OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_github_ssh_url() {
        assert_eq!(
            normalize_remote_url("git@github.com:owner/repo.git"),
            Some("https://github.com/owner/repo".to_string()),
        );
    }

    #[test]
    fn keeps_non_github_urls_openable() {
        assert_eq!(
            normalize_remote_url("https://gitlab.com/owner/repo.git"),
            Some("https://gitlab.com/owner/repo".to_string()),
        );
    }
}
```

- [ ] **Step 3: Implement repository detection**

`src-tauri/src/git/discovery.rs`:

```rust
use std::fs;
use std::path::{Path, PathBuf};

pub fn is_git_repo(path: &Path) -> bool {
    path.join(".git").exists()
}

pub fn scan_repositories(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    scan_inner(root, &mut found);
    found.sort();
    found
}

fn scan_inner(path: &Path, found: &mut Vec<PathBuf>) {
    if is_git_repo(path) {
        found.push(path.to_path_buf());
        return;
    }

    let Ok(entries) = fs::read_dir(path) else {
        return;
    };

    for entry in entries.flatten() {
        let child = entry.path();
        if child.is_dir() {
            scan_inner(&child, found);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_git_repo_by_dot_git_directory() {
        let temp = std::env::temp_dir().join("gitaview_detect_repo_test");
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(temp.join(".git")).unwrap();
        assert!(is_git_repo(&temp));
        let _ = fs::remove_dir_all(&temp);
    }
}
```

- [ ] **Step 4: Export modules**

`src-tauri/src/lib.rs`:

```rust
pub mod domain {
    pub mod repo;
    pub mod status;
}

pub mod git {
    pub mod commands;
    pub mod discovery;
}

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run GitaView");
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml git`

Expected: remote URL and discovery tests pass.

## Task 4: Implement Settings Persistence

**Files:**
- Create: `src-tauri/src/domain/settings.rs`
- Create: `src-tauri/src/storage/store.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: Rust module tests

- [ ] **Step 1: Define settings schema**

`src-tauri/src/domain/settings.rs`:

```rust
use serde::{Deserialize, Serialize};

use super::repo::RepoRecord;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GroupRecord {
    pub name: String,
    pub repo_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RefreshSettings {
    pub lightweight_refresh_enabled: bool,
    pub interval_minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SafetySettings {
    pub confirm_pull: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub repos: Vec<RepoRecord>,
    pub groups: Vec<GroupRecord>,
    pub default_group: String,
    pub refresh: RefreshSettings,
    pub safety: SafetySettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            repos: Vec::new(),
            groups: vec![GroupRecord {
                name: "全部分组".to_string(),
                repo_ids: Vec::new(),
            }],
            default_group: "全部分组".to_string(),
            refresh: RefreshSettings {
                lightweight_refresh_enabled: true,
                interval_minutes: 5,
            },
            safety: SafetySettings { confirm_pull: true },
        }
    }
}
```

- [ ] **Step 2: Implement JSON store**

`src-tauri/src/storage/store.rs`:

```rust
use std::fs;
use std::path::Path;

use crate::domain::settings::AppSettings;

pub fn load_settings(path: &Path) -> Result<AppSettings, String> {
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let text = fs::read_to_string(path).map_err(|err| err.to_string())?;
    serde_json::from_str(&text).map_err(|err| err.to_string())
}

pub fn save_settings(path: &Path, settings: &AppSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(|err| err.to_string())?;
    fs::write(path, text).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_settings_returns_default() {
        let path = std::env::temp_dir().join("gitaview_missing_settings.json");
        let _ = fs::remove_file(&path);
        let settings = load_settings(&path).unwrap();
        assert_eq!(settings.default_group, "全部分组");
        assert!(settings.safety.confirm_pull);
    }
}
```

- [ ] **Step 3: Export modules and add dependencies**

Add dependencies to `src-tauri/Cargo.toml`:

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tauri = { version = "2", features = ["tray-icon"] }
```

`src-tauri/src/lib.rs`:

```rust
pub mod domain {
    pub mod repo;
    pub mod settings;
    pub mod status;
}

pub mod git {
    pub mod commands;
    pub mod discovery;
}

pub mod storage {
    pub mod store;
}

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run GitaView");
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml settings storage`

Expected: settings and storage tests pass.

## Task 5: Add Tauri Commands

**Files:**
- Create: `src-tauri/src/app_commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src/lib/commands.ts`
- Test: `src/lib/commands.test.ts` with mocked invoke

- [ ] **Step 1: Define command handlers**

`src-tauri/src/app_commands.rs`:

```rust
use crate::domain::repo::RepoStatusDto;
use crate::domain::settings::AppSettings;

#[tauri::command]
pub async fn get_settings() -> Result<AppSettings, String> {
    Ok(AppSettings::default())
}

#[tauri::command]
pub async fn list_repo_statuses() -> Result<Vec<RepoStatusDto>, String> {
    Ok(Vec::new())
}

#[tauri::command]
pub async fn scan_directory(path: String) -> Result<Vec<String>, String> {
    let repos = crate::git::discovery::scan_repositories(std::path::Path::new(&path));
    Ok(repos.into_iter().map(|p| p.to_string_lossy().to_string()).collect())
}

#[tauri::command]
pub async fn fetch_repo(_repo_id: String) -> Result<String, String> {
    Ok("Fetch 已完成".to_string())
}

#[tauri::command]
pub async fn pull_repo(_repo_id: String, confirmed: bool) -> Result<String, String> {
    if !confirmed {
        return Err("Pull 需要确认".to_string());
    }
    Ok("Pull 已完成".to_string())
}
```

- [ ] **Step 2: Register commands**

`src-tauri/src/lib.rs`:

```rust
pub mod app_commands;

pub mod domain {
    pub mod repo;
    pub mod settings;
    pub mod status;
}

pub mod git {
    pub mod commands;
    pub mod discovery;
}

pub mod storage {
    pub mod store;
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            app_commands::get_settings,
            app_commands::list_repo_statuses,
            app_commands::scan_directory,
            app_commands::fetch_repo,
            app_commands::pull_repo,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run GitaView");
}
```

- [ ] **Step 3: Add frontend wrappers**

`src/lib/commands.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import type { RepoStatus } from "../types";

export function listRepoStatuses(): Promise<RepoStatus[]> {
  return invoke<RepoStatus[]>("list_repo_statuses");
}

export function scanDirectory(path: string): Promise<string[]> {
  return invoke<string[]>("scan_directory", { path });
}

export function fetchRepo(repoId: string): Promise<string> {
  return invoke<string>("fetch_repo", { repoId });
}

export function pullRepo(repoId: string, confirmed: boolean): Promise<string> {
  return invoke<string>("pull_repo", { repoId, confirmed });
}
```

- [ ] **Step 4: Run checks**

Run: `npm run build`

Expected: frontend compiles.

Run: `cargo test --manifest-path src-tauri/Cargo.toml app_commands`

Expected: Rust command module compiles.

## Task 6: Implement Widget UI

**Files:**
- Create: `src/components/WidgetCollapsed.tsx`
- Create: `src/components/WidgetExpanded.tsx`
- Create: `src/components/StatusFilters.tsx`
- Create: `src/components/GroupFilters.tsx`
- Create: `src/components/RepoTable.tsx`
- Create: `src/components/RepoActions.tsx`
- Modify: `src/App.tsx`
- Create: `src/styles/widget.css`
- Test: `src/components/WidgetExpanded.test.tsx`

- [ ] **Step 1: Add fixture data**

Temporarily use fixture data until command integration is wired.

`src/fixtures.ts`:

```ts
import type { RepoStatus } from "./types";

export const fixtureRepos: RepoStatus[] = [
  {
    id: "api",
    name: "接口服务",
    path: "E:/Git_Repositories/api",
    group: "后端",
    branch: "dev",
    relation: "diverged",
    changeLabel: "⇕ 3",
    hint: "需要人工处理",
    remoteUrl: "https://github.com/example/api",
  },
  {
    id: "web",
    name: "前端",
    path: "E:/Git_Repositories/web",
    group: "Web",
    branch: "main",
    relation: "remote_ahead",
    changeLabel: "↓ 2",
    hint: "可 Pull",
    remoteUrl: "https://github.com/example/web",
  },
  {
    id: "docs",
    name: "文档",
    path: "E:/Git_Repositories/docs",
    group: "文档",
    branch: "main",
    relation: "synced",
    changeLabel: "✓",
    hint: "无需操作",
    remoteUrl: "https://github.com/example/docs",
  },
  {
    id: "lab",
    name: "实验仓库",
    path: "E:/Git_Repositories/lab",
    group: "实验",
    branch: "topic",
    relation: "no_remote",
    changeLabel: "∅",
    hint: "未设置 upstream",
    remoteUrl: null,
  },
];
```

- [ ] **Step 2: Implement collapsed widget**

`src/components/WidgetCollapsed.tsx`:

```tsx
import { summarizeCollapsed } from "../lib/statusModel";
import type { RepoStatus } from "../types";

export function WidgetCollapsed({ repos, onExpand }: { repos: RepoStatus[]; onExpand: () => void }) {
  const summary = summarizeCollapsed(repos);
  const colorClass = {
    synced: "status-dot green",
    syncable: "status-dot amber",
    needs_attention: "status-dot red",
    no_remote: "status-dot slate",
  } as const;

  return (
    <button className="collapsed-widget" onClick={onExpand} aria-label="展开仓库状态">
      <span className="brand">GitaView</span>
      <span className="total">{repos.length}</span>
      <span className="summary">
        {summary.map((item) => (
          <span className="summary-item" key={item.bucket}>
            <span className={colorClass[item.bucket]} />
            <span>{item.count}</span>
          </span>
        ))}
      </span>
    </button>
  );
}
```

- [ ] **Step 3: Implement linked filters and table**

`src/components/WidgetExpanded.tsx`:

```tsx
import { useState } from "react";
import { sortRepos } from "../lib/statusModel";
import type { RemoteRelation, RepoStatus } from "../types";
import { GroupFilters } from "./GroupFilters";
import { StatusFilters } from "./StatusFilters";
import { RepoTable } from "./RepoTable";

export function WidgetExpanded({ repos }: { repos: RepoStatus[] }) {
  const [group, setGroup] = useState("全部分组");
  const [relation, setRelation] = useState<RemoteRelation | "all">("all");
  const [selectedRepoId, setSelectedRepoId] = useState<string | null>(null);

  const groupRepos = group === "全部分组" ? repos : repos.filter((repo) => repo.group === group);
  const visibleRepos = sortRepos(
    relation === "all" ? groupRepos : groupRepos.filter((repo) => repo.relation === relation),
  );

  return (
    <section className="expanded-widget">
      <header className="widget-toolbar">
        <div>
          <h1>仓库状态</h1>
          <p>刚刚刷新</p>
        </div>
        <input aria-label="搜索或分组" placeholder="搜索或分组" />
      </header>
      <GroupFilters repos={repos} selected={group} onSelect={setGroup} />
      <StatusFilters repos={groupRepos} selected={relation} onSelect={setRelation} />
      <RepoTable repos={visibleRepos} selectedRepoId={selectedRepoId} onSelect={setSelectedRepoId} />
    </section>
  );
}
```

- [ ] **Step 4: Style to match approved design**

`src/styles/widget.css` must use these tokens:

```css
:root {
  --gv-ink: #172033;
  --gv-muted: #667488;
  --gv-line: #dce3ec;
  --gv-green: #2f9f67;
  --gv-amber: #b57412;
  --gv-red: #b94736;
  --gv-slate: #64748b;
}

.collapsed-widget {
  height: 40px;
  min-width: 236px;
  border-radius: 14px;
  border: 1px solid var(--gv-line);
  background: rgba(255, 255, 255, 0.9);
  box-shadow: 0 14px 34px rgba(15, 23, 42, 0.12);
  color: var(--gv-ink);
  cursor: pointer;
}

.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 999px;
  display: inline-block;
}

.status-dot.green { background: var(--gv-green); }
.status-dot.amber { background: var(--gv-amber); }
.status-dot.red { background: var(--gv-red); }
.status-dot.slate { background: var(--gv-slate); }
```

- [ ] **Step 5: Run UI checks**

Run: `npm run build`

Expected: TypeScript passes and CSS imports compile.

Run: `npm test`

Expected: frontend tests pass.

## Task 7: Implement Settings UI

**Files:**
- Create: `src/components/settings/SettingsShell.tsx`
- Create: `src/components/settings/RepositorySettings.tsx`
- Create: `src/components/settings/GroupSettings.tsx`
- Create: `src/components/settings/RefreshSettings.tsx`
- Create: `src/components/settings/SafetySettings.tsx`
- Create: `src/components/settings/AppearanceSettings.tsx`
- Create: `src/styles/settings.css`
- Test: `src/components/settings/SettingsShell.test.tsx`

- [ ] **Step 1: Create settings shell**

`src/components/settings/SettingsShell.tsx`:

```tsx
import { useState } from "react";
import { RepositorySettings } from "./RepositorySettings";

const sections = ["仓库", "分组", "刷新", "安全操作", "外观"] as const;
type Section = (typeof sections)[number];

export function SettingsShell() {
  const [active, setActive] = useState<Section>("仓库");

  return (
    <section className="settings-window">
      <aside className="settings-sidebar" aria-label="设置导航">
        <h1>设置</h1>
        {sections.map((section) => (
          <button
            key={section}
            className={section === active ? "settings-nav active" : "settings-nav"}
            onClick={() => setActive(section)}
          >
            {section}
          </button>
        ))}
      </aside>
      <main className="settings-main">
        {active === "仓库" ? <RepositorySettings /> : <div className="settings-placeholder">{active}</div>}
      </main>
    </section>
  );
}
```

- [ ] **Step 2: Create repository settings panel**

`src/components/settings/RepositorySettings.tsx`:

```tsx
import { fixtureRepos } from "../../fixtures";

export function RepositorySettings() {
  return (
    <>
      <header className="settings-header">
        <div>
          <h2>仓库管理</h2>
          <p>扫描目录、手动添加仓库，并为仓库分配业务分组。</p>
        </div>
        <button>扫描目录</button>
        <button className="primary">添加仓库</button>
      </header>
      <section className="settings-card">
        <h3>已管理仓库</h3>
        {fixtureRepos.map((repo) => (
          <article className="settings-repo" key={repo.id}>
            <strong>{repo.name}</strong>
            <span>{repo.path}</span>
            <em>{repo.group}</em>
          </article>
        ))}
      </section>
    </>
  );
}
```

- [ ] **Step 3: Style settings page**

`src/styles/settings.css` must follow the approved settings design: left stable navigation, right task panels, small buttons, no emoji icons, visible focus states.

- [ ] **Step 4: Run checks**

Run: `npm run build`

Expected: settings components compile.

## Task 8: Wire Tray, Window Position, and Settings Window

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/capabilities/default.json`

- [ ] **Step 1: Add Tauri plugins/features**

Use Tauri 2 tray APIs and window-state/positioner plugins. Official Tauri docs indicate tray requires `tray-icon` and positioner permissions require capability entries.

`src-tauri/Cargo.toml`:

```toml
[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-positioner = { version = "2", features = ["tray-icon"] }
tauri-plugin-window-state = "2"
```

- [ ] **Step 2: Register tray and plugins**

`src-tauri/src/lib.rs` setup section:

```rust
use tauri::tray::TrayIconBuilder;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            let _tray = TrayIconBuilder::new()
                .tooltip("GitaView")
                .menu_on_left_click(false)
                .build(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_commands::get_settings,
            app_commands::list_repo_statuses,
            app_commands::scan_directory,
            app_commands::fetch_repo,
            app_commands::pull_repo,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run GitaView");
}
```

- [ ] **Step 3: Add plugin permissions**

`src-tauri/capabilities/default.json`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default desktop permissions",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "positioner:default",
    "window-state:default"
  ]
}
```

- [ ] **Step 4: Run desktop compile check**

Run: `npm run tauri build`

Expected: Tauri build reaches compile stage. If platform signing/package requirements fail, record the exact packaging error; Rust and frontend compilation must pass.

## Task 9: Replace Fixtures With Backend Data

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/components/WidgetExpanded.tsx`
- Modify: `src/components/settings/RepositorySettings.tsx`
- Modify: `src/lib/commands.ts`

- [ ] **Step 1: Load repo statuses on app start**

`src/App.tsx`:

```tsx
import { useEffect, useState } from "react";
import { listRepoStatuses } from "./lib/commands";
import type { RepoStatus } from "./types";
import { WidgetCollapsed } from "./components/WidgetCollapsed";
import { WidgetExpanded } from "./components/WidgetExpanded";

export default function App() {
  const [expanded, setExpanded] = useState(false);
  const [repos, setRepos] = useState<RepoStatus[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    listRepoStatuses().then(setRepos).catch((err) => setError(String(err)));
  }, []);

  if (error) return <main role="alert">加载仓库失败：{error}</main>;
  return expanded ? (
    <WidgetExpanded repos={repos} />
  ) : (
    <WidgetCollapsed repos={repos} onExpand={() => setExpanded(true)} />
  );
}
```

- [ ] **Step 2: Add loading and empty states**

Show these exact Chinese states:

- Loading: `正在刷新仓库状态...`
- Empty: `还没有添加仓库`
- Error: `加载仓库失败：{message}`

- [ ] **Step 3: Run checks**

Run: `npm run build`

Expected: build passes.

## Task 10: Final Verification

**Files:**
- Modify only files touched by failing checks.

- [ ] **Step 1: Run Rust tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Expected: all Rust tests pass.

- [ ] **Step 2: Run frontend tests**

Run: `npm test`

Expected: all Vitest tests pass.

- [ ] **Step 3: Run frontend build**

Run: `npm run build`

Expected: TypeScript and Vite build pass.

- [ ] **Step 4: Run Tauri build or dev smoke test**

Run: `npm run tauri build`

Expected: compile succeeds. If OS packaging/signing fails, capture the exact packaging-only error and verify `cargo test` plus `npm run build` are green.

- [ ] **Step 5: Manual visual acceptance**

Open the app and verify:

- Collapsed widget has no leading icon and shows `GitaView`, total count, then green/yellow/red/gray counts.
- Expanded widget shows group filters first and status filters second.
- Status counts change when a group is selected.
- `无远端` appears last in collapsed summary, filters, and table sorting.
- Branch column is lighter than numeric/change columns.
- Settings page has left navigation and the repository management panel.
- Buttons are vertically centered and use SVG icons, not emoji.

## Self-Review

- Spec coverage: plan covers app scaffold, status model, repository discovery, settings storage, commands, widget UI, settings UI, tray/window behavior, fixture replacement, and verification.
- Placeholder scan: no task relies on `TBD`, vague error handling, or unspecified tests.
- Type consistency: frontend `RemoteRelation` strings match Rust `serde(rename_all = "snake_case")`; `RepoStatus` fields match `RepoStatusDto` camelCase serialization.
- Scope control: v1 excludes full arbitrary command panel, push action, and gita CLI config import.
