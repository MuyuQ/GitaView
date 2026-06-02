# Collapsed Summary Clarity Design

## Goal

Make the collapsed widget understandable without requiring users to decode single-character abbreviations. Keep the capsule compact and action-oriented while preserving a stable, quickly scannable structure on Windows and macOS.

## Approved Summary

The collapsed widget displays:

```text
GitaView 仓库14 ·待同步4 ·需关注0 ·无远端2
```

The approved design intentionally omits the synced count from the collapsed capsule. Users primarily need to know whether repositories require a sync action, need attention, or have no remote configured. The expanded view remains the detailed source of truth for every repository state.

## Display Rules

The collapsed capsule renders exactly three visible status groups after the total repository count:

| Order | Visible label | Included relations |
| --- | --- | --- |
| 1 | `待同步` | `local_ahead`, `remote_ahead` |
| 2 | `需关注` | `diverged`, `error` |
| 3 | `无远端` | `no_remote` |

Each visible status group includes its existing colored dot, complete label, and numeric count. Text and numbers remain visible because color alone must never carry status meaning.

All three groups remain visible when their count is zero. A stable layout lets users confirm `需关注0` explicitly instead of guessing whether the category was omitted.

`无远端` remains last. This preserves the project-wide ordering rule.

## Component Boundaries

`summarizeCollapsed` continues to calculate all four buckets, including `synced`. The underlying model remains complete and reusable.

`WidgetCollapsed` selects the three action-oriented buckets for visible rendering. It replaces the single-character labels with the approved complete labels and renders `仓库` before the total count.

`TransitionSurface` mirrors the same visible summary as `WidgetCollapsed`. The transition capsule must not briefly regress to unlabeled dots or show a different set of buckets during open and close animations.

The collapsed native window width and CSS capsule width increase from `218px` to `312px`. This applies to the initial Tauri window configuration, the runtime collapsed window size, the stable collapsed capsule, and the transition capsule. The approved width leaves a small safety margin around the measured content for the configured Chinese fallback fonts on both Windows and macOS.

## Accessibility

The visible labels are also the accessible labels. The implementation removes the split between screen-reader-only complete labels and visible single-character labels.

The collapsed button keeps its existing interaction model:

- Click expands the widget.
- Drag moves the widget when dragging is enabled.
- Right click opens the native context menu.
- Hover and focus states do not shift layout.

## Testing

Tests must verify:

- The collapsed widget renders only `待同步`, `需关注`, and `无远端`.
- The synced bucket remains present in the summary model but is omitted from the collapsed rendering.
- Zero-count action buckets remain visible.
- The transition capsule uses the same visible labels and bucket selection as the stable collapsed widget.
- Single-character summary labels do not return.
- The Tauri initial width, runtime collapsed window width, stable CSS capsule width, and transition CSS capsule width all stay aligned at `312px`.
- The existing frontend test suite and production build pass.

## Scope

This change affects only collapsed presentation, its transition surface, and the matching native window size. It does not alter repository scanning, expanded view sorting, filtering, Git actions, settings persistence, release packaging, or Rust backend behavior.
