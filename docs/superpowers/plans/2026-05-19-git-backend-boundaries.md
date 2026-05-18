# Git Backend Boundaries Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce `src-tauri/src/git/commands.rs` responsibility by moving remote URL normalization and Git status text formatting into focused modules with direct tests and source boundary contracts.

**Architecture:** Keep `git/commands.rs` focused on process execution and branch state collection. Add `git/remote.rs` for remote URL normalization and `git/status_text.rs` for UI-facing labels/hints derived from `GitBranchState`. Update command and status consumers to import the focused modules directly.

**Tech Stack:** Rust/Tauri 2 backend, Vitest source contract tests, Cargo unit tests, Clippy.

---

## File Structure

- Create `src-tauri/src/git/remote.rs`
  - Owns `normalize_remote_url`.
  - Owns tests for GitHub SSH, HTTP/HTTPS, unsupported URL schemes, and empty values.
- Create `src-tauri/src/git/status_text.rs`
  - Owns `change_label`, `relation_hint`, `state_hint`.
  - Owns tests for status labels and `no_remote` hints.
- Modify `src-tauri/src/git/commands.rs`
  - Keeps `GitBranchState`, `run_git`, `format_git_failure`, `branch_state`, and Git command tests.
  - Imports `normalize_remote_url` from `git::remote`.
  - Removes status text functions and their tests.
- Modify `src-tauri/src/repo_status.rs`
  - Imports `change_label` and `state_hint` from `git::status_text`.
- Modify `src-tauri/src/app_commands.rs`
  - Imports `normalize_remote_url` from `git::remote`.
- Modify `src-tauri/src/lib.rs`
  - Registers `git::remote` and `git::status_text`.
- Create `src/lib/gitBackendBoundaryContract.test.ts`
  - Guards the new module boundaries.

---

### Task 1: Extract Remote URL Normalization

**Files:**
- Create: `src-tauri/src/git/remote.rs`
- Modify: `src-tauri/src/git/commands.rs`
- Modify: `src-tauri/src/app_commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src/lib/gitBackendBoundaryContract.test.ts`

- [x] **Step 1: Write failing source contract for Git remote boundary**

Create `src/lib/gitBackendBoundaryContract.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { readProjectFile, readProjectFileCompact } from "./sourceContract";

describe("git backend boundary contract", () => {
  it("keeps remote URL normalization outside git commands", () => {
    const lib = readProjectFileCompact("src-tauri/src/lib.rs");
    const commands = readProjectFile("src-tauri/src/git/commands.rs");
    const remote = readProjectFile("src-tauri/src/git/remote.rs");
    const appCommands = readProjectFileCompact("src-tauri/src/app_commands.rs");

    expect(lib).toContain("pub mod remote;");
    expect(commands).not.toContain("pub fn normalize_remote_url");
    expect(commands).toContain("use crate::git::remote::normalize_remote_url;");
    expect(remote).toContain("pub fn normalize_remote_url");
    expect(appCommands).toContain("use crate::git::remote::normalize_remote_url;");
  });
});
```

- [x] **Step 2: Verify contract fails before implementation**

Run:

```bash
npm test -- src/lib/gitBackendBoundaryContract.test.ts
```

Expected: FAIL because `src-tauri/src/git/remote.rs` does not exist yet.

- [x] **Step 3: Create remote module and move normalization tests**

Create `src-tauri/src/git/remote.rs`:

```rust
pub fn normalize_remote_url(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }
    if let Some(rest) = value.strip_prefix("git@github.com:") {
        return Some(format!(
            "https://github.com/{}",
            rest.trim_end_matches(".git")
        ));
    }
    if value.starts_with("https://") || value.starts_with("http://") {
        return Some(value.trim_end_matches(".git").to_string());
    }
    None
}
```

Move these tests from `commands.rs` into `remote.rs`:

```rust
normalizes_github_ssh_url
keeps_non_github_urls_openable
rejects_unsupported_remote_urls_for_opening
```

Add one direct empty-value test:

```rust
#[test]
fn rejects_empty_remote_urls() {
    assert_eq!(normalize_remote_url("   "), None);
}
```

- [x] **Step 4: Update imports and module registration**

In `src-tauri/src/lib.rs`:

```rust
pub mod git {
    pub mod commands;
    pub mod discovery;
    pub mod remote;
}
```

In `src-tauri/src/git/commands.rs`, remove the local `normalize_remote_url` function and add:

```rust
use crate::git::remote::normalize_remote_url;
```

In `src-tauri/src/app_commands.rs`, replace:

```rust
let remote = crate::git::commands::normalize_remote_url(&remote)
```

with:

```rust
let remote = normalize_remote_url(&remote)
```

and add:

```rust
use crate::git::remote::normalize_remote_url;
```

- [x] **Step 5: Verify remote extraction**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml remote
npm test -- src/lib/gitBackendBoundaryContract.test.ts
```

Expected: all pass.

---

### Task 2: Extract Git Status Text Formatting

**Files:**
- Create: `src-tauri/src/git/status_text.rs`
- Modify: `src-tauri/src/git/commands.rs`
- Modify: `src-tauri/src/repo_status.rs`
- Modify: `src/lib/gitBackendBoundaryContract.test.ts`

- [x] **Step 1: Extend source contract for status text boundary**

Append to `src/lib/gitBackendBoundaryContract.test.ts`:

```ts
it("keeps status text formatting outside git commands", () => {
  const lib = readProjectFileCompact("src-tauri/src/lib.rs");
  const commands = readProjectFile("src-tauri/src/git/commands.rs");
  const statusText = readProjectFile("src-tauri/src/git/status_text.rs");
  const repoStatus = readProjectFileCompact("src-tauri/src/repo_status.rs");

  expect(lib).toContain("pub mod status_text;");
  expect(commands).not.toContain("pub fn change_label");
  expect(commands).not.toContain("pub fn relation_hint");
  expect(commands).not.toContain("pub fn state_hint");
  expect(statusText).toContain("pub fn change_label");
  expect(statusText).toContain("pub fn state_hint");
  expect(repoStatus).toContain("use crate::git::status_text::{change_label, state_hint};");
});
```

- [x] **Step 2: Verify contract fails before implementation**

Run:

```bash
npm test -- src/lib/gitBackendBoundaryContract.test.ts
```

Expected: FAIL because `src-tauri/src/git/status_text.rs` does not exist yet.

- [x] **Step 3: Create status text module and move tests**

Create `src-tauri/src/git/status_text.rs` with the moved functions:

```rust
use crate::domain::status::RemoteRelation;
use crate::git::commands::GitBranchState;

pub fn change_label(state: &GitBranchState) -> String {
    match state.relation {
        RemoteRelation::Error => "!".to_string(),
        RemoteRelation::Synced => "✓".to_string(),
        RemoteRelation::LocalAhead => format!("↑ {}", state.ahead),
        RemoteRelation::RemoteAhead => format!("↓ {}", state.behind),
        RemoteRelation::Diverged => format!("⇕ {}", state.ahead + state.behind),
        RemoteRelation::NoRemote => "∅".to_string(),
    }
}

pub fn relation_hint(relation: RemoteRelation) -> &'static str {
    match relation {
        RemoteRelation::Error => "读取失败",
        RemoteRelation::Synced => "无需操作",
        RemoteRelation::LocalAhead => "可 Push",
        RemoteRelation::RemoteAhead => "可 Pull",
        RemoteRelation::Diverged => "需要人工处理",
        RemoteRelation::NoRemote => "未配置远端",
    }
}

pub fn state_hint(state: &GitBranchState) -> String {
    if state.relation == RemoteRelation::NoRemote {
        if state.has_remote {
            "未设置可比较的 upstream".to_string()
        } else {
            "未配置远端".to_string()
        }
    } else {
        relation_hint(state.relation).to_string()
    }
}
```

Move `formats_change_labels_for_all_relations` from `commands.rs` into `status_text.rs`.

Add this no-remote hint test:

```rust
#[test]
fn state_hint_distinguishes_missing_remote_from_missing_upstream() {
    let mut state = sample_state(RemoteRelation::NoRemote);
    state.has_remote = false;
    assert_eq!(state_hint(&state), "未配置远端");

    state.has_remote = true;
    assert_eq!(state_hint(&state), "未设置可比较的 upstream");
}
```

- [x] **Step 4: Update imports**

In `src-tauri/src/git/commands.rs`, remove `change_label`, `relation_hint`, and `state_hint`.

In `src-tauri/src/lib.rs`, add:

```rust
pub mod git {
    pub mod commands;
    pub mod discovery;
    pub mod remote;
    pub mod status_text;
}
```

In `src-tauri/src/repo_status.rs`, change imports from:

```rust
use crate::git::commands::{branch_state, change_label, state_hint, GitBranchState};
```

to:

```rust
use crate::git::commands::{branch_state, GitBranchState};
use crate::git::status_text::{change_label, state_hint};
```

- [x] **Step 5: Verify status text extraction**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml status_text
cargo test --manifest-path src-tauri/Cargo.toml commands
npm test -- src/lib/gitBackendBoundaryContract.test.ts
```

Expected: all pass.

---

### Task 3: Full Verification

**Files:**
- No new production files.

- [x] **Step 1: Run Rust tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: all Rust tests pass.

- [x] **Step 2: Run Rust lint**

Run:

```bash
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Expected: no warnings.

- [x] **Step 3: Run frontend tests**

Run:

```bash
npm test
```

Expected: all Vitest tests pass.

- [x] **Step 4: Run frontend build**

Run:

```bash
npm run build
```

Expected: TypeScript and Vite build complete successfully.

---

## Self-Review

- Spec coverage: The plan covers the chosen Git backend boundary cleanup by splitting remote URL normalization and status text formatting out of `git/commands.rs`.
- Placeholder scan: No placeholder comments or unspecified code remain in implementation snippets.
- Type consistency: Module names and imports match current project paths and existing public function names.

## Execution Results

- `npm test -- src/lib/gitBackendBoundaryContract.test.ts`: failed before each extraction as expected, then passed after implementation.
- `cargo fmt --manifest-path src-tauri/Cargo.toml`: completed after each Rust edit set.
- `cargo test --manifest-path src-tauri/Cargo.toml remote`: passed after remote extraction.
- `cargo test --manifest-path src-tauri/Cargo.toml status_text`: passed after status text extraction.
- `cargo test --manifest-path src-tauri/Cargo.toml commands`: passed after command module cleanup.
- `cargo test --manifest-path src-tauri/Cargo.toml`: 41 passed, 0 failed.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`: completed with no warnings.
- `npm test`: 19 files passed, 76 tests passed.
- `npm run build`: TypeScript and Vite production build completed.
