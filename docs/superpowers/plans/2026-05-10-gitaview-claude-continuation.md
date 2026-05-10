# GitaView Claude Continuation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish GitaView from the current verified first pass into a usable Tauri desktop widget with reliable Git operations, usable settings, and polished Chinese UI.

**Architecture:** Keep the existing Tauri 2 + Rust backend + React/TypeScript/Vite frontend. Rust owns filesystem, settings persistence, and Git command execution. React owns state, filtering, presentation, and user confirmations.

**Tech Stack:** Tauri 2, Rust 1.95, React 19, TypeScript 5.9, Vite 7, Vitest.

---

## Current Baseline

These already pass as of 2026-05-10:

```powershell
npm test
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
```

Current known working areas:

- Rust settings persistence exists in `src-tauri/src/app_commands.rs`.
- Rust Git status detection exists in `src-tauri/src/git/commands.rs`.
- Frontend widget UI exists in `src/components/WidgetCollapsed.tsx`, `src/components/WidgetExpanded.tsx`, `src/components/RepoTable.tsx`, and `src/components/RepoActions.tsx`.
- Settings shell and initial functional settings exist under `src/components/settings/`.
- Design requirements already established: Chinese UI, collapsed widget without left icon, gray "无远端" last, row actions appear after row click, no checkboxes, remote URL button exists, low-noise graphite visual style.

Important guardrails:

- Do not revert unrelated working tree changes: `.sisyphus/ralph-loop.local.md`, `src-tauri/icons/*`, `create-icon.cjs`.
- Do not replace the current design language with generic purple SaaS UI.
- Do not reintroduce fixture repository data.
- Every task must end with `npm test`, `npm run build`, and `cargo test --manifest-path src-tauri\Cargo.toml` unless the task only changes docs.

---

## File Responsibility Map

- `src-tauri/src/app_commands.rs`: Tauri command boundary. Keep this thin: load settings, find repo, call Git/storage helpers, return DTOs.
- `src-tauri/src/git/commands.rs`: Git command execution, remote URL normalization, branch relation calculation, timeout/non-interactive behavior.
- `src-tauri/src/git/discovery.rs`: Directory scanning and Git repository detection.
- `src-tauri/src/domain/settings.rs`: Persisted settings schema and defaults.
- `src-tauri/src/domain/repo.rs`: Repo DTOs shared with frontend.
- `src-tauri/src/domain/status.rs`: Remote relation enums, sort order, collapsed buckets.
- `src-tauri/src/storage/store.rs`: JSON settings load/save.
- `src/lib/commands.ts`: Typed frontend wrappers around Tauri commands.
- `src/lib/statusModel.ts`: Frontend sorting, grouping, and collapsed summary logic.
- `src/App.tsx`: Top-level widget/settings view state and refresh orchestration.
- `src/components/RepoActions.tsx`: Row action buttons and operation feedback.
- `src/components/RepoTable.tsx`: Expanded repository table and selected-row action area.
- `src/components/GroupFilters.tsx`: Primary group filter.
- `src/components/StatusFilters.tsx`: Secondary status filter, dependent on selected group.
- `src/components/settings/*.tsx`: Settings sections.
- `src/styles/widget.css`: Widget visual system.
- `src/styles/settings.css`: Settings visual system.

---

## Task 1: Add Real Refresh State And Manual Refresh Button

**Problem:** `WidgetExpanded` still displays hardcoded `刚刚刷新`. Fetch/Pull does not refresh repository status after completing.

**Files:**

- Modify: `src/App.tsx`
- Modify: `src/components/WidgetExpanded.tsx`
- Modify: `src/components/RepoTable.tsx`
- Modify: `src/components/RepoActions.tsx`

- [ ] **Step 1: Add refresh metadata in `src/App.tsx`**

Add state:

```ts
const [lastRefreshAt, setLastRefreshAt] = useState<Date | null>(null);
```

Update `refreshRepos()` so success sets it:

```ts
function refreshRepos() {
  setLoading(true);
  setError(null);
  listRepoStatuses()
    .then((data) => {
      setRepos(data);
      setLastRefreshAt(new Date());
      setLoading(false);
    })
    .catch((err) => {
      setError(String(err));
      setLoading(false);
    });
}
```

- [ ] **Step 2: Pass refresh props into `WidgetExpanded`**

Use:

```tsx
<WidgetExpanded
  repos={repos}
  lastRefreshAt={lastRefreshAt}
  onRefresh={refreshRepos}
  onCollapse={() => setView("collapsed")}
  onOpenSettings={() => setView("settings")}
/>
```

- [ ] **Step 3: Update `WidgetExpanded` props**

Expected props:

```ts
{
  repos: RepoStatus[];
  lastRefreshAt: Date | null;
  onRefresh: () => void;
  onCollapse: () => void;
  onOpenSettings: () => void;
}
```

Display refresh time:

```tsx
<p>{lastRefreshAt ? `刷新时间 ${lastRefreshAt.toLocaleTimeString("zh-CN", { hour12: false })}` : "尚未刷新"}</p>
```

Add a small button:

```tsx
<button className="collapse-btn" onClick={onRefresh} aria-label="刷新">
  刷新
</button>
```

- [ ] **Step 4: Refresh after row actions**

Pass `onRefresh` from `WidgetExpanded` into `RepoTable`, then into `RepoActions`.

In `RepoActions`, after successful `Fetch` or `Pull`, call `onRefresh()`.

Use this behavior:

```ts
const shouldRefresh = action === "Fetch" || action === "Pull";
if (shouldRefresh) onRefresh();
```

- [ ] **Step 5: Verify**

Run:

```powershell
npm test
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
```

Expected: all pass.

---

## Task 2: Implement Search Input In Expanded Widget

**Problem:** The search input exists but does nothing.

**Files:**

- Modify: `src/components/WidgetExpanded.tsx`

- [ ] **Step 1: Add search state**

```ts
const [query, setQuery] = useState("");
```

- [ ] **Step 2: Apply query after group/status filtering**

Use case-insensitive search over repo name, path, branch, and group:

```ts
const normalizedQuery = query.trim().toLowerCase();
const searchedRepos = normalizedQuery
  ? visibleRepos.filter((repo) =>
      [repo.name, repo.path, repo.branch, repo.group]
        .join(" ")
        .toLowerCase()
        .includes(normalizedQuery),
    )
  : visibleRepos;
```

Pass `searchedRepos` to `RepoTable`.

- [ ] **Step 3: Wire input**

```tsx
<input
  aria-label="搜索仓库"
  placeholder="搜索仓库 / 分支"
  value={query}
  onChange={(event) => setQuery(event.target.value)}
/>
```

- [ ] **Step 4: Add empty state**

If search returns zero, show:

```tsx
<p className="repo-empty">没有匹配的仓库</p>
```

Style it in `src/styles/widget.css`:

```css
.repo-empty {
  margin: 12px 16px 16px;
  color: var(--gv-muted);
  font-size: 12px;
}
```

- [ ] **Step 5: Verify**

Run:

```powershell
npm test
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
```

Expected: all pass.

---

## Task 3: Make Git Commands Non-Interactive And Timeout Safe

**Problem:** `GIT_OPERATION_TIMEOUT` exists but is unused. Git commands can hang on auth prompts or slow network.

**Files:**

- Modify: `src-tauri/src/git/commands.rs`
- Test: `src-tauri/src/git/commands.rs`

- [ ] **Step 1: Add non-interactive env vars to `run_git`**

Update command construction:

```rust
let output = Command::new("git")
    .args(args)
    .current_dir(repo_path)
    .env("GIT_TERMINAL_PROMPT", "0")
    .env("GCM_INTERACTIVE", "Never")
    .output()
    .map_err(|err| format!("failed to run git {:?}: {}", args, err))?;
```

- [ ] **Step 2: Decide timeout implementation**

Use a small helper with `std::process::Child` and polling. Do not add a large dependency just for this.

Target helper signature:

```rust
fn run_command_with_timeout(mut command: Command, timeout: Duration) -> Result<std::process::Output, String>
```

Behavior:

- Spawn process.
- Poll `try_wait()` every 50ms.
- If timeout expires, kill the child and return `git command timed out`.
- If process exits, collect output with `wait_with_output()`.

- [ ] **Step 3: Replace `.output()` call**

Use:

```rust
let output = run_command_with_timeout(command, GIT_OPERATION_TIMEOUT)?;
```

- [ ] **Step 4: Add testable helper for timeout message**

If direct timeout test is hard on Windows, at minimum add a unit test for a new pure function:

```rust
fn format_git_failure(args: &[&str], stderr: &[u8]) -> String
```

Expected:

```rust
assert_eq!(
    format_git_failure(&["fetch"], b"fatal: failed"),
    "git fetch failed: fatal: failed"
);
```

- [ ] **Step 5: Verify**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
npm test
npm run build
```

Expected: all pass.

---

## Task 4: Add Push Action For `local_ahead`

**Problem:** UI says `可 Push` for local-ahead repositories, but there is no Push button.

**Files:**

- Modify: `src-tauri/src/app_commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/commands.ts`
- Modify: `src/components/RepoActions.tsx`
- Modify: `src-tauri/src/domain/settings.rs`
- Modify: `src/types.ts`

- [ ] **Step 1: Add safety setting**

Rust `SafetySettings`:

```rust
pub struct SafetySettings {
    pub confirm_pull: bool,
    pub confirm_push: bool,
}
```

Default:

```rust
safety: SafetySettings {
    confirm_pull: true,
    confirm_push: true,
},
```

Frontend `AppSettings`:

```ts
safety: {
  confirmPull: boolean;
  confirmPush: boolean;
};
```

- [ ] **Step 2: Add Tauri command**

In `app_commands.rs`:

```rust
#[tauri::command]
pub async fn push_repo(app: tauri::AppHandle, repo_id: String, confirmed: bool) -> Result<String, String> {
    let settings = load_app_settings(&app)?;
    if settings.safety.confirm_push && !confirmed {
        return Err("Push 需要确认".to_string());
    }
    let repo = find_repo(&settings, &repo_id)?;
    run_git(&repo.path, &["push"])?;
    Ok("Push 已完成".to_string())
}
```

Register it in `src-tauri/src/lib.rs` inside `tauri::generate_handler![...]`.

- [ ] **Step 3: Add frontend command wrapper**

In `src/lib/commands.ts`:

```ts
export function pushRepo(repoId: string, confirmed: boolean): Promise<string> {
  return invoke<string>("push_repo", { repoId, confirmed });
}
```

- [ ] **Step 4: Add Push button**

In `RepoActions`, show Push only when `repo.relation === "local_ahead"` or `repo.relation === "diverged"`.

Text:

- First click: `Push`
- Confirm state: `确认 Push`
- Warning: `Push 会更新远端分支，是否继续？`

Call:

```ts
runAction("Push", () => pushRepo(repo.id, true));
```

Refresh after success.

- [ ] **Step 5: Update safety settings UI**

In `src/components/settings/SafetySettings.tsx`, add checkbox:

```tsx
<input
  type="checkbox"
  checked={confirmPush}
  onChange={(event) => setConfirmPush(event.target.checked)}
/> Push 操作需要确认
```

Persist both values:

```ts
safety: { confirmPull, confirmPush }
```

- [ ] **Step 6: Verify**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
npm test
npm run build
```

Expected: all pass.

---

## Task 5: Add Folder Picker Instead Of Manual-Only Path Input

**Problem:** Repository settings currently requires typing paths manually. Desktop app should use folder picker.

**Files:**

- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`
- Modify: `package.json`
- Modify: `src/components/settings/RepositorySettings.tsx`

- [ ] **Step 1: Add Tauri dialog plugin**

Install:

```powershell
npm install @tauri-apps/plugin-dialog
cargo add tauri-plugin-dialog --manifest-path src-tauri\Cargo.toml
```

Register in `src-tauri/src/lib.rs`:

```rust
.plugin(tauri_plugin_dialog::init())
```

- [ ] **Step 2: Add folder picker button**

In `RepositorySettings.tsx`:

```ts
import { open } from "@tauri-apps/plugin-dialog";
```

Handler:

```ts
async function handlePickDirectory() {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "选择仓库或扫描根目录",
  });
  if (typeof selected === "string") {
    setPath(selected);
  }
}
```

Add button before scan:

```tsx
<button onClick={handlePickDirectory} disabled={busy}>选择目录</button>
```

Update grid columns in `settings.css`:

```css
.settings-path-row {
  grid-template-columns: minmax(0, 1fr) auto auto auto;
}
```

- [ ] **Step 3: Verify**

Run:

```powershell
npm test
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
npm run tauri dev
```

Expected:

- Folder picker opens.
- Selected path fills input.
- Scan and add still work.

---

## Task 6: Improve Repository Discovery

**Problem:** `is_git_repo` only checks for `.git` path existence. Worktrees and submodules can have `.git` as a file, not a directory. Scan can also traverse huge ignored directories.

**Files:**

- Modify: `src-tauri/src/git/discovery.rs`
- Test: `src-tauri/src/git/discovery.rs`

- [ ] **Step 1: Accept `.git` file or directory**

Implementation:

```rust
pub fn is_git_repo(path: &Path) -> bool {
    path.join(".git").exists()
}
```

Keep this behavior, but add explicit tests for file and directory.

- [ ] **Step 2: Skip heavy folders while scanning**

Add helper:

```rust
fn should_skip_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("node_modules" | "target" | ".venv" | "dist" | "build" | ".next")
    )
}
```

In `scan_inner`, before descending:

```rust
if child.is_dir() && !should_skip_dir(&child) {
    scan_inner(&child, found);
}
```

- [ ] **Step 3: Add tests**

Add:

```rust
#[test]
fn detects_git_repo_by_dot_git_file() {
    let temp = std::env::temp_dir().join("gitaview_detect_worktree_test");
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&temp).unwrap();
    fs::write(temp.join(".git"), "gitdir: ../real/.git/worktrees/example").unwrap();
    assert!(is_git_repo(&temp));
    let _ = fs::remove_dir_all(&temp);
}
```

Add a skip test with `node_modules` containing a fake repo and assert it is not returned.

- [ ] **Step 4: Verify**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
npm test
npm run build
```

Expected: all pass.

---

## Task 7: Settings Schema Hardening And Migration

**Problem:** Future settings changes can break old JSON. `appearance` is defaulted, but missing groups/default group can still cause odd UI states.

**Files:**

- Modify: `src-tauri/src/storage/store.rs`
- Modify: `src-tauri/src/domain/settings.rs`
- Test: `src-tauri/src/storage/store.rs`

- [ ] **Step 1: Add normalization function**

In `domain/settings.rs`:

```rust
impl AppSettings {
    pub fn normalized(mut self) -> Self {
        if self.default_group.trim().is_empty() {
            self.default_group = "全部分组".to_string();
        }
        if !self.groups.iter().any(|group| group.name == self.default_group) {
            self.groups.insert(0, GroupRecord {
                name: self.default_group.clone(),
                repo_ids: Vec::new(),
            });
        }
        for repo in &mut self.repos {
            if repo.group.trim().is_empty() || !self.groups.iter().any(|group| group.name == repo.group) {
                repo.group = self.default_group.clone();
            }
        }
        self
    }
}
```

- [ ] **Step 2: Normalize after load**

In `storage/store.rs`:

```rust
serde_json::from_str::<AppSettings>(&text)
    .map(|settings| settings.normalized())
    .map_err(|err| err.to_string())
```

- [ ] **Step 3: Test missing group repair**

Add test:

```rust
#[test]
fn load_settings_repairs_missing_default_group() {
    let path = std::env::temp_dir().join("gitaview_settings_repair.json");
    let _ = fs::remove_file(&path);
    fs::write(
        &path,
        r#"{
          "repos": [],
          "groups": [],
          "defaultGroup": "全部分组",
          "refresh": { "lightweightRefreshEnabled": true, "intervalMinutes": 5 },
          "safety": { "confirmPull": true, "confirmPush": true },
          "appearance": { "compactMode": false }
        }"#,
    ).unwrap();
    let settings = load_settings(&path).unwrap();
    assert!(settings.groups.iter().any(|group| group.name == "全部分组"));
    let _ = fs::remove_file(&path);
}
```

- [ ] **Step 4: Verify**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
npm test
npm run build
```

Expected: all pass.

---

## Task 8: Visual QA And Responsive Fixes

**Problem:** Frontend builds, but it still needs visual QA in the actual Tauri/web runtime.

**Files:**

- Modify as needed: `src/styles/widget.css`
- Modify as needed: `src/styles/settings.css`
- Modify as needed: `src/components/*.tsx`

- [ ] **Step 1: Run app**

```powershell
npm run tauri dev
```

- [ ] **Step 2: Test collapsed state**

Expected:

- No icon on far left of collapsed main button.
- `GitaView` text is not oversized.
- Number is crisp.
- Dots appear in order: green, amber, red, gray.
- Gray no-remote count appears last.
- Settings gear is vertically centered and not visually heavier than collapsed widget.

- [ ] **Step 3: Test expanded state**

Expected:

- Columns align: status, repo, group, branch, relation, change, hint.
- Branch column is lighter than numeric/change column but not blurry.
- Row action buttons are vertically centered.
- Buttons are compact, not oversized.
- Clicking a row reveals actions only for that row.
- No checkboxes anywhere in repo table.
- "摘要 / 详情" text does not exist.
- Only refresh time is shown.

- [ ] **Step 4: Test settings state**

Expected:

- Left navigation labels: 仓库 / 分组 / 刷新 / 安全操作 / 外观.
- `完成` button returns to collapsed widget and refreshes status.
- Repository list does not overflow horizontally with long paths.
- Group select is readable.
- Buttons have consistent height.

- [ ] **Step 5: Fix any issues and verify**

Run:

```powershell
npm test
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
```

Expected: all pass.

---

## Task 9: Add Frontend Tests For Status Filtering And Search

**Problem:** Only `statusModel` has minimal tests. The two-dimensional filtering behavior is important and should not regress.

**Files:**

- Modify: `src/lib/statusModel.ts`
- Test: `src/lib/statusModel.test.ts`

- [ ] **Step 1: Extract filtering into pure function**

Add to `statusModel.ts`:

```ts
export function filterRepos(
  repos: RepoStatus[],
  group: string,
  relation: RemoteRelation | "all",
  query: string,
): RepoStatus[] {
  const groupRepos = group === "全部分组" ? repos : repos.filter((repo) => repo.group === group);
  const relationRepos = relation === "all" ? groupRepos : groupRepos.filter((repo) => repo.relation === relation);
  const normalizedQuery = query.trim().toLowerCase();
  const searchedRepos = normalizedQuery
    ? relationRepos.filter((repo) =>
        [repo.name, repo.path, repo.branch, repo.group].join(" ").toLowerCase().includes(normalizedQuery),
      )
    : relationRepos;
  return sortRepos(searchedRepos);
}
```

Use this function in `WidgetExpanded`.

- [ ] **Step 2: Add tests**

Add tests that assert:

- `全部分组` includes all repos.
- Selecting a group changes available repo set.
- Selecting `no_remote` keeps no-remote repos last in sorted all view but filters correctly when selected.
- Query matches repo name and branch.

- [ ] **Step 3: Verify**

Run:

```powershell
npm test
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
```

Expected: all pass.

---

## Task 10: Package Build Smoke Test

**Problem:** Unit tests and frontend build pass, but desktop packaging may still fail due Tauri config, icons, permissions, or plugin setup.

**Files:**

- Modify only if build fails: `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, icon files, plugin setup.

- [ ] **Step 1: Run package build**

```powershell
npm run tauri build
```

- [ ] **Step 2: If build fails due Windows WebView or Visual Studio tools**

Do not hack around it in code. Install the missing platform dependency, then rerun.

- [ ] **Step 3: If build fails due icon format**

Regenerate icons using the existing `create-icon.cjs` if it is intended for this repo. Do not delete current icon changes without asking.

- [ ] **Step 4: Verify final package**

Expected:

- Build produces installer/bundle under `src-tauri/target/release/bundle`.
- App launches.
- Tray/widget appears.
- Settings opens.
- Add a test repository.
- Status loads.
- Open directory works.
- Open remote works for GitHub HTTPS/SSH remotes.

---

## Final Acceptance Checklist

Claude should not call the work complete until all of these are true:

- [ ] `npm test` passes.
- [ ] `npm run build` passes.
- [ ] `cargo test --manifest-path src-tauri\Cargo.toml` passes.
- [ ] `npm run tauri dev` launches the app.
- [ ] `npm run tauri build` completes or any platform dependency failure is documented clearly.
- [ ] No fixture repository data is shown.
- [ ] Empty app opens settings instead of dead-ending.
- [ ] User can add a real Git repo.
- [ ] User can scan a directory for repos.
- [ ] User can assign repos to groups.
- [ ] Group filter and status filter interact correctly.
- [ ] Search input filters repos.
- [ ] Fetch/Pull/Push update status after success.
- [ ] Pull and Push confirmations respect settings.
- [ ] Open directory works on Windows.
- [ ] Open remote works for `git@github.com:owner/repo.git` and `https://github.com/owner/repo.git`.
- [ ] `无远端` is gray and sorted last in collapsed and expanded states.
- [ ] Collapsed state has no leading icon in the main button.
- [ ] Expanded row columns remain aligned.
- [ ] Settings UI is Chinese and visually consistent with the widget.

