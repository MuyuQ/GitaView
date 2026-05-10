# Claude Fix Guide: Read This Before Touching Code

This file is a strict repair guide for the current GitaView codebase. Do not redesign the app. Do not add new features before fixing the issues below. Do not delete unrelated files.

## Current Verified Baseline

These commands currently pass:

```powershell
npm test
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
npm run tauri build
```

Your job is to preserve this baseline while fixing the real product issues found in review.

## Hard Rules

- Do not delete `.sisyphus/ralph-loop.local.md`.
- Do not revert icon files unless explicitly asked.
- Do not remove `create-icon.cjs` unless explicitly asked.
- Do not reintroduce fixture repository data.
- Do not replace the current Chinese graphite-style UI with generic SaaS styling.
- Do not commit code that only passes tests but is not actually used by the UI.
- After each fix, run the verification commands listed in that task.

## Fix 0: Restore Deleted Local File

### Problem

The working tree currently shows `.sisyphus/ralph-loop.local.md` as deleted. This file was explicitly called out as unrelated and should not be touched.

### Required Fix

Restore it:

```powershell
git restore .sisyphus/ralph-loop.local.md
```

### Verify

```powershell
git status --short
```

Expected: `.sisyphus/ralph-loop.local.md` should no longer appear as deleted.

Do this before any other task.

---

## Fix 1: Resize The Tauri Window For Collapsed, Expanded, And Settings Views

### Problem

`src-tauri/tauri.conf.json` sets the main window to `360x80`, `resizable: false`.

But the actual UI sizes are:

- Collapsed widget: roughly 280px wide.
- Expanded widget: `680px` wide in `src/styles/widget.css`.
- Settings window: `720px` wide and `480px` tall in `src/styles/settings.css`.

This means the built desktop app can compile but still crop the expanded/settings UI badly.

### Required Behavior

- Collapsed view should use a compact window size.
- Expanded view should resize to fit the expanded widget.
- Settings view should resize to fit the settings window.
- Window should remain undecorated, transparent, and always-on-top.

### Files To Modify

- `src/App.tsx`
- `src-tauri/capabilities/default.json`
- Possibly `package.json` if adding a Tauri window API package is required.

### Implementation Guidance

Use Tauri window APIs from `@tauri-apps/api/window`.

In `src/App.tsx`, import:

```ts
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
```

Add a view-size helper:

```ts
const windowSizes = {
  collapsed: new LogicalSize(300, 72),
  expanded: new LogicalSize(720, 560),
  settings: new LogicalSize(760, 540),
} as const;
```

Add an effect:

```ts
useEffect(() => {
  const appWindow = getCurrentWindow();
  const size = windowSizes[view === "settings" || repos.length === 0 ? "settings" : view];
  appWindow.setSize(size).catch((err) => {
    console.error("调整窗口尺寸失败", err);
  });
}, [view, repos.length]);
```

If Tauri requires permissions, update `src-tauri/capabilities/default.json` with the minimum required window permission. Do not grant broad permissions unless necessary.

### Verify

```powershell
npm test
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
npm run tauri build
```

Manual check:

```powershell
npm run tauri dev
```

Expected:

- Collapsed widget is not surrounded by a giant blank window.
- Expanded widget is not cropped.
- Settings page is not cropped.

---

## Fix 2: Make Settings Schema Backward Compatible

### Problem

`src-tauri/src/domain/settings.rs` added `confirm_push` to `SafetySettings`, but old `settings.json` files will not have `confirmPush`.

Current risk:

- Old settings JSON loads.
- Serde sees missing `confirmPush`.
- Deserialization fails.
- App shows `加载仓库失败`.

### Required Behavior

Old settings files without `confirmPush` must load successfully and default `confirmPush` to `true`.

### Files To Modify

- `src-tauri/src/domain/settings.rs`
- `src-tauri/src/storage/store.rs`

### Implementation Guidance

Add a default helper:

```rust
fn default_true() -> bool {
    true
}
```

Update `SafetySettings`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SafetySettings {
    #[serde(default = "default_true")]
    pub confirm_pull: bool,
    #[serde(default = "default_true")]
    pub confirm_push: bool,
}
```

Do not only fix this in frontend TypeScript. The failure happens in Rust while loading JSON.

### Add Test

In `src-tauri/src/storage/store.rs`, add a test that writes JSON without `confirmPush`:

```rust
#[test]
fn load_settings_defaults_missing_confirm_push() {
    let path = std::env::temp_dir().join("gitaview_missing_confirm_push.json");
    let _ = fs::remove_file(&path);
    fs::write(
        &path,
        r#"{
          "repos": [],
          "groups": [{ "name": "全部分组", "repoIds": [] }],
          "defaultGroup": "全部分组",
          "refresh": { "lightweightRefreshEnabled": true, "intervalMinutes": 5 },
          "safety": { "confirmPull": true },
          "appearance": { "compactMode": false }
        }"#,
    ).unwrap();
    let settings = load_settings(&path).unwrap();
    assert!(settings.safety.confirm_pull);
    assert!(settings.safety.confirm_push);
    let _ = fs::remove_file(&path);
}
```

### Verify

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
npm test
npm run build
```

Expected: all pass.

---

## Fix 3: Do Not Let One Broken Repo Break The Whole List

### Problem

`src-tauri/src/app_commands.rs` currently does this:

```rust
let state = branch_state(&repo.path)?;
```

Inside a loop. If one repository path is deleted, corrupted, or temporarily inaccessible, the entire `list_repo_statuses` command fails.

### Required Behavior

The app should still show the rest of the repositories. The broken repository should appear as a row with an error hint.

### Files To Modify

- `src-tauri/src/app_commands.rs`
- `src-tauri/src/domain/status.rs` if you decide to add a new status
- `src/types.ts` if you add a new frontend relation
- `src/lib/statusModel.ts` if you add a new relation

### Preferred Minimal Fix

Do not add a new relation yet. Use existing `no_remote` as a degraded state for broken repos, with a clear hint.

In `list_repo_statuses`, replace the failing loop with match logic:

```rust
for repo in settings.repos {
    match branch_state(&repo.path) {
        Ok(state) => statuses.push(RepoStatusDto {
            id: repo.id,
            name: repo.name,
            path: repo.path.to_string_lossy().to_string(),
            group: repo.group,
            branch: state.branch.clone(),
            relation: state.relation,
            change_label: change_label(&state),
            hint: relation_hint(state.relation).to_string(),
            remote_url: state.remote_url,
        }),
        Err(err) => statuses.push(RepoStatusDto {
            id: repo.id,
            name: repo.name,
            path: repo.path.to_string_lossy().to_string(),
            group: repo.group,
            branch: "未知".to_string(),
            relation: crate::domain::status::RemoteRelation::NoRemote,
            change_label: "!".to_string(),
            hint: format!("读取失败：{err}"),
            remote_url: None,
        }),
    }
}
```

This is not semantically perfect, but it prevents a bad repo from killing the whole widget.

### Better Fix Later

Later, add a separate `error` relation. Do not do that in this patch unless you update all sort/filter/display tests.

### Verify

Add a Rust test if you extract a helper function for mapping repo + result to DTO. If you do not extract a helper, at minimum run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
npm test
npm run build
```

Manual check:

- Add two repos.
- Delete or rename one repo folder.
- Refresh.
- Expected: the remaining repo still appears.

---

## Fix 4: Use The Tested `filterRepos` Function In The Real UI

### Problem

`src/lib/statusModel.ts` exports `filterRepos`, and tests cover it. But `src/components/WidgetExpanded.tsx` still reimplements filtering manually.

Tests are less useful if the production UI does not use the tested function.

### Required Fix

In `src/components/WidgetExpanded.tsx`, import:

```ts
import { filterRepos } from "../lib/statusModel";
```

Remove the local `sortRepos` import and manual filtering code.

Use:

```ts
const groupRepos = group === "全部分组" ? repos : repos.filter((repo) => repo.group === group);
const visibleRepos = filterRepos(repos, group, relation, query);
```

Keep `groupRepos` only because `StatusFilters` needs the selected group scope.

### Verify

```powershell
npm test
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
```

Expected: all pass.

---

## Fix 5: Background Refresh Should Not Blank The Whole UI

### Problem

`src/App.tsx` uses one `loading` state for both first load and later refreshes.

Current behavior:

- User is in expanded view.
- User clicks refresh.
- UI disappears and shows `正在刷新仓库状态...`.
- After refresh, view comes back.

This feels broken and jittery.

### Required Behavior

- First load can show full loading.
- Later refresh should keep current UI visible.
- Refresh button can show a small busy state.

### Files To Modify

- `src/App.tsx`
- `src/components/WidgetExpanded.tsx`

### Implementation Guidance

In `src/App.tsx`, split state:

```ts
const [initialLoading, setInitialLoading] = useState(true);
const [refreshing, setRefreshing] = useState(false);
```

Update `refreshRepos`:

```ts
function refreshRepos() {
  setRefreshing(true);
  setError(null);
  listRepoStatuses()
    .then((data) => {
      setRepos(data);
      setLastRefreshAt(new Date());
    })
    .catch((err) => {
      setError(String(err));
    })
    .finally(() => {
      setInitialLoading(false);
      setRefreshing(false);
    });
}
```

Use:

```ts
if (initialLoading) return <main className="app-shell">正在刷新仓库状态...</main>;
```

Pass `refreshing` into `WidgetExpanded`.

In `WidgetExpanded`, button text:

```tsx
{refreshing ? "刷新中" : "刷新"}
```

Disable refresh button while refreshing.

### Verify

```powershell
npm test
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
```

Manual check:

- Open expanded widget.
- Click refresh.
- Expected: table remains visible; only refresh button changes state.

---

## Fix 6: Make Refresh Settings Actually Do Something Or Rename Them

### Problem

`RefreshSettings` saves:

- `lightweightRefreshEnabled`
- `intervalMinutes`

But the app never uses these settings. A setting that does nothing is worse than no setting.

### Option A: Implement It

Files:

- `src/App.tsx`
- `src/lib/commands.ts`

Implementation:

- Load settings in `App`.
- If `refresh.lightweightRefreshEnabled` is true, set an interval.
- Interval calls `refreshRepos`.
- Clear interval on unmount.
- Reload refresh settings after closing settings page.

Sketch:

```ts
const [refreshSettings, setRefreshSettings] = useState<AppSettings["refresh"] | null>(null);
```

Load:

```ts
getSettings().then((settings) => setRefreshSettings(settings.refresh));
```

Effect:

```ts
useEffect(() => {
  if (!refreshSettings?.lightweightRefreshEnabled) return;
  const id = window.setInterval(refreshRepos, refreshSettings.intervalMinutes * 60_000);
  return () => window.clearInterval(id);
}, [refreshSettings]);
```

Be careful: avoid creating multiple intervals accidentally.

### Option B: Rename UI Clearly

If not implementing interval refresh now, rename settings copy to:

```text
刷新偏好（暂未自动执行）
```

But Option A is preferred.

### Verify

```powershell
npm test
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
```

Manual check:

- Set interval to 1 minute.
- Wait.
- Expected: refresh time updates without clicking refresh.

---

## Final Verification Checklist

Before saying the work is done, run:

```powershell
git status --short
npm test
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
npm run tauri build
```

Expected:

- No unexpected deleted files.
- All tests pass.
- Frontend build passes.
- Rust tests pass.
- Tauri build passes.

Manual checks:

- Collapsed window is compact.
- Expanded window is not cropped.
- Settings window is not cropped.
- Existing old `settings.json` without `confirmPush` still loads.
- One broken repo does not break the whole repo list.
- Search/filter behavior still works.
- Refresh does not blank the UI after first load.
- Refresh interval either works or the UI copy clearly says it is not automatic yet.

