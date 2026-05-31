# GitaView Design and Build Specification

This document is a self-contained handoff for implementing GitaView from an empty repository.

## 1. Product Summary

GitaView is a lightweight cross-platform desktop widget for managing many local Git repositories.

It is inspired by `gita`, but v1 is not a CLI wrapper and does not depend on `gita` configuration files. GitaView owns its repository list, groups, settings, and UI.

Primary user goal:

- See the health of many repositories at a glance.
- Expand the widget to inspect exact Git state.
- Quickly open a repo folder or remote URL.
- Safely run `fetch`, `pull`, and guarded `push` on one selected repository.

Target platforms:

- macOS: menu-bar/tray entry plus a fixed desktop widget.
- Windows: system tray entry plus the same fixed desktop widget.

## 2. Tech Stack

Use:

- Tauri 2 for the desktop shell.
- Rust for backend Git operations, scanning, status classification, persistence, and Tauri commands.
- React + TypeScript + Vite for frontend UI.
- Plain CSS or CSS modules for styling.
- Vitest for frontend unit tests.
- Rust unit tests for backend logic.

Avoid:

- Electron.
- Python runtime embedding.
- Arbitrary shell command execution in v1.
- Depending on `gita` CLI config.

## 3. Core UX

### 3.1 Collapsed Widget

Collapsed widget is a small horizontal status strip.

It must not have a leading icon. It should show:

```text
GitaView  24   green 17   yellow 4   red 2   gray 1
```

Meaning:

- Total count: all managed repositories.
- Green: synced repositories.
- Yellow: repositories that are local-ahead or remote-ahead.
- Red: diverged repositories.
- Gray: repositories with no upstream/remote.

Collapsed order:

1. Green synced
2. Yellow syncable
3. Red diverged
4. Gray no remote

Clicking the collapsed widget expands it.

### 3.2 Expanded Widget

Expanded widget is a compact floating panel.

Header:

- Title: `仓库状态`
- Subtitle: refresh time only, e.g. `刚刚刷新`
- Search input placeholder: `搜索或分组`

Filtering model has two linked dimensions:

1. Group filter
2. Status filter

Group filter appears first:

```text
全部分组 24 | 后端 4 | Web 6 | 共享 3 | 文档 5 | 实验 1
```

Status filter appears second and its counts must update based on the selected group:

```text
全部 24 | 同步 17 | 本地领先 2 | 远程领先 2 | 分叉 2 | 无远端 1
```

If group `Web 6` is selected, status counts should reflect only Web repos, e.g.:

```text
全部 6 | 同步 3 | 远程领先 2 | 分叉 1 | 无远端 0
```

List columns:

```text
status dot | 仓库 | 分类 | 分支 | 关系 | 变更 | 提示
```

Expanded list sort order:

1. Diverged
2. Remote ahead
3. Local ahead
4. Synced
5. No remote

No remote must always be last, including in collapsed summary, filters, and list sort.

### 3.3 Repository Row Actions

Rows are not selected with checkboxes.

Clicking a repository row selects it and shows a contextual action row below that repository only.

Actions:

- `目录`: open local folder.
- `远端`: open remote URL in browser.
- `Fetch`: run `git fetch`.
- `Pull`: run `git pull`.
- `Push`: run `git push` only when the repository is local-ahead or diverged.

Rules:

- Actions apply only to the selected repository.
- `Pull` requires confirmation because it modifies the working tree.
- `远端` is disabled if no remote URL exists.
- Show loading and result feedback for actions longer than 300ms.
- Show `Push` only for local-ahead or diverged repositories and require confirmation before invoking it.
- Do not implement arbitrary command panels in v1.

## 4. Settings UX

Settings page uses the same visual language as the widget.

Structure:

- Left navigation:
  - `仓库`
  - `分组`
  - `刷新`
  - `安全操作`
  - `外观`

Default section: `仓库`.

Repository settings must include:

- `扫描目录`
- `添加仓库`
- Managed repository list
- Repository path display
- Group tag display

Common preferences shown in settings:

- Lightweight timed refresh: on/off.
- Refresh interval: default `5 分钟`.
- Pull and Push confirmation: mandatory and not configurable.
- Default group: default `全部分组`.

## 5. Visual Design System

Style direction:

- Light graphite.
- Low-noise glass layer.
- Compact desktop utility.
- Chinese-first readability.
- Swiss/grid-like alignment.

Use these color tokens:

```css
:root {
  --gv-ink: #172033;
  --gv-muted: #667488;
  --gv-line: #dce3ec;
  --gv-green: #2f9f67;
  --gv-amber: #b57412;
  --gv-red: #b94736;
  --gv-slate: #64748b;
  --gv-bg: #f7f9fc;
}
```

Typography:

- Chinese UI: `Noto Sans SC`, `HarmonyOS Sans SC`, `MiSans`, `Microsoft YaHei`, sans-serif.
- Numeric/change columns: `JetBrains Mono`, `SF Mono`, `Cascadia Mono`, monospace.
- Branch column should use monospace but must be lighter than numeric/change columns.

UI rules:

- No emoji icons.
- Use consistent SVG icons.
- Buttons must be compact and vertically centered.
- Action buttons should be visually quiet.
- `Pull` uses warmer styling to indicate risk.
- Status must not be conveyed by color alone; include text pills and numbers.
- Light mode contrast must be readable.
- Hover/focus states must not shift layout.

## 6. Git Status Model

Backend must preserve five remote relation states:

```ts
type RemoteRelation =
  | "synced"
  | "local_ahead"
  | "remote_ahead"
  | "diverged"
  | "no_remote";
```

Meaning:

- `synced`: local branch equals upstream.
- `local_ahead`: local has commits not on upstream.
- `remote_ahead`: upstream has commits not local.
- `diverged`: both local and upstream have unique commits.
- `no_remote`: branch has no supported `origin` comparison.

Collapsed buckets:

```ts
type CollapsedBucket =
  | "synced"
  | "syncable"
  | "needs_attention"
  | "no_remote";
```

Mapping:

- `synced` -> `synced`
- `local_ahead` -> `syncable`
- `remote_ahead` -> `syncable`
- `diverged` -> `needs_attention`
- `no_remote` -> `no_remote`

Expanded sort rank:

```ts
const expandedRank = {
  diverged: 0,
  remote_ahead: 1,
  local_ahead: 2,
  synced: 3,
  no_remote: 4,
};
```

## 7. Data Model

Use app-owned persistence, preferably JSON in Tauri app data directory.

Repository record:

```ts
interface RepoRecord {
  id: string;
  name: string;
  path: string;
  group: string;
}
```

Repository status DTO:

```ts
interface RepoStatus {
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

Settings:

```ts
interface AppSettings {
  repos: RepoRecord[];
  groups: GroupRecord[];
  defaultGroup: string;
  refresh: {
    lightweightRefreshEnabled: boolean;
    intervalMinutes: number;
  };
  safety: {
    confirmPull: boolean;
  };
  appearance: {
    compactMode: boolean;
  };
}

interface GroupRecord {
  name: string;
  repoIds: string[];
}
```

Defaults:

- `defaultGroup`: `全部分组`
- `lightweightRefreshEnabled`: `true`
- `intervalMinutes`: `5`
- `confirmPull`: `true`

## 8. Backend Responsibilities

Rust backend must implement:

- Recursive repository scanning.
- Manual repository validation.
- Settings load/save.
- Git status classification.
- Remote URL normalization.
- Tauri commands for frontend.
- Tray/menu-bar setup.
- Window persistence.

### 8.1 Repository Scanning

When scanning a root directory:

- Recursively search subdirectories.
- A repo is valid if `.git` exists as a directory or file.
- Once a repo is found, do not scan its children further.
- Return unique absolute paths.

### 8.2 Remote URL Normalization

Examples:

```text
git@github.com:owner/repo.git -> https://github.com/owner/repo
https://github.com/owner/repo.git -> https://github.com/owner/repo
https://gitlab.com/owner/repo.git -> https://gitlab.com/owner/repo
```

If no remote exists, return `null`.

### 8.3 Git Commands

Use `git` executable via Rust process calls.

Needed operations:

- Current branch.
- Upstream relation.
- Ahead/behind counts.
- Remote origin URL.
- `git fetch`
- `git pull`
- `git push`

v1 reads and opens only the `origin` remote URL. Other remote topologies are out of scope.

Operations must return structured success/error messages.

## 9. Tauri Commands

Expose these commands:

```ts
get_settings(): Promise<AppSettings>
save_settings(settings: AppSettings): Promise<AppSettings>
scan_directory(path: string): Promise<string[]>
add_repository(path: string): Promise<RepoRecord>
remove_repository(repoId: string): Promise<void>
list_repo_statuses(): Promise<RepoStatus[]>
fetch_repo(repoId: string): Promise<string>
pull_repo(repoId: string, confirmed: boolean): Promise<string>
push_repo(repoId: string, confirmed: boolean): Promise<string>
open_repo_directory(repoId: string): Promise<void>
open_repo_remote(repoId: string): Promise<void>
```

`pull_repo` must reject when `confirmed` is false.
`push_repo` must reject when `confirmed` is false.

## 10. Frontend Responsibilities

Frontend must implement:

- Collapsed widget.
- Expanded widget.
- Group filters.
- Status filters.
- Linked counts.
- Repository table.
- Row selection and contextual actions.
- Settings page.
- Loading, empty, and error states.

Required Chinese states:

- Loading: `正在刷新仓库状态...`
- Empty: `还没有添加仓库`
- Error: `加载仓库失败：{message}`
- Pull confirmation title: `确认 Pull`
- Pull confirmation message: `Pull 会修改当前仓库工作区，是否继续？`

## 11. Suggested File Structure

```text
package.json
vite.config.ts
tsconfig.json
index.html
src/
  main.tsx
  App.tsx
  types.ts
  lib/
    commands.ts
    statusModel.ts
  components/
    WidgetCollapsed.tsx
    WidgetExpanded.tsx
    GroupFilters.tsx
    StatusFilters.tsx
    RepoTable.tsx
    RepoActions.tsx
    settings/
      SettingsShell.tsx
      RepositorySettings.tsx
      GroupSettings.tsx
      RefreshSettings.tsx
      SafetySettings.tsx
      AppearanceSettings.tsx
  styles/
    tokens.css
    widget.css
    settings.css
src-tauri/
  Cargo.toml
  tauri.conf.json
  capabilities/default.json
  src/
    main.rs
    lib.rs
    app_commands.rs
    domain/
      repo.rs
      settings.rs
      status.rs
    git/
      commands.rs
      discovery.rs
    storage/
      store.rs
```

## 12. Implementation Order

1. Scaffold Tauri + React + TypeScript + Vite.
2. Implement status model and frontend status helpers with tests.
3. Implement settings schema and JSON persistence.
4. Implement repository discovery.
5. Implement Git command helpers and remote URL normalization.
6. Implement Tauri commands.
7. Build widget UI using fixture data.
8. Build settings UI using fixture data.
9. Replace fixture data with Tauri command calls.
10. Add tray/menu-bar integration and persistent window state.
11. Add final action feedback, confirmation dialogs, and error states.
12. Run full verification.

## 13. Tests

Required backend tests:

- Status sort order puts `no_remote` last.
- Collapsed bucket mapping keeps `no_remote` separate.
- Remote URL normalization for GitHub SSH and HTTPS.
- Repository scanning finds `.git` directories.
- Missing settings file returns defaults.
- `pull_repo` rejects when `confirmed` is false.
- `push_repo` rejects when `confirmed` is false.

Required frontend tests:

- Collapsed summary order is green/yellow/red/gray.
- Status counts update when a group is selected.
- Expanded list sorts `no_remote` last.
- Clicking a repo row shows contextual actions.
- Pull action opens confirmation before invoking backend.
- Settings navigation switches sections.

## 14. Acceptance Criteria

The implementation is acceptable when:

- App starts as a lightweight desktop widget.
- Collapsed widget has no leading icon.
- Collapsed summary order is green, yellow, red, gray.
- Expanded widget shows group filter first and status filter second.
- Status counts change with selected group.
- Expanded list shows exact five Git relation states.
- `无远端` is gray and always last.
- Branch column is visually lighter than numeric/change columns.
- Selecting a repo shows `目录`, `远端`, `Fetch`, and applicable guarded `Pull` or `Push` actions.
- `Pull` requires confirmation.
- Settings page includes `仓库`, `分组`, `刷新`, `安全操作`, `外观`.
- Repository scanning and manual add work.
- Opening remote URL works for GitHub-style remotes.
- No emoji icons are used.
- Frontend build, frontend tests, Rust tests, and Tauri compile pass.

## 15. Reference Implementation Plan

There is also a task-by-task implementation plan at:

```text
docs/superpowers/plans/2026-05-10-gitaview-desktop-widget.md
```

Use that file as the execution checklist. Use this document as the product and architecture source of truth.
