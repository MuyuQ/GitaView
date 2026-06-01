# Fourth-Layer Review Remediation Design

## Goal

Resolve the five new findings from the fourth multi-platform review without
changing GitaView's v1 product shape.

## Decisions

- Repository network actions remain origin-only in v1. Fetch, Pull, and Push
  must name `origin` explicitly. Pull and Push must also name the current branch
  so repository-local defaults cannot redirect a confirmed operation.
- Windows desktop attachment remains best-effort. If attachment fails after
  mutating the native window, the implementation restores the original parent
  and styles before returning the error so startup can actually fall back to a
  normal window.
- Expanded-widget filters remain local UI state, but repository refreshes
  reconcile disappearing groups, unavailable relations, and stale row
  selections.
- Expanded search is restored as a lightweight client-side filter over
  repository name, path, branch, and group. The toolbar keeps its current
  trailing action group.
- Settings cards continue to synchronize through the existing
  `gitaview:settings-updated` event. Repository add and remove flows broadcast
  the settings document returned by reload so mounted group statistics update.

## Architecture

### Git Boundary

`src-tauri/src/git/commands.rs` owns argument builders for origin-only actions.
`src-tauri/src/app_commands.rs` uses those builders after the existing state and
confirmation checks. This gives tests a small pure boundary while ensuring
production execution cannot silently fall back to Git configuration defaults.

### Windows Attachment

`src-tauri/src/desktop_widget/windows.rs` captures the original parent, style,
and extended style before attachment. Failures after mutation invoke a rollback
helper. Style changes notify Win32 with `SWP_FRAMECHANGED`.

### Expanded Widget

`src/lib/statusModel.ts` restores query filtering and adds a pure reconciliation
helper. `src/components/WidgetExpanded.tsx` applies that helper when refreshed
repository data changes, clears removed row selections, and renders the search
control.

### Settings Synchronization

`RepositorySettings.reload()` returns the loaded settings snapshot. Add and
remove flows broadcast that snapshot after reload, reusing the existing event
contract instead of adding component coupling.

## Verification

- Add focused Vitest and Rust regression tests before production edits.
- Run focused tests after each implementation slice.
- Run `npm test`.
- Run `cargo test --manifest-path src-tauri/Cargo.toml`.
- Run `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`.
- Run `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
- Run `npm run build`.
- Run `npm run tauri -- build --debug --no-bundle`.
- Run `git diff --check`.

