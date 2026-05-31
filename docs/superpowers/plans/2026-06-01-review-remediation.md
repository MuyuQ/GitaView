# Review Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve every actionable finding from the 2026-06-01 full project review and verify a release-ready Windows build plus CI coverage for macOS.

**Architecture:** Preserve the existing Tauri command boundary and React component structure. Add small testable helpers for confirmation, refresh generation, filter reconciliation, atomic persistence, bounded diagnostics, and concurrent Git output draining, then wire those helpers into existing call sites.

**Tech Stack:** Tauri 2, Rust, React 19, TypeScript, Vite, Vitest, GitHub Actions.

---

### Task 1: Restore desktop widget setup and enforce confirmation

**Files:**
- Modify: `src/lib/traySetupContract.test.ts`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/app_commands.rs`
- Modify: `src/components/RepoActions.tsx`
- Modify: `src/components/settings/SafetySettings.tsx`

- [ ] Replace the stale startup contract with one that requires
  `reapply_desktop_widget_layer`.
- [ ] Add Rust tests for unconditional Pull and Push confirmation rejection.
- [ ] Run focused tests and verify they fail for the missing behavior.
- [ ] Restore desktop-layer startup wiring and make confirmation unconditional.
- [ ] Remove UI switches that claim confirmation can be disabled.
- [ ] Run focused tests and verify they pass.

### Task 2: Keep settings snapshots current and persist atomically

**Files:**
- Modify: `src/components/settings/RefreshSettings.tsx`
- Modify: `src/components/settings/AppearanceSettings.tsx`
- Modify: `src/components/settings/SettingsShell.tsx`
- Modify: `src-tauri/src/domain/settings.rs`
- Modify: `src-tauri/src/storage/store.rs`

- [ ] Add contracts that require settings subscriptions and truthful close text.
- [ ] Add Rust tests that prove normalized safety flags and atomic temp cleanup.
- [ ] Run focused tests and verify red failures.
- [ ] Subscribe mounted cards to settings events, normalize safety invariants,
  relabel close, and replace direct writes with atomic persistence.
- [ ] Run focused tests and verify green results.

### Task 3: Reconcile filters and serialize refresh completion

**Files:**
- Create: `src/lib/refreshGeneration.ts`
- Create: `src/lib/refreshGeneration.test.ts`
- Modify: `src/lib/statusModel.ts`
- Modify: `src/lib/statusModel.test.ts`
- Modify: `src/components/WidgetExpanded.tsx`
- Modify: `src/App.tsx`

- [ ] Add tests for stale refresh generations and invalid relation filters.
- [ ] Run focused tests and verify red failures.
- [ ] Add generation and relation reconciliation helpers.
- [ ] Wire helpers into `App` and `WidgetExpanded`.
- [ ] Prevent initial sizing decisions until loading completes and keep recovery
  UI in an expanded frame.
- [ ] Run focused tests and verify green results.

### Task 4: Harden Git execution and diagnostics

**Files:**
- Modify: `src-tauri/src/git/commands.rs`
- Modify: `src-tauri/src/diagnostics.rs`
- Modify: `src-tauri/src/app_settings.rs`
- Modify: `src-tauri/src/repo_status.rs`
- Modify: `src-tauri/src/app_commands.rs`

- [ ] Add Rust tests for high-volume command output, rotation, and redaction.
- [ ] Run focused Rust tests and verify red failures.
- [ ] Drain stdout and stderr concurrently during command execution.
- [ ] Add bounded log rotation and redact filesystem paths from diagnostics.
- [ ] Run focused Rust tests and verify green results.

### Task 5: Align release metadata, accessibility, styles, and docs

**Files:**
- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `.github/workflows/release.yml`
- Modify: `src/components/WidgetCollapsed.tsx`
- Modify: `src/components/settings/RefreshSettings.tsx`
- Modify: `src/components/settings/RepositorySettings.tsx`
- Modify: `src/components/settings/GroupSettings.tsx`
- Modify: `src/styles/widget.css`
- Modify: `index.html`
- Modify: `README.md`
- Modify: `DESIGN_AND_BUILD_SPEC.md`
- Modify: `AGENTS.md`

- [ ] Add source contracts for aligned versions, release quality gates,
  accessibility labels, stable hover styling, and a valid favicon.
- [ ] Run focused frontend tests and verify red failures.
- [ ] Apply metadata, workflow, accessibility, style, and documentation changes.
- [ ] Run focused frontend tests and verify green results.

### Task 6: Remove tracked generated artifacts and complete verification

**Files:**
- Remove: tracked `__pycache__/*.pyc`
- Remove: malformed `.superpowers/brainstorm/manual-$(Get-Date -Format yyyyMMddHHmmss)/`

- [ ] Remove generated tracked artifacts without deleting intentional prototypes
  or the vendored UI/UX skill source.
- [ ] Run `git diff --check`.
- [ ] Run `npm test`.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml`.
- [ ] Run `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`.
- [ ] Run `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
- [ ] Run `npm run build`.
- [ ] Run `npm run tauri -- build --debug --no-bundle`.
- [ ] Run browser interaction checks and inspect `git status`.
