# Cross-Platform Hardening Design

## Scope

Harden the Windows and macOS desktop-widget implementations before the next release. Linux remains an explicit unsupported fallback for v1. Existing GitHub tags and draft releases are not mutated automatically.

## Windows Desktop Frame Ownership

The Windows widget is reparented into the Explorer desktop host. Once this happens, Win32 expects frame coordinates relative to the parent client area rather than the screen. Frontend code must stop calling Tauri `setPosition` and `setSize` directly. Instead, it invokes a Rust command with an optional screen position and a physical size.

The Windows implementation converts a provided screen position with `ScreenToClient`, then calls `SetWindowPos` with the converted child coordinates. It also applies `WS_CHILD`, `WS_VISIBLE`, `WS_EX_TOOLWINDOW`, and removal of `WS_POPUP` and `WS_EX_APPWINDOW` before calling `SetParent`.

## Explorer Recovery

Windows receives a lightweight desktop-widget watchdog. Every five seconds it checks whether the main window still has a valid parent. If the desktop host was rebuilt, it reapplies the widget layer. Tray-driven window display also requests a reapply so recovery is prompt when the user opens the app.

The watchdog is intentionally Windows-only. macOS and unsupported platforms expose no-op implementations to keep setup platform-neutral.

## macOS Menu-Bar Behavior

On macOS, setup switches the Tauri application activation policy to `Accessory`, removing normal Dock and Cmd+Tab behavior. The tray icon uses a transparent monochrome template asset and allows menu opening on left click. Windows continues using the existing colored tray icon and right-click menu behavior.

## Hidden Windows Git Processes

Every Windows Git child process receives `CREATE_NO_WINDOW`, not just timeout cleanup commands. This prevents periodic status refreshes from flashing console windows in the packaged GUI app.

## CI And Release Trust

A new pull-request CI workflow checks frontend tests, Rust tests, formatting, native Clippy, and explicit target compilation for Windows, macOS Intel, and macOS Apple Silicon.

The release workflow performs explicit target compilation and rejects releases when required signing secrets are absent. Windows imports a base64 `.pfx` certificate into the runner certificate store, patches the Tauri thumbprint and timestamp URL for the build, and verifies Authenticode signatures after packaging. macOS receives Developer ID and notarization credentials through Tauri-supported environment variables.

Documentation records the required secrets and the manual cleanup commands for stale draft releases. No workflow deletes tags or releases automatically.

## Testing

- Source-contract tests lock platform-specific setup, native frame synchronization, watchdog startup, hidden Git process creation, PR CI, and release signing guards.
- Rust tests cover Windows frame-flag selection where the logic is platform-independent enough to execute locally.
- Existing frontend and Rust suites remain required.
- Windows debug Tauri packaging remains a local verification step.
- macOS packaging and behavior are verified by GitHub macOS runners and final real-device smoke tests.
