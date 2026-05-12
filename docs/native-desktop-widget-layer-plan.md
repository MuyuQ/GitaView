# Native Desktop Widget Layer Plan

**Audience:** GLM / implementation agent  
**Goal:** Make GitaView a real desktop-layer widget on Windows and macOS, not a normal always-on-top window.

## 1. Summary

GitaView currently behaves like a transparent, borderless Tauri top-level window with `skipTaskbar: true`. That is not a real desktop widget. On Windows, clicking "Show desktop" can hide it because the OS treats it as a normal app window. On macOS, it is also still a normal app window unless assigned to a desktop-level `NSWindow` layer.

Implement platform-native desktop-layer behavior:

- Windows: attach the GitaView HWND to the desktop icon host window, above desktop icons but below normal app windows.
- macOS: set the `NSWindow` level just above desktop icons and configure Space/Mission Control behavior.
- Remove the existing "置顶 / always on top" product feature because the new behavior is explicitly desktop-layer, not floating-window.
- Preserve existing widget UI, collapsed/expanded state, right-click menu, tray, drag, refresh, settings, and repo actions.

Definition of success:

- Clicking Windows "Show desktop" keeps GitaView visible and clickable.
- On macOS, showing desktop / switching Spaces keeps GitaView in desktop-widget semantics.
- Normal app windows can cover GitaView.
- GitaView remains above desktop icons when it overlaps them.
- No "置顶" setting or menu item remains.

## 2. Architecture

Create a small platform abstraction in Rust:

```text
src-tauri/src/desktop_widget/
  mod.rs
  windows.rs
  macos.rs
  unsupported.rs
```

Public internal API:

```rust
pub fn apply_desktop_widget_layer(window: &tauri::WebviewWindow) -> Result<(), String>;
pub fn reapply_desktop_widget_layer(app: &tauri::AppHandle) -> Result<(), String>;
```

Behavior:

- `apply_desktop_widget_layer` applies the native desktop-layer behavior for one window.
- `reapply_desktop_widget_layer` finds the `"main"` window and reapplies the layer.
- Calls must be idempotent.
- On unsupported platforms, return `Ok(())` after logging a clear unsupported message, leaving the normal window behavior unchanged.
- Do not expose these as frontend commands unless needed for temporary debug builds.

Call sites:

- In `src-tauri/src/lib.rs` setup, after the `"main"` window exists and settings are loaded, call `desktop_widget::reapply_desktop_widget_layer(app.handle())`.
- Do not call `set_always_on_top` anymore.
- If applying the desktop layer fails, log the error and keep the app running as a normal window.

## 3. Windows Implementation

Use Tauri's existing native handle:

```rust
let hwnd = window.hwnd()?;
```

Implementation target:

- Place GitaView as a child of the desktop icon host window, not as a normal app top-level window.
- Keep it above desktop icons by setting it to top among the desktop host's children.
- Keep it below normal app windows by not using always-on-top.

Win32 approach:

1. Find `Progman`:

```text
FindWindowW("Progman", null)
```

2. Ask Explorer to create/refresh WorkerW windows:

```text
SendMessageTimeoutW(Progman, 0x052C, ...)
```

3. Locate the desktop icon host:

- Enumerate top-level windows with `EnumWindows`.
- Find the window whose child tree contains `SHELLDLL_DefView`.
- Prefer the actual parent of `SHELLDLL_DefView` as the desktop host.
- Fallback to `Progman` if no WorkerW/SHELLDLL_DefView host is found.

4. Reparent GitaView:

```text
SetParent(gitaview_hwnd, desktop_host_hwnd)
```

5. Adjust styles:

- Add child/visible style required for parented desktop behavior.
- Remove normal popup/top-level style if it conflicts after `SetParent`.
- Keep tool-window/no-taskbar behavior.
- Preserve enough original style data internally if later restoration is needed for debug.

6. Set z-order within the desktop host:

```text
SetWindowPos(gitaview_hwnd, HWND_TOP, ..., SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW)
```

7. Reapply after Explorer restarts:

- At minimum, reapply on app startup and after window creation.
- Preferred: add a lightweight Windows message/event hook or periodic low-frequency retry only if manual testing shows Explorer restart breaks parentage.
- Do not add a busy polling loop.

Required Windows crate dependency:

```toml
[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.61", features = [
  "Win32_Foundation",
  "Win32_UI_WindowsAndMessaging"
] }
```

Manual Windows validation:

- Start GitaView.
- Place widget over desktop icon area.
- Confirm widget is visible and clickable above icons.
- Open a normal app window over it; confirm app covers GitaView.
- Click taskbar "Show desktop"; confirm GitaView remains visible/clickable.
- Restart Explorer; confirm GitaView recovers after app restart, and preferably without app restart if reapply hook is implemented.

## 4. macOS Implementation

Use Tauri's existing native handle:

```rust
let ns_window = window.ns_window()?;
```

Implementation target:

- GitaView should live near the desktop icon layer.
- It should be above desktop icons so it remains clickable.
- It should stay below normal app windows.
- It should not appear as a normal app-window target in window cycling.

Cocoa behavior:

1. Cast `ns_window` to `NSWindow`.

2. Set window level to desktop icon level + 1:

```text
CGWindowLevelForKey(kCGDesktopIconWindowLevelKey) + 1
```

3. Set collection behavior:

```text
NSWindowCollectionBehaviorCanJoinAllSpaces
NSWindowCollectionBehaviorStationary
NSWindowCollectionBehaviorIgnoresCycle
```

4. Preserve existing window transparency, borderless style, size changes, and frontend interactions.

5. Do not call always-on-top APIs on macOS.

Required macOS dependencies should be added directly, matching compatible versions already pulled through Tauri where possible:

```toml
[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.6"
objc2-app-kit = "0.3"
objc2-core-graphics = "0.3"
```

If exact versions conflict, use the versions already resolved in `Cargo.lock`.

Manual macOS validation:

- Start GitaView on the desktop.
- Confirm normal app windows cover it.
- Confirm it remains visible when showing desktop.
- Confirm Mission Control and Space switching do not treat it like a normal app window.
- Confirm it remains clickable when placed over desktop icons.
- Confirm collapsed/expanded transitions still work.

## 5. Remove Always-On-Top Product Feature

Remove "置顶" entirely from product behavior.

Frontend changes:

- Remove `alwaysOnTop` from `src/types.ts`.
- Remove preview/mock `alwaysOnTop` from `src/lib/commands.ts`.
- Remove `toggleAlwaysOnTop()` wrapper.
- Remove `alwaysOnTop` state and `handleToggleAlwaysOnTop` from `src/App.tsx`.
- Update `WidgetCollapsed` props so it no longer receives `alwaysOnTop` or `onToggleAlwaysOnTop`.
- Update `collapsedContextMenu.ts` to only include:
  - `刷新`
  - `退出`
- Update `collapsedContextMenu.test.ts` expected menu items accordingly.
- Remove "窗口始终置顶" from `AppearanceSettings`.
- Update any appearance/settings tests to only cover compact mode and drag mode.

Backend changes:

- Remove `always_on_top` from `AppearanceSettings`.
- Keep serde compatibility with old settings files:
  - Old `alwaysOnTop` / `always_on_top` data should be ignored, not fail loading.
- Remove startup call to `window.set_always_on_top(...)`.
- Remove `toggle_always_on_top` command and command registration.
- Remove dynamic always-on-top update from `save_settings`.

Important:

- Do not replace "置顶" with a new user-facing mode switch.
- The app's only mode after this change is real desktop-widget layer on Windows/macOS.

## 6. Dragging and Positioning

Preserve current drag behavior if it still works after desktop-layer parenting.

If Windows `startDragging()` fails after `SetParent`, implement a fallback:

- Frontend tracks pointer delta while dragging.
- Use existing Tauri window position APIs to set the new position.
- Keep this fallback only for desktop-widget mode.
- Do not change visual drag affordance.

Existing edge-anchored expanded positioning must remain intact:

- The recent `resolveAnchoredWindowPosition` logic should continue to run before resizing.
- Desktop-layer parenting must not break collapsed-to-expanded positioning.

## 7. Tests

Rust tests:

- Settings load should tolerate older settings that contain `alwaysOnTop`.
- `AppearanceSettings::default()` should not include always-on-top.
- Platform-independent desktop widget API should return `Ok(())` on unsupported platforms.
- Windows/macOS native calls may be thin wrappers and mostly manually validated, but isolate any pure host-selection logic that can be tested.

Frontend tests:

- `collapsedContextMenu.test.ts` expects only `刷新` and `退出`.
- Appearance settings tests no longer reference "窗口始终置顶".
- Commands/types tests no longer reference `toggleAlwaysOnTop` or `alwaysOnTop`.
- Existing widget transition, window motion, repo table, settings tests continue to pass.

Required verification commands:

```powershell
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri -- build --debug --no-bundle
```

Platform manual verification is mandatory:

- Windows behavior must be tested on Windows.
- macOS behavior must be tested on macOS.
- Linux can remain unsupported with normal window behavior.

## 8. Implementation Order

1. Remove always-on-top UI/state/commands/tests.
2. Add `desktop_widget` module with unsupported no-op implementation.
3. Wire `reapply_desktop_widget_layer` into Tauri setup.
4. Implement macOS layer behavior.
5. Implement Windows layer behavior.
6. Re-run all tests/builds.
7. Perform manual Windows validation.
8. Perform manual macOS validation.
9. Fix drag fallback only if native drag fails after desktop parenting.

## 9. Non-Goals

- Do not implement Linux desktop-layer support.
- Do not add a new "desktop mode vs floating mode" setting.
- Do not make GitaView always-on-top.
- Do not redesign the frontend UI.
- Do not replace Tauri with Electron or another shell.
- Do not remove tray support.

## 10. Known Risks

- Windows WorkerW/Progman behavior is not a formal high-level app API; it may need reapply logic after Explorer restarts.
- Windows child-window parenting can affect drag behavior; validate early.
- macOS Spaces, Mission Control, Stage Manager, and fullscreen Spaces may behave differently across OS versions.
- Native layer code must be heavily `#[cfg]`-guarded to avoid breaking builds on the other platform.

## 11. Acceptance Criteria

Implementation is accepted only when:

- Windows "Show desktop" no longer hides GitaView.
- macOS desktop semantics work in normal desktop and Space switching scenarios.
- GitaView remains above desktop icons and clickable.
- Normal app windows can cover GitaView.
- The "置顶" setting and right-click menu item are gone.
- `npm test`, `npm run build`, and `cargo test --manifest-path src-tauri/Cargo.toml` pass.
- Windows and macOS manual validation results are recorded in the final implementation report.
