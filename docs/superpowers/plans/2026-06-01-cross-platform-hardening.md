# Cross-Platform Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Harden native Windows and macOS widget behavior and prevent unsigned or weakly validated multi-platform releases.

**Architecture:** Route frontend frame synchronization through a Rust platform boundary so Windows can convert screen coordinates to desktop-host child coordinates. Add Windows host recovery, macOS accessory/menu-bar setup, hidden Windows Git children, target-aware PR CI, and release signing preflight scripts.

**Tech Stack:** Tauri 2, Rust, Win32 APIs through `windows`, React/TypeScript, Vitest source contracts, GitHub Actions, Node.js release validation, PowerShell certificate import.

---

### Task 1: Cross-Platform Source Contracts

**Files:**
- Create: `src/lib/crossPlatformHardeningContract.test.ts`

- [ ] Write failing Vitest source-contract assertions for the Rust frame command, `ScreenToClient`, style-before-parent ordering, Windows watchdog, macOS `Accessory`, tray template behavior, hidden Git child flags, PR CI targets, and release signing validation.
- [ ] Run `npm test -- src/lib/crossPlatformHardeningContract.test.ts` and confirm the assertions fail because the hardening code is absent.

### Task 2: Windows Frame Synchronization And Recovery

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/desktop_widget/mod.rs`
- Modify: `src-tauri/src/desktop_widget/windows.rs`
- Modify: `src-tauri/src/desktop_widget/macos.rs`
- Modify: `src-tauri/src/desktop_widget/unsupported.rs`
- Modify: `src-tauri/src/app_commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/commands.ts`
- Modify: `src/App.tsx`

- [ ] Add a Tauri command accepting optional screen `x` and `y` plus physical `width` and `height`.
- [ ] Convert screen coordinates with Win32 `ScreenToClient` when the Windows window is a desktop-host child.
- [ ] Apply child styles before `SetParent`.
- [ ] Start a Windows-only five-second watchdog and reapply the layer when displaying the main window from the tray.
- [ ] Replace frontend `setPosition` and `setSize` calls with the native synchronization command.
- [ ] Run the focused contract test and Rust tests.

### Task 3: macOS Menu-Bar Setup And Tray Asset

**Files:**
- Create: `src-tauri/icons/tray-template.svg`
- Create: `src-tauri/icons/tray-template.png`
- Modify: `src-tauri/src/lib.rs`

- [ ] Add a transparent monochrome tray glyph and generate the committed PNG from the SVG.
- [ ] Switch macOS setup to `tauri::ActivationPolicy::Accessory`.
- [ ] Use the template icon and left-click menu behavior only on macOS.
- [ ] Keep the existing colored icon and right-click behavior on Windows.
- [ ] Run the focused contract test.

### Task 4: Hidden Windows Git Children

**Files:**
- Modify: `src-tauri/src/git/commands.rs`

- [ ] Add a Windows-only helper that applies `CREATE_NO_WINDOW` to every Git command before spawn.
- [ ] Keep timeout cleanup behavior unchanged.
- [ ] Run the focused contract test and Rust tests.

### Task 5: Pull-Request CI And Signed Release Guard

**Files:**
- Create: `.github/workflows/ci.yml`
- Create: `scripts/validate-release-signing.cjs`
- Create: `scripts/configure-windows-signing.ps1`
- Modify: `.github/workflows/release.yml`
- Create: `docs/RELEASE_SIGNING.md`

- [ ] Add pull-request and main-push multi-platform CI with explicit target checks.
- [ ] Add release preflight validation for required Windows and macOS secrets.
- [ ] Import the Windows `.pfx`, patch Tauri signing configuration for that runner, and verify generated installer signatures.
- [ ] Pass macOS signing and notarization secrets to `tauri-action`.
- [ ] Document secrets, local verification, stale draft-release cleanup commands, and required real-device smoke checks.
- [ ] Run the focused contract test and the release validators with passing and intentionally failing environments.

### Task 6: Full Verification And GitHub Sync

**Files:**
- Verify all modified files.

- [ ] Run `npm test`.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml`.
- [ ] Run `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`.
- [ ] Run `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
- [ ] Run `npm run build`.
- [ ] Run `npm run tauri -- build --debug --no-bundle`.
- [ ] Run `npm audit --audit-level=low`.
- [ ] Run `git diff --check`.
- [ ] Commit, push `codex/cross-platform-hardening`, and open a draft PR against `main`.
