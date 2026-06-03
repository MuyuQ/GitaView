# Platform Acceptance Checklist

Use this checklist before publishing or promoting a release. Automated CI proves
build and contract coverage, but desktop-widget behavior depends on native window
managers and must be verified on real target platforms.

Current unsigned baseline: `v0.3.1-unsigned`.

## Required Platforms

| Platform | Artifact | Tester | Result | Date |
|------|------|------|------|------|
| Windows | `.exe` or `.msi` from the release draft | _TBD_ | _TBD_ | _TBD_ |
| macOS Apple Silicon | Apple Silicon `.dmg` | _TBD_ | _TBD_ | _TBD_ |
| macOS Intel | Intel `.dmg` | _TBD_ | _TBD_ | _TBD_ |

## Windows

- Install the `v0.3.1-unsigned` Windows artifact and confirm any SmartScreen
  warning is expected for unsigned test builds.
- Launch GitaView and confirm it starts as a compact desktop widget, not a
  taskbar window.
- Confirm the tray icon opens the app menu and the show action reapplies the
  desktop widget layer.
- Confirm the compact widget expands, collapses, opens settings, and refreshes.
- Confirm dragging the widget preserves its position across collapsed, expanded,
  and settings views.
- Press Win+D or use Show desktop and confirm the widget remains visible and
  clickable.
- Restart Explorer or simulate Explorer restart, then confirm the watchdog
  reattaches the widget to the current desktop host.
- Confirm repository Fetch/Pull/Push actions never flash a console window.
- Confirm quitting from the tray exits the app and removes the tray icon.

## macOS Apple Silicon

- Install the Apple Silicon `v0.3.1-unsigned` DMG and confirm any Gatekeeper
  warning is expected for unsigned test builds.
- Launch GitaView and confirm it runs as an accessory-style app: menu-bar icon
  present, Dock entry absent, Cmd+Tab entry absent.
- Confirm the menu-bar icon uses the template asset and remains visible in light
  and dark menu bars.
- Confirm left click opens the menu on macOS.
- Confirm the compact widget expands, collapses, opens settings, and refreshes.
- Confirm dragging the widget works without moving controls on hover/focus.
- Use Show Desktop, Mission Control, and Space switching; confirm the widget
  keeps desktop-widget semantics and does not behave like a normal app window.
- Confirm repository Fetch/Pull/Push confirmations and disabled states match the
  repository status.
- Quit from the menu and confirm no menu-bar icon remains.

## macOS Intel

- Repeat the macOS Apple Silicon checklist with the Intel DMG on an Intel Mac.
- Confirm the Intel artifact launches natively and does not rely on Rosetta for
  the tested package.
- Record any differences in menu-bar, Mission Control, Space switching, and
  widget drag behavior.

## Result Notes

When a platform passes, replace `_TBD_` in the table with the tester, date, and
`PASS`. For failures, record `FAIL` plus the exact OS version, artifact filename,
steps to reproduce, and whether the issue blocks release promotion.
