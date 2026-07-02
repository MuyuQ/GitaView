# GitaView PROJECT KNOWLEDGE BASE

**Updated:** 2026-06-23
**Branch:** main

## OVERVIEW

GitaView is a cross-platform desktop Git repository status widget built with
Tauri 2, Rust, React, TypeScript, and Vite.

The app manages its own repository list, groups, settings, tray menu, and desktop
widget window. It does not depend on `gita` CLI configuration.

## STRUCTURE

```text
GitaView/
├── AGENTS.md
├── CODE_REVIEW_REPORT.md
├── DESIGN_AND_BUILD_SPEC.md
├── PRODUCT.md
├── README.md
├── docs/
│   ├── native-desktop-widget-layer-plan.md
│   ├── platform-acceptance-checklist.md
│   ├── RELEASE_SIGNING.md
│   └── superpowers/
├── src/
│   ├── components/
│   ├── lib/
│   │   └── useWidgetView.ts
│   ├── styles/
│   ├── App.tsx
│   ├── main.tsx
│   └── types.ts
├── src-tauri/
│   ├── capabilities/
│   ├── icons/
│   ├── src/
│   │   ├── app_commands.rs
│   │   ├── app_settings.rs
│   │   ├── desktop_widget/
│   │   ├── diagnostics.rs
│   │   ├── domain/
│   │   ├── git/
│   │   ├── lib.rs
│   │   ├── main.rs
│   │   ├── repo_operation.rs
│   │   ├── repo_registry.rs
│   │   ├── repo_status.rs
│   │   ├── storage/
│   │   ├── system_open.rs
│   │   ├── tray_menu_rows.rs
│   │   └── tray_status.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
└── AGENTS.md
```

## WHERE TO LOOK

| Task | Location | Notes |
|------|----------|-------|
| Product requirements | `DESIGN_AND_BUILD_SPEC.md` | v1 product and technical contract |
| Widget view hook | `src/lib/useWidgetView.ts` | Widget modes, refresh lifecycle, native sizing, settings routing |
| Root React flow | `src/App.tsx` | View switching between collapsed, expanded, and settings |
| Status model | `src/lib/statusModel.ts` | Summary, sorting, filters, action availability |
| Settings UI | `src/components/settings/` | Repository, group, refresh, safety, appearance |
| Tauri command boundary | `src-tauri/src/app_commands.rs` | Fixed app commands exposed to React |
| Git execution | `src-tauri/src/git/commands.rs` | Fixed-argument Git subprocess execution |
| Native desktop layer | `src-tauri/src/desktop_widget/` | Windows and macOS widget attachment |
| Persistence | `src-tauri/src/storage/store.rs` | Normalized atomic settings writes |
| Tray menu | `src-tauri/src/tray_status.rs` | Async tray refresh and generation guard |

## CONVENTIONS

### Tests

- Frontend: Vitest, colocated as `src/**/*.test.{ts,tsx}`.
- Backend: Rust inline `#[cfg(test)]` modules.
- Add regression tests before behavior fixes.

### Styles

- Use plain CSS or CSS Modules.
- Do not use emoji as icons. Use consistent SVG icons.
- Status must not be communicated by color alone. Include text labels and counts,
  including screen-reader labels in compact views.
- Hover and focus states must not move layout or content.

## V1 BOUNDARIES

### Forbidden

- Electron.
- Embedded Python runtime.
- Arbitrary shell command execution.
- Dependency on `gita` CLI configuration.
- Arbitrary command panels.

### Git Operations

- Git subprocesses use fixed argument arrays only.
- v1 manages the `origin` remote URL only. Other remote topologies are out of
  scope.
- Fetch is allowed when `origin` exists.
- Pull always requires explicit confirmation because it can modify the working
  tree.
- Push always requires explicit confirmation and is available only for
  `local_ahead` or `diverged` repositories.
- The remote URL button is disabled when no browser-safe origin URL exists.

### Status Model

| Relation | Color | Meaning |
|----------|-------|---------|
| `synced` | green | Local and upstream match |
| `local_ahead` | yellow | Local has commits to push |
| `remote_ahead` | yellow | Upstream has commits to pull |
| `diverged` | red | Both sides have unique commits |
| `no_remote` | gray | No supported origin comparison exists |

`error` is an application read-failure state, not a sixth Git relation.

`no_remote` must remain last in compact summaries, filters, and expanded lists.

## COMMANDS

```bash
# Install dependencies
npm ci

# Development mode
npm run tauri dev

# Frontend
npm test
npm run build

# Rust
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings

# Windows debug Tauri build
npm run tauri -- build --debug --no-bundle
```

## FINAL ACCEPTANCE

Run frontend tests, Rust tests, Rust formatting, Clippy, frontend build, and a
Tauri build before release. macOS native-layer compilation must also pass in the
macOS release matrix.
