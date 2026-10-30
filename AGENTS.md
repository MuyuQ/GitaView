# GitaView

Native desktop Git status application. Windows uses C# / WinUI 3 with a Win32
tray and Windows Widget Provider. Rust implements the shared core and C ABI.
The macOS SwiftUI / AppKit host and WidgetKit extension are planned; only the
shared Rust core currently runs in macOS CI.

This checkout contains only the native application. Do not restore a web UI,
Tauri client, WorkerW / Progman embedding, or an old release pipeline. Earlier
planning and review documents contain historical paths; use README.md and
docs/native-architecture.md for current entry points.

## Commands

```powershell
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
powershell -NoProfile -File scripts/native/test-windows.ps1
powershell -NoProfile -File scripts/native/build-windows.ps1 -Configuration Release
powershell -NoProfile -File scripts/native/package-windows.ps1 -Configuration Release -SkipBuild
python -m pip install -r scripts/native/requirements-test.txt
python scripts/native/test-contract-schema.py
```

Windows build/package scripts support x64. Package signing uses
GITAVIEW_SIGN_THUMBPRINT and the -Sign switch. -Install removes the existing
package and its LocalState, installs the signed package and restarts widget
hosts; use it only when installation is part of the user's request.

The only app CI is .github/workflows/native-ci.yml. Windows runs Rust format,
Clippy, tests, layout checks, C# tests, build and unsigned MSIX packaging.
macOS runs Rust format, Clippy, tests and checks both Apple targets. No Linux
targets. There is no automatic tag-triggered release workflow.

## Architecture

| Layer | Location | Contract |
| --- | --- | --- |
| Core / engine | crates/gitaview-core/src/engine.rs | Lifecycle, admission, versioned snapshots, configuration reprojection |
| Git execution | crates/gitaview-core/src/git/ | Fixed arguments, deadlines, Windows Job Object / macOS process group |
| Settings / storage | crates/gitaview-core/src/settings.rs, src/storage/ | Revision checks, atomic writes, corrupt-data isolation |
| FFI | crates/gitaview-ffi/, include/gitaview.h | Owned buffers, owner leases, explicit shutdown |
| Native contracts | contracts/native/v1/ | JSON Schema, fixtures and cross-language expectations |
| Windows host | apps/windows/GitaView.Native/ | Main window, settings, tray and flyout |
| C# interop | apps/windows/GitaView.Native.Interop/ | Native library binding and handle lifetime |
| DTO / cache | apps/windows/GitaView.Native.Contracts/ | Shared JSON validation and atomic widget cache |
| Widget Provider | apps/windows/GitaView.Native.WidgetProvider/ | Serialized updates, COM lifetime, no scanning engine |
| Windows tests | apps/windows/GitaView.Native.Tests/ | Real FFI plus lifecycle, UI presentation and widget contracts |
| Brand assets | assets/brand/, scripts/native/ | Shared SVG and optional Node/Sharp generation tool |

The root Cargo workspace has gitaview-core and gitaview-ffi as members and one
Cargo.lock. There is no root npm project. The package in scripts/native is
optional asset tooling; ordinary builds use committed PNG/ICO/SVG files.

## Tests and evidence

- Add regression tests before behavior fixes. Filesystem tests use distinct
  temporary directories, including a counter when a timestamp may collide.
- Await owned background work before deleting its files. A global log flush
  does not stop later producers; tests must not delete a live global sink.
- Run checks appropriate to the touched layer. Do not report compilation,
  mocked UI, or unsigned packaging as signed installation or GUI acceptance.
- Keep platform limitations and pending device checks visible in
  docs/native-platform-acceptance.md. Shared-core macOS CI is not a macOS app.

## Product and safety boundaries

- No Electron, embedded Python, arbitrary shell panels, gita CLI dependency,
  old-version data migration, or web-rendered application UI.
- Use fixed Git argument arrays and the origin remote only. Automatic scans
  never Fetch. Fetch requires origin but does not require a first commit.
- Pull always requires explicit confirmation. Push requires confirmation and
  is offered only for local_ahead / diverged. The core revalidates operations.
- Disable remote opening when there is no browser-safe origin URL.
- Status relations are synced, local_ahead, remote_ahead, diverged and
  no_remote. Read failures are errors, not a sixth Git relation. Pending rows
  have no invented Git fields. Keep no_remote last in summaries and filters.
- Both platforms share product layout and semantic styles. Respect native
  font rendering, window chrome, pickers, tray/menu bar and widget hosting.
- Use SVG icons, text labels and counts; do not rely on color alone. Hover
  and focus must not move content.
- One main-app engine owns scanning. Explicit exit stops it; widgets retain
  the last valid cache and never start an independent scanner.
