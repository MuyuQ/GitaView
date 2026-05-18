# Backend Maintainability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Continue the GitaView backend cleanup by separating pure tray menu row formatting from Tauri runtime mutation, and by adding source-level contracts that keep `app_commands.rs` as orchestration only.

**Architecture:** `tray_status.rs` should own Tauri menu construction, tray lookup, refresh generation, and async refresh. A new `tray_menu_rows.rs` should own pure row data, menu action ids, status labels, and row unit tests. A frontend source contract test should protect `app_commands.rs` from regressing into mixed platform/system/policy code.

**Tech Stack:** Rust/Tauri 2 backend, Vitest source contract tests, Cargo unit tests, Clippy.

---

## File Structure

- Create `src-tauri/src/tray_menu_rows.rs`
  - Owns `TrayMenuRow`, `TrayMenuRowKind`, `TRAY_REFRESH_ID`, `TRAY_SHOW_ID`, `TRAY_QUIT_ID`.
  - Owns `loading_tray_rows`, `tray_status_rows`, `error_tray_rows`.
  - Owns row-format unit tests currently living in `tray_status.rs`.
- Modify `src-tauri/src/tray_status.rs`
  - Keeps `MAIN_TRAY_ID`, `TRAY_MENU_GENERATION`, Tauri `Menu` building, `set_*_menu`, and `refresh_tray_menu_async`.
  - Imports row data from `tray_menu_rows`.
  - Re-exports action ids so existing `lib.rs` event matching can stay unchanged.
- Modify `src-tauri/src/lib.rs`
  - Registers `pub mod tray_menu_rows;`.
- Create `src/lib/appCommandsBoundaryContract.test.ts`
  - Uses `sourceContract` helpers.
  - Asserts `app_commands.rs` does not contain platform/system-open code, Git operation policy enums, or repository registry helper functions.
- Modify `src/lib/trayStatusMenuContract.test.ts`
  - Points row-content assertions at `tray_menu_rows.rs` after the split.

---

### Task 1: Split Pure Tray Row Formatting From Tauri Runtime

**Files:**
- Create: `src-tauri/src/tray_menu_rows.rs`
- Modify: `src-tauri/src/tray_status.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/trayStatusMenuContract.test.ts`

- [ ] **Step 1: Write the failing source contract for the split**

Update `src/lib/trayStatusMenuContract.test.ts` to expect the new module and to read row labels from it:

```ts
it("keeps pure tray row formatting separate from Tauri runtime menu updates", () => {
  const lib = readProjectFileCompact("src-tauri/src/lib.rs");
  const trayStatus = readProjectFileCompact("src-tauri/src/tray_status.rs");
  const trayRows = readProjectFile("src-tauri/src/tray_menu_rows.rs");

  expect(lib).toContain("pub mod tray_menu_rows;");
  expect(trayStatus).toContain("crate::tray_menu_rows");
  expect(trayStatus).toContain("pub use crate::tray_menu_rows::{");
  expect(trayRows).toContain("正在读取仓库状态...");
  expect(trayRows).toContain("GitaView · 共");
});
```

- [ ] **Step 2: Run the contract test and verify it fails**

Run:

```bash
npm test -- src/lib/trayStatusMenuContract.test.ts
```

Expected: FAIL because `src-tauri/src/tray_menu_rows.rs` does not exist yet.

- [ ] **Step 3: Create `tray_menu_rows.rs` with pure row types and moved tests**

Move the following items from `tray_status.rs` into `tray_menu_rows.rs`:

```rust
pub const TRAY_REFRESH_ID: &str = "refresh-status";
pub const TRAY_SHOW_ID: &str = "show";
pub const TRAY_QUIT_ID: &str = "quit";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayMenuRowKind {
    Display,
    Separator,
    Action { id: &'static str },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayMenuRow {
    pub kind: TrayMenuRowKind,
    pub text: String,
}
```

Also move `loading_tray_rows`, `tray_status_rows`, `error_tray_rows`, `append_action_rows`, `status_row_label`, `format_status_with_change`, and the row-focused tests:

```rust
status_rows_show_summary_and_hide_synced_repositories
status_rows_include_every_unsynced_relation_and_keep_no_remote_last
loading_rows_use_display_item_before_actions
empty_status_rows_still_include_summary_and_actions
```

- [ ] **Step 4: Update runtime module imports**

In `src-tauri/src/tray_status.rs`, import and re-export row items:

```rust
use crate::tray_menu_rows::{
    error_tray_rows, loading_tray_rows, tray_status_rows, TrayMenuRow, TrayMenuRowKind,
};

pub use crate::tray_menu_rows::{TRAY_QUIT_ID, TRAY_REFRESH_ID, TRAY_SHOW_ID};
```

Keep only `tray_generation_marks_older_refreshes_as_stale` in `tray_status.rs` tests.

- [ ] **Step 5: Register the new Rust module**

In `src-tauri/src/lib.rs`, add:

```rust
pub mod tray_menu_rows;
```

- [ ] **Step 6: Verify the split**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml tray_menu_rows
cargo test --manifest-path src-tauri/Cargo.toml tray_status
npm test -- src/lib/trayStatusMenuContract.test.ts
```

Expected: all pass.

---

### Task 2: Add Source Boundary Contract For `app_commands.rs`

**Files:**
- Create: `src/lib/appCommandsBoundaryContract.test.ts`

- [ ] **Step 1: Write the boundary contract test**

Create `src/lib/appCommandsBoundaryContract.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { readProjectFile, readProjectFileCompact } from "./sourceContract";

describe("app command boundary contract", () => {
  it("keeps platform opening and repository policy out of app_commands", () => {
    const commands = readProjectFile("src-tauri/src/app_commands.rs");
    const compactCommands = readProjectFileCompact("src-tauri/src/app_commands.rs");

    expect(commands).not.toContain("target_os");
    expect(commands).not.toContain("Command::new");
    expect(commands).not.toContain("enum RepoGitOperation");
    expect(commands).not.toContain("fn validate_repo_git_operation");
    expect(commands).not.toContain("fn repo_id_from_path");
    expect(commands).not.toContain("fn find_repo");
    expect(compactCommands).toContain("use crate::repo_operation::{validate_repo_git_operation, RepoGitOperation};");
    expect(compactCommands).toContain("use crate::repo_registry::{find_repo, repo_id_from_path};");
    expect(compactCommands).toContain("use crate::system_open::{open_directory, open_http_url};");
  });
});
```

- [ ] **Step 2: Run the boundary contract**

Run:

```bash
npm test -- src/lib/appCommandsBoundaryContract.test.ts
```

Expected: PASS because the previous cleanup already moved those responsibilities out.

---

### Task 3: Full Verification

**Files:**
- No new production files.

- [ ] **Step 1: Run Rust tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: all Rust tests pass.

- [ ] **Step 2: Run Rust lint**

Run:

```bash
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Expected: no warnings.

- [ ] **Step 3: Run frontend tests**

Run:

```bash
npm test
```

Expected: all Vitest tests pass.

- [ ] **Step 4: Run frontend build**

Run:

```bash
npm run build
```

Expected: TypeScript and Vite build complete successfully.

---

## Self-Review

- Spec coverage: The plan covers the two requested maintainability goals for this iteration: tray row/runtime separation and command boundary protection.
- Placeholder scan: No placeholder steps remain; commands and expected outcomes are explicit.
- Type consistency: Rust item names match existing code (`TrayMenuRow`, `TrayMenuRowKind`, `RepoStatusDto`, `TRAY_REFRESH_ID`) and source contract helper names match `src/lib/sourceContract.ts`.
