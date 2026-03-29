---
managed-file: crates/lajjzy-tui/src/widgets/diff_view.rs
intent: >
  Renders a scrollable unified-diff view as a ratatui widget. Given a slice of
  DiffHunk values and a scroll offset, it flattens all hunk headers and diff
  lines into a single ordered sequence, applies per-kind prefixes ("+"/"-"/"
  "/none) and color styles (green for added, red for removed, default for
  context, bold dark-gray for headers), and writes exactly area.height rows
  starting at the scroll offset into the ratatui Buffer. When the hunk slice is
  empty it renders a single "(empty diff)" placeholder in dark gray instead.
intent-approved: false
intent-hash: 55a86266cf70
distilled-from:
  - path: crates/lajjzy-tui/src/widgets/diff_view.rs
    hash: b0db9b4eeed2
non-goals:
  - Does not manage scroll state — caller owns and passes the scroll offset
  - Does not handle user input or keyboard navigation
  - Does not perform syntax highlighting beyond the four DiffLineKind color rules
depends-on:
  - crates/lajjzy-core/src/types.spec.md
---

## Purpose

`DiffViewWidget` is a ratatui `Widget` that renders a unified-diff view into a
buffer region. Callers supply a slice of `DiffHunk` values and a `scroll` offset;
the widget maps that data to colored, prefixed lines visible within the allotted
area. It is stateless — all data flows in through the constructor.

## Behavior

1. **Empty guard** — if `hunks` is empty, renders `"(empty diff)"` in
   `Color::DarkGray` at row 0 and returns immediately; remaining rows are
   untouched.

2. **Line flattening** — builds an ordered flat list by iterating hunks in order:
   for each hunk, pushes `(DiffLineKind::Header, hunk.header)` first, then each
   `DiffLine` as `(dl.kind, dl.content)`.

3. **Viewport slicing** — renders rows `scroll .. scroll + area.height`, stopping
   early if the flat list is exhausted. Rows beyond the flat list are left
   untouched in the buffer.

4. **Prefix insertion** — prepends a single character to each line's text:
   - `DiffLineKind::Added` → `"+"`
   - `DiffLineKind::Removed` → `"-"`
   - `DiffLineKind::Context` → `" "`
   - `DiffLineKind::Header` → no prefix (raw header text)

5. **Color styling** — applies a `Style` to each rendered line:
   - Added → `Color::Green`
   - Removed → `Color::Red`
   - Context → default style (no color)
   - Header → `Color::DarkGray` + `Modifier::BOLD`

6. **Buffer writes** — each visible line is written via `buf.set_line(area.x, y,
   &line, area.width)`, where `y = area.y + row`. Lines are clipped to
   `area.width` by ratatui; no manual truncation is performed.

## Constraints

- `scroll` values pointing past the end of the flat list result in an empty
  render (no rows written, no panic).
- Row index `row` is cast from `usize` to `u16` via `row as u16`; correctness
  relies on `area.height` fitting in `u16`, which is guaranteed by ratatui's
  `Rect` type. The cast is annotated with `#[expect(clippy::cast_possible_truncation)]`.
- All data is borrowed — the widget holds `&'a [DiffHunk]`. No allocations
  occur beyond format strings for each rendered line.
- Colors and styles are hardcoded per `DiffLineKind`; they are not configurable
  by the caller.

## Dependencies

- `ratatui::{Buffer, Rect, Color, Modifier, Style, Line, Widget}`
- `lajjzy_core::types::{DiffHunk, DiffLineKind}`
