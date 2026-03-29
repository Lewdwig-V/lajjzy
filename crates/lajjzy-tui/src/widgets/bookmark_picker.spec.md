---
managed-file: crates/lajjzy-tui/src/widgets/bookmark_picker.rs
intent: >
  Renders a scrollable, bordered list of bookmarks (name + change description) inside a blue-bordered "Bookmarks" panel. Each row shows the bookmark name in magenta alongside the associated change description in dark gray. The cursor row is highlighted with reversed video. When the list is empty, displays a "(no bookmarks)" placeholder. Auto-scrolls the viewport so the cursor row is always visible.
intent-approved: false
intent-hash: 79d956fa56a0
distilled-from:
  - path: crates/lajjzy-tui/src/widgets/bookmark_picker.rs
    hash: e8254ca1e593
non-goals:
  - Does not own or mutate bookmark data; it is a pure rendering widget
  - Does not handle keyboard/mouse input or emit actions; navigation state is provided by the caller
  - Does not fetch or resolve change descriptions from the backend; descriptions are pre-loaded by the caller
depends-on:
  - crates/lajjzy-core/src/types.spec.md
---

## Purpose

`BookmarkPickerWidget` is a stateless ratatui `Widget` that renders a scrollable list of repository bookmarks. Callers provide a slice of `(name, change_id)` pairs, a map from `change_id` to `ChangeDetail`, and a cursor index. The widget paints itself into a `Buffer` area on every render call.

## Behavior

1. **Border and title:** Always draws a full (all-sides) border in blue with the title "Bookmarks".
2. **Empty state:** When the bookmarks slice is empty, renders the text `(no bookmarks)` in `DarkGray` on the first inner row and returns immediately.
3. **Row layout:** Each row contains three spans in order: the bookmark name (magenta), two spaces, and the change description (dark gray). The description is looked up from the `descriptions` map by `change_id`; if the `change_id` is absent from the map the description is the empty string.
4. **Cursor highlight:** The row whose index equals `cursor` receives a full-width reversed-video style applied cell-by-cell across the inner width.
5. **Viewport / auto-scroll:** The widget computes a `scroll` offset so that the cursor row is always the last visible row when it would otherwise fall off the bottom. Specifically: if `cursor >= inner_height`, `scroll = cursor - inner_height + 1`; otherwise `scroll = 0`. Rows from index `scroll` through `scroll + inner_height - 1` (clamped to the list length) are rendered.
6. **Zero-height guard:** When `inner_height == 0` the scroll offset is forced to `0` and no rows are rendered.
7. **Overflow clamp:** Rendering stops when the bookmark index reaches the end of the slice, leaving trailing rows blank.

## Constraints

- `cursor` is a caller-supplied `usize`; the widget does not clamp or validate it. Callers must ensure `cursor < bookmarks.len()` when the list is non-empty, otherwise the cursor row is never highlighted.
- Row y-coordinates are computed with `inner.y + row as u16`; `row` is bounded by `inner_height` (a `u16` cast to `usize`), so truncation on cast back to `u16` cannot occur within the rendered range.
- The widget borrows all input data for lifetime `'a`; it holds no owned allocations.
- No I/O, no side effects: pure `Buffer` mutation.

## Dependencies

- `lajjzy_core::types::ChangeDetail` — provides the `.description` field displayed on each row.
- `ratatui` — `Buffer`, `Rect`, `Widget`, `Block`, `Borders`, `Style`, `Color`, `Modifier`, `Line`, `Span`.
