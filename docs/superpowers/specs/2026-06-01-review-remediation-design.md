# Review Remediation Design

## Goal

Resolve the release-blocking, reliability, accessibility, release-governance, and
repository-hygiene findings from the 2026-06-01 full project review without
changing GitaView's v1 product shape.

## Product Decisions

- Push remains a v1 action because `AGENTS.md` is the repository-local source of
  truth. It is visible only for `local_ahead` and `diverged` repositories.
- Pull and Push always require explicit confirmation. Stored legacy safety flags
  remain readable for compatibility but normalize to `true` and are no longer
  editable.
- GitaView supports the `origin` remote in v1. Other remote topologies are out of
  scope and are documented as such.

## Architecture

### Desktop Layer And Safety

The Tauri setup path reapplies the native desktop widget layer after the main
window is available. Failure remains non-fatal and is logged so Explorer or
platform-specific issues degrade to a normal window instead of aborting startup.

Mutating repository actions enforce confirmation in the Rust command boundary.
The React action panel always presents the second-click confirmation step. UI
preferences cannot weaken this invariant.

### Settings

All settings cards listen for settings update events before saving full settings
documents. This keeps their snapshots current while preserving the existing
backend command API. Legacy safety settings normalize to enabled and the safety
card becomes an informational invariant display. The sidebar footer is labeled
`关闭` because card-level buttons persist changes immediately.

Settings persistence writes a sibling temporary file and atomically replaces the
destination. The implementation removes any stale temporary file before writing.

### Refresh And Filtering

Refresh completion is guarded by a monotonically increasing request generation.
Only the latest request can update UI state or clear the refreshing indicator.
Initial-loading layout decisions wait until the initial request finishes, and
initial errors use the expanded native frame so recovery controls stay visible.

Changing a group reconciles the selected relation. If the selected relation has
no repositories in the new group, the filter resets to `all`.

### Git Execution And Diagnostics

Git stdout and stderr are drained concurrently while the timeout loop monitors
the child process. This prevents a full OS pipe from stalling the command.

Diagnostics rotate the active log when it reaches a fixed size. Logged repository
paths are reduced to a redacted marker or basename where the event needs context.

### Release, Accessibility, And Hygiene

The package, Cargo, and Tauri versions align at `0.2.2`. The release workflow runs
frontend tests, Rust tests, formatting checks, and Clippy before packaging.

Collapsed summaries expose status names to assistive technology. Settings inputs
receive associated labels. Undefined style variables, hover movement, and the
stale Vite favicon are removed. Generated caches and the malformed brainstorm
directory stop being tracked. Documentation is updated to match the implemented
v1 product.

## Verification

- Add focused Vitest and Rust regression tests before production edits.
- Run `npm test`.
- Run `cargo test --manifest-path src-tauri/Cargo.toml`.
- Run `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`.
- Run `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
- Run `npm run build`.
- Run `npm run tauri -- build --debug --no-bundle`.
- Run browser preview interaction checks for settings close behavior and filter
  reconciliation.
- Rely on release CI for macOS native compilation because the local host is
  Windows.
