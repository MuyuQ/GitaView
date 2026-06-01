# Fourth-Layer Review Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the five new multi-platform review findings and publish the verified changes to the existing GitHub pull request.

**Architecture:** Add small pure boundaries for origin-only Git arguments and expanded-widget reconciliation, restore the search UI through the existing filter pipeline, make Windows attachment rollback explicit, and reuse the existing settings event bus for repository mutations.

**Tech Stack:** Tauri 2, Rust, Win32 APIs through `windows`, React/TypeScript, Vitest, Cargo, GitHub CLI.

---

### Task 1: Origin-Only Git Actions

**Files:**
- Modify: `src-tauri/src/git/commands.rs`
- Modify: `src-tauri/src/app_commands.rs`

- [ ] Add failing Rust tests asserting:

```rust
assert_eq!(origin_fetch_args(), vec!["fetch", "origin"]);
assert_eq!(origin_pull_args("main"), vec!["pull", "origin", "main"]);
assert_eq!(
    origin_push_args("main"),
    vec!["push", "origin", "HEAD:refs/heads/main"],
);
```

- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml origin_action_args_pin_remote_and_branch -- --nocapture` and confirm failure because the builders are absent.
- [ ] Add the three argument builders and pass their output to `run_git` from Fetch, Pull, and Push.
- [ ] Re-run the focused Rust test and confirm it passes.

### Task 2: Expanded Search And Refresh Reconciliation

**Files:**
- Modify: `src/lib/statusModel.test.ts`
- Modify: `src/lib/statusModel.ts`
- Modify: `src/components/WidgetExpanded.tsx`
- Modify: `src/styles/widget.css`

- [ ] Add failing Vitest cases proving query matching over name, group, branch, and path, plus reconciliation of removed groups and unavailable relations.
- [ ] Run `npm test -- src/lib/statusModel.test.ts` and confirm the new cases fail.
- [ ] Extend `filterRepos` with an optional query and add `reconcileExpandedFilters`.
- [ ] Restore the toolbar search input, pass the deferred query to `filterRepos`, and reconcile filters and row selection in a `useEffect`.
- [ ] Add search-control styles without changing the trailing action group layout.
- [ ] Re-run `npm test -- src/lib/statusModel.test.ts src/lib/widgetToolbarLayout.test.ts` and confirm it passes.

### Task 3: Repository Settings Broadcast

**Files:**
- Modify: `src/lib/reviewRemediationContract.test.ts`
- Modify: `src/components/settings/RepositorySettings.tsx`

- [ ] Add a failing source-contract assertion that add and remove flows broadcast the settings snapshot returned by reload.
- [ ] Run `npm test -- src/lib/reviewRemediationContract.test.ts` and confirm failure.
- [ ] Return the snapshot from `reload()` and invoke `notifySettingsUpdated(nextSettings)` after add and remove reloads.
- [ ] Re-run the focused contract test and confirm it passes.

### Task 4: Windows Attachment Rollback

**Files:**
- Modify: `src/lib/crossPlatformHardeningContract.test.ts`
- Modify: `src-tauri/src/desktop_widget/windows.rs`

- [ ] Add a failing source-contract assertion for an attachment snapshot, rollback helper, and `SWP_FRAMECHANGED`.
- [ ] Run `npm test -- src/lib/crossPlatformHardeningContract.test.ts` and confirm failure.
- [ ] Capture original parent and styles before mutation, rollback after `SetParent` or `SetWindowPos` errors, and apply frame-change notification after style writes.
- [ ] Re-run the focused contract test and Rust tests.

### Task 5: Full Verification And GitHub Sync

**Files:**
- Verify all modified files.

- [ ] Run `npm test`.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml`.
- [ ] Run `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`.
- [ ] Run `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
- [ ] Run `npm run build`.
- [ ] Run `npm run tauri -- build --debug --no-bundle`.
- [ ] Run `git diff --check`.
- [ ] Inspect `git status --short` and `git diff --stat`.
- [ ] Commit the intended files and push `codex/cross-platform-hardening`.
- [ ] Confirm the existing draft PR points at the pushed commit.

