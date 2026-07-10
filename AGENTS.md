# GitaView

Tauri 2 + Rust + React + TypeScript desktop Git status widget. Manages its own
repo list; no dependency on `gita` CLI.

## Commands

```bash
npm ci                                  # install deps
npm run tauri dev                       # dev mode (Vite + Tauri window)
npm test                                # Vitest (frontend)
npm run build                           # tsc --noEmit + vite build (this IS the typecheck)
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

CI validation order (ci.yml): `npm test` → `cargo test` → `cargo fmt --check`
→ `cargo check --target <triple>` → `cargo clippy`. No Linux targets—macOS
(aarch64 + x86_64) and Windows only.

Release: push a `v*` tag. `release.yml` validates version + signing secrets,
runs the same test suite, then builds signed installers via `tauri-apps/tauri-action`.

## Architecture

| Layer | Key files | Notes |
|-------|-----------|-------|
| Tauri commands | `src-tauri/src/app_commands.rs` | All IPC commands exposed to React; fixed arg arrays only |
| Git execution | `src-tauri/src/git/commands.rs` | Subprocess with fixed args; `run_git`, `run_git_args` |
| Git worktree | `src-tauri/src/git/worktree.rs` | Preflight: Clean/Dirty/Conflicted/Detached |
| Desktop widget | `src-tauri/src/desktop_widget/{macos,windows,unsupported}.rs` | Platform-conditional via `cfg(target_os)` |
| Persistence | `src-tauri/src/storage/store.rs` | Atomic JSON writes (temp file + rename) |
| Tray | `src-tauri/src/tray_status.rs` | Async refresh with generation guard |
| Status model | `src/lib/statusModel.ts` | Sorting, filtering, bucketing, action availability |
| React entry | `src/App.tsx` | Widget modes, refresh lifecycle |
| Settings UI | `src/components/settings/` | Repo, group, refresh, safety, appearance |
| Product spec | `DESIGN_AND_BUILD_SPEC.md` | v1 contract |

Rust crate name is `gitaview_lib` (see `src-tauri/Cargo.toml` `[lib]`).
Frontend package name in package.json is `gitaview`.

## Tests

- Frontend: Vitest, colocated `src/**/*.test.{ts,tsx}`. Many are boundary
  contract tests verifying Tauri command shapes stay stable.
- Backend: Rust `#[cfg(test)]` modules in most source files. Tests that touch
  the filesystem use `unique_temp_dir()` for isolation—clean up after yourself.
- Add regression tests before behavior fixes.

## Styles

- Plain CSS or CSS Modules only. No emoji icons—use SVG.
- Status must not rely on color alone: include text labels and counts.
- Hover/focus must not move layout or content.

## v1 Boundaries

### Forbidden

Electron, embedded Python, arbitrary shell execution, `gita` CLI dependency,
arbitrary command panels.

### Git Operations

- Fixed argument arrays only. v1 manages `origin` remote URL only.
- Fetch: allowed when `origin` exists.
- Pull: always requires explicit confirmation (modifies working tree).
- Push: requires confirmation; available only for `local_ahead` or `diverged`.
- Remote URL button disabled when no browser-safe origin URL exists.

### Status Model

| Relation | Color | Meaning |
|----------|-------|---------|
| `synced` | green | Local and upstream match |
| `local_ahead` | yellow | Local has commits to push |
| `remote_ahead` | yellow | Upstream has commits to pull |
| `diverged` | red | Both sides have unique commits |
| `no_remote` | gray | No supported origin comparison |

`error` is an app read-failure state, not a sixth relation.
`no_remote` must remain last in compact summaries, filters, and expanded lists.

## Native Desktop Widgets

**Status:** Planning complete, implementation pending

**Detailed plan:** See `NATIVE_WIDGET_IMPLEMENTATION_PLAN.md`

### macOS WidgetKit Widget

- **Feasibility:** ✅ Feasible with Tauri + Xcode hybrid build
- **Approach:** Swift/SwiftUI Widget Extension embedded in Tauri app bundle
- **Data sharing:** Shared JSON file at `~/Library/Application Support/GitaView/widget-data.json`
- **Deep linking:** `gitaview://` URL scheme to open app from widget
- **Minimum system:** macOS 11.0 (Big Sur)
- **Build:** `xcodebuild` via `beforeBundleCommand` in `tauri.conf.json`
- **Key files:**
  - `src-tauri/widget-extension/` - Widget Extension project (new)
  - `src-tauri/src/widget_data.rs` - Data writer module (new)
  - `src-tauri/tauri.conf.json` - Bundle config and deep link setup

### Windows Widget

- **Status:** Keep current implementation (Progman/WorkerW reparenting)
- **Reason:** Windows 11 Widgets Board has no public third-party extension API
- **Current implementation:** `src-tauri/src/desktop_widget/windows.rs`
- **Note:** This is already the best approach for third-party desktop widgets on Windows
