# Claude Review Report 2: Remaining Fixes After Latest Review

This report covers the latest review after commit `3a5845b fix: apply CLAUDE_FIX_GUIDE.md fixes 0-6` and `10cdabe chore: add CLAUDE_FIX_GUIDE.md`.

The baseline is mostly healthy, but several issues remain. Do not add new features until these are fixed.

## Verification Results

These commands currently pass:

```powershell
npm test
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
npm run tauri build
```

Passing these commands is necessary, but not sufficient. The issues below are product/behavior problems that tests do not fully catch.

## Current Remaining Issues

### P1: `.sisyphus/ralph-loop.local.md` Is Still Deleted

**File:** `.sisyphus/ralph-loop.local.md`

**Problem:** The working tree still shows this file as deleted:

```text
D .sisyphus/ralph-loop.local.md
```

This was explicitly marked as unrelated and must not be touched.

**Required fix:**

```powershell
git restore .sisyphus/ralph-loop.local.md
```

**Verify:**

```powershell
git status --short
```

Expected: `.sisyphus/ralph-loop.local.md` must not appear.

Do this first. Do not commit unrelated deletion.

---

### P2: Background Refresh Failure Still Replaces The Whole UI

**File:** `src/App.tsx`

**Problem:** `App.tsx` now has `initialLoading` and `refreshing`, which is good. But any later refresh error still sets global `error`, and this line replaces the whole UI:

```tsx
if (error) return <main className="app-shell" role="alert">加载仓库失败：{error}</main>;
```

That means:

- User opens expanded widget.
- A scheduled refresh fails once.
- The entire repo list disappears.
- User sees only the full-screen error.

This is not acceptable for background refresh.

**Required behavior:**

- Initial load failure may show a full-page error.
- Later refresh failure should keep the existing repo list visible.
- Later refresh failure should show a small inline warning, preferably in expanded widget.

**Required implementation:**

Replace the single `error` state with two states:

```ts
const [initialError, setInitialError] = useState<string | null>(null);
const [refreshError, setRefreshError] = useState<string | null>(null);
```

Update `refreshRepos`:

```ts
function refreshRepos() {
  setRefreshing(true);
  setRefreshError(null);
  listRepoStatuses()
    .then((data) => {
      setRepos(data);
      setLastRefreshAt(new Date());
      setInitialError(null);
    })
    .catch((err) => {
      const message = String(err);
      if (initialLoading && repos.length === 0) {
        setInitialError(message);
      } else {
        setRefreshError(message);
      }
    })
    .finally(() => {
      setInitialLoading(false);
      setRefreshing(false);
    });
}
```

Use full-page error only for initial failure:

```tsx
if (initialError && repos.length === 0) {
  return <main className="app-shell" role="alert">加载仓库失败：{initialError}</main>;
}
```

Pass `refreshError` into `WidgetExpanded`:

```tsx
<WidgetExpanded
  repos={repos}
  lastRefreshAt={lastRefreshAt}
  refreshing={refreshing}
  refreshError={refreshError}
  onRefresh={refreshRepos}
  onCollapse={() => setView("collapsed")}
  onOpenSettings={() => setView("settings")}
/>
```

Update `WidgetExpanded` props:

```ts
refreshError: string | null;
```

Render small warning below toolbar or near refresh time:

```tsx
{refreshError && <p className="refresh-warning" role="status">刷新失败：{refreshError}</p>}
```

Add style in `src/styles/widget.css`:

```css
.refresh-warning {
  margin: 0 16px 8px;
  color: var(--gv-red);
  font-size: 11px;
}
```

**Verify:**

```powershell
npm test
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
```

Manual check:

- Start app with at least one valid repo.
- Trigger a refresh failure by temporarily breaking `list_repo_statuses` or using an invalid repo setup.
- Expected: existing UI remains visible and only a small warning appears.

---

### P2: Broken Repositories Are Still Treated As `no_remote`

**File:** `src-tauri/src/app_commands.rs`

**Problem:** A repository that cannot be read is currently mapped to:

```rust
relation: crate::domain::status::RemoteRelation::NoRemote,
change_label: "!".to_string(),
hint: format!("读取失败：{err}"),
```

This prevents the whole list from failing, which is good. But it also sorts broken repos with normal no-remote repos at the bottom.

Product requirement: abnormal repositories should be visible first. A broken repo is abnormal.

**Preferred fix:** Add an explicit `Error` relation.

#### Rust changes

In `src-tauri/src/domain/status.rs`, add:

```rust
Error,
```

Update sort rank:

```rust
RemoteRelation::Error => 0,
RemoteRelation::Diverged => 1,
RemoteRelation::RemoteAhead => 2,
RemoteRelation::LocalAhead => 3,
RemoteRelation::Synced => 4,
RemoteRelation::NoRemote => 5,
```

Update collapsed bucket:

```rust
RemoteRelation::Error | RemoteRelation::Diverged => CollapsedBucket::NeedsAttention,
```

Update tests in the same file so error sorts before diverged and is counted as `needs_attention`.

In `src-tauri/src/app_commands.rs`, use:

```rust
relation: crate::domain::status::RemoteRelation::Error,
change_label: "!".to_string(),
hint: format!("读取失败：{err}"),
```

#### Frontend changes

In `src/types.ts`, add:

```ts
| "error"
```

to `RemoteRelation`.

In `src/lib/statusModel.ts`, update `expandedRank`:

```ts
error: 0,
diverged: 1,
remote_ahead: 2,
local_ahead: 3,
synced: 4,
no_remote: 5,
```

Update `toCollapsedBucket`:

```ts
if (relation === "error" || relation === "diverged") return "needs_attention";
```

In `src/components/RepoTable.tsx`, update relation labels:

```ts
error: "读取失败",
```

In status dot mapping, use red for `error`.

Update tests in `src/lib/statusModel.test.ts`:

- `error` sorts before `diverged`.
- `error` contributes to `needs_attention`.

**Verify:**

```powershell
npm test
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
```

Manual check:

- Add a valid repo.
- Add another repo and then rename/delete its folder.
- Refresh.
- Expected: broken repo appears at the top with red/error styling and hint `读取失败：...`.
- Valid repo still appears.

---

### P2: Auto Refresh Interval Needs Defensive Normalization

**Files:**

- `src/App.tsx`
- `src-tauri/src/domain/settings.rs`

**Problem:** `App.tsx` uses:

```ts
window.setInterval(refreshRepos, refreshSettings.intervalMinutes * 60_000);
```

If an old or manually edited settings file has `intervalMinutes: 0`, negative values, or a very small invalid value, the app can refresh too frequently.

**Required behavior:**

- Rust settings normalization should clamp `interval_minutes` to `1..=60`.
- Frontend should also defensively clamp before creating the interval.

**Rust fix:**

In `AppSettings::normalized`:

```rust
self.refresh.interval_minutes = self.refresh.interval_minutes.clamp(1, 60);
```

Add test in `src-tauri/src/storage/store.rs`:

```rust
#[test]
fn load_settings_clamps_refresh_interval() {
    let path = std::env::temp_dir().join("gitaview_refresh_interval_clamp.json");
    let _ = fs::remove_file(&path);
    fs::write(
        &path,
        r#"{
          "repos": [],
          "groups": [{ "name": "全部分组", "repoIds": [] }],
          "defaultGroup": "全部分组",
          "refresh": { "lightweightRefreshEnabled": true, "intervalMinutes": 0 },
          "safety": { "confirmPull": true, "confirmPush": true },
          "appearance": { "compactMode": false }
        }"#,
    ).unwrap();
    let settings = load_settings(&path).unwrap();
    assert_eq!(settings.refresh.interval_minutes, 1);
    let _ = fs::remove_file(&path);
}
```

**Frontend fix:**

In `src/App.tsx`:

```ts
const intervalMinutes = Math.min(Math.max(refreshSettings.intervalMinutes, 1), 60);
const id = window.setInterval(refreshRepos, intervalMinutes * 60_000);
```

**Verify:**

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
npm test
npm run build
```

---

### P3: Main Window Is User-Resizable

**File:** `src-tauri/tauri.conf.json`

**Problem:** The window is currently:

```json
"resizable": true
```

Programmatic resize is needed. User resize is probably not needed for a desktop widget and can make the widget feel broken.

**Recommended fix:**

Set:

```json
"resizable": false
```

Keep programmatic `setSize` in `src/App.tsx`. Tauri window APIs can still resize the window programmatically when permission is granted.

**Verify:**

```powershell
npm run tauri build
```

Manual check:

- Collapsed, expanded, and settings states still resize correctly.
- User cannot manually drag-resize the widget.

---

## Final Required Verification

After fixing all issues above, run:

```powershell
git status --short
npm test
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
npm run tauri build
```

Expected:

- No deleted `.sisyphus/ralph-loop.local.md`.
- Frontend tests pass.
- Frontend build passes.
- Rust tests pass.
- Tauri build passes.

Manual checks:

- Expanded widget is not cropped.
- Settings page is not cropped.
- Background refresh failure does not replace the whole UI.
- Broken repo appears at the top as an error/attention item.
- Auto refresh interval cannot become 0ms.
- Window cannot be manually resized if `resizable` is set to false.

