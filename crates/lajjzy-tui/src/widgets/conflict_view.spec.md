---
managed-file: crates/lajjzy-tui/src/widgets/conflict_view.rs
intent: >
  Render a scrollable conflict resolution viewer showing jj conflict regions as base/left/right panels separated by styled ─── separator lines, with color-coded sides (DarkGray base, Blue left, Green right), dimming of the rejected side when a resolution is chosen, bold separators for the current hunk, a resolution status footer per conflict block, and collapsed '··· N lines ···' summaries for resolved regions.
intent-approved: false
intent-hash: 696dc404fb98
distilled-from:
  - path: crates/lajjzy-tui/src/widgets/conflict_view.rs
    hash: d41b887d070f
non-goals:
  - Does not handle user input or key events — display only
  - Does not write resolved content to disk — resolution state is in-memory only
  - Does not support side-by-side layout — always stacked vertically
depends-on:
  - crates/lajjzy-tui/src/app.spec.md
  - crates/lajjzy-core/src/types.spec.md
---

## Purpose

`ConflictViewWidget` is a ratatui `Widget` that renders conflict data from a
`ConflictView` as a vertically-scrollable flat list of styled lines. Each
conflict block shows base, left (ours), and right (theirs) sections; resolved
non-conflict regions are collapsed to a single summary line.

## Behavior

The widget flattens `ConflictView.data.regions` into a `Vec<RenderLine>`:

- **`ConflictRegion::Resolved(text)`** → `ResolvedCollapsed { line_count }`:
  renders as `"··· N lines ···"` in `DarkGray`.
- **`ConflictRegion::Conflict { base, left, right }`** → sequence of:
  1. **Base separator** + content lines: always `DarkGray` (dimmed).
  2. **Left separator** (`"─── left (ours) ─── [1] ───"`) + content lines:
     `Blue` normally; `DarkGray` when `resolution == AcceptRight`.
  3. **Right separator** (`"─── right (theirs) ─── [2] ───"`) + content lines:
     `Green` normally; `DarkGray` when `resolution == AcceptLeft`.
  4. **Resolution status** (`"─── resolved: {label} ───"`): `Yellow`; all
     three separators/status lines gain `BOLD` when `hunk_idx == view.cursor`.
- **Separator rows** have their full row background filled with the separator
  style (via cell-by-cell `set_style`) before the text is written.
- **Content lines** are indented with a leading space (`" {text}"`).
- **Empty content side** renders as `"  (file deleted)"` in italic.
- **Empty regions list** renders `"(no conflict regions)"` in `DarkGray` on the
  first row and returns.
- **Zero area** (width or height == 0): returns immediately.
- **Scrolling** uses `view.scroll` as the skip offset into the flat render list.

## Constraints

- `resolutions` length must match conflict count; an out-of-bounds access panics
  with `expect("resolutions length must match conflict count...")`.
- `#[expect(clippy::cast_possible_truncation)]` on the row→y cast.
- 9 tests in `#[cfg(test)]`.

## Dependencies

- `ratatui::{Buffer, Rect, Color, Modifier, Style, Line, Widget}`
- `lajjzy_core::types::ConflictRegion`
- `crate::app::{ConflictView, HunkResolution}`
