---
managed-file: crates/lajjzy-tui/src/widgets/graph.rs
intent: >
  Render the jj change graph with per-node coloring (yellow change ID, blue author, cyan timestamp, magenta bookmarks), REVERSED highlight for the cursor block (change node + its connector lines), DarkGray for connector-only lines and excluded nodes in picking mode, bold+green glyph for the working-copy node, PR status indicators per bookmark, and a ⚠N conflict indicator. Scroll offset is computed to keep the cursor block plus scrolloff padding visible.
intent-approved: false
intent-hash: 9d1b86b368eb
distilled-from:
  - path: crates/lajjzy-tui/src/widgets/graph.rs
    hash: 7f9ce34375dc
non-goals:
  - Does not handle user input — display only
  - Does not own GraphData or PrInfo state — all data passed via constructor
  - Does not parse or transform jj graph output — assumes GraphData is pre-parsed
depends-on:
  - crates/lajjzy-tui/src/app.spec.md
  - crates/lajjzy-core/src/types.spec.md
---

## Purpose

`GraphWidget` is a ratatui `Widget` that renders the full jj change graph into
the main panel. It applies rich per-node coloring, highlights the cursor block,
dims excluded nodes during picking mode, and annotates bookmarks with PR status.

## Behavior

- **Cursor block:** All lines from `cursor` up to (but not including) the next
  change node are highlighted with `REVERSED` modifier applied cell-by-cell after
  `set_line`.
- **Scroll offset:** Computed via `scroll_offset(height)` to keep the cursor
  block within `scrolloff = 3` rows of the visible top/bottom. The offset is
  clamped so the last page fills the area.
- **Node lines** (lines with `change_id`): Built as multi-span `Line` via
  `colored_node_line`:
  - Glyph prefix: `DarkGray`, or `Green + BOLD` for the working-copy node.
  - Change ID: `Yellow`.
  - Author: `Blue` (if non-empty).
  - Timestamp: `Cyan` (if non-empty).
  - Bookmarks: `Magenta` in `[name, ...]` brackets (if non-empty).
  - PR indicators: appended after bookmarks as `" #N symbol"` — symbol/color
    by state: Merged/Closed → `✗`/`✓` DarkGray; Open+Approved → `✓` Green;
    Open+ChangesRequested → `✗` Red; Open+ReviewRequired/Unknown → `●` Yellow.
  - Conflict count: `" ⚠N"` in `Yellow` when `conflict_count > 0`.
- **Connector lines** (no `change_id`): Rendered as plain `DarkGray` spans.
- **Picking-mode dimming:** When `target_pick` is set and a node's `change_id`
  is in `pick.excluded`, the entire line is rendered as a single `DarkGray`
  span; the REVERSED highlight is suppressed for that row.
- **Empty graph:** Returns immediately, renders nothing.
- **Missing detail:** `debug_assert!(false, ...)` fires; falls back to the raw
  tail after the glyph prefix.

## Constraints

- Zero-height guard: implicit — loop over `0..height` produces no iterations.
- `scrolloff` is hardcoded to 3; not configurable via constructor.
- Highlight is applied cell-by-cell (`buf[(x,y)].set_style(style)`) after
  `set_line`, so the REVERSED modifier overlays individual span colors.
- `#[expect(clippy::cast_possible_truncation)]` on the row→y cast.
- 8 tests in `#[cfg(test)]`.

## Dependencies

- `ratatui::{Buffer, Rect, Color, Modifier, Style, Line, Span, Widget}`
- `lajjzy_core::forge::{PrInfo, PrState, ReviewStatus}`
- `lajjzy_core::types::GraphData`
- `crate::app::TargetPick`
- `std::collections::HashMap`
