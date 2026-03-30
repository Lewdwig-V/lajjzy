---
managed-file: crates/lajjzy-tui/src/widgets/op_log.rs
intent: >
  Renders a scrollable, cursor-tracked list of jj operation log entries inside a titled, blue-bordered box. Each entry displays its operation ID (yellow), timestamp (cyan), and description. Highlights the cursor row with REVERSED style. Auto-scrolls the viewport to keep the cursor visible. Renders a "(no operations)" placeholder when the entry list is empty.
intent-approved: false
intent-hash: 74beb3819a90
distilled-from:
  - path: crates/lajjzy-tui/src/widgets/op_log.rs
    hash: 9560c2f89ec3
non-goals:
  - Handling keyboard input or mutating cursor/scroll state — callers own navigation.
  - Executing or invoking jj operations — the widget is read-only display only.
  - Truncating or wrapping long entry fields — lines are set at full inner width with no overflow guard.
depends-on:
  - crates/lajjzy-core/src/types.spec.md
---

## Purpose

`OpLogWidget` is a stateless ratatui `Widget` that renders a paginated view of `OpLogEntry` items. Callers supply the full entry slice, the logical cursor index, and a scroll offset; the widget computes viewport alignment and draws the result into the provided `Buffer`.

## Behavior

1. **Border and title.** Always draws a full `Borders::ALL` box styled `Color::Blue` with the title `"Operation Log"` before any content.

2. **Empty state.** When `entries` is empty, renders the text `"(no operations)"` in `Color::DarkGray` on the first inner row and returns immediately — no further rows are drawn.

3. **Auto-scroll correction.** Before rendering rows the widget recalculates the effective scroll offset:
   - If inner height is zero, the supplied scroll offset is used unchanged.
   - If the cursor has moved past the bottom of the viewport (`cursor >= scroll + height`), scroll advances to `cursor - height + 1`.
   - If the cursor has moved above the top of the viewport (`cursor < scroll`), scroll resets to `cursor`.
   - Otherwise the supplied scroll offset is used unchanged.
   The corrected scroll value is local to the render call; it does not mutate the stored field.

4. **Row layout.** For each visible index in `[scroll, scroll + height)` that is within bounds:
   - `entry.id` is rendered in `Color::Yellow`.
   - Two spaces separate id from timestamp.
   - `entry.timestamp` is rendered in `Color::Cyan`.
   - Two spaces separate timestamp from description.
   - `entry.description` is rendered with default style.

5. **Cursor highlight.** When a rendered row's index equals `cursor`, every cell across the full inner width of that row is post-styled with `Modifier::REVERSED`.

6. **Viewport clipping.** Rows are rendered only up to `inner.height` rows and only up to `entries.len()` entries; whichever limit is reached first stops iteration.

## Constraints

- `cursor` and `scroll` are caller-supplied `usize` values; the widget does not validate that `cursor < entries.len()`. Callers must ensure `cursor` is a valid index when `entries` is non-empty to avoid rendering a highlight on a non-existent row.
- Row `y` position is computed as `inner.y + row as u16`; rendering more than `u16::MAX` rows in a single call is not supported (guarded by `#[expect(clippy::cast_possible_truncation)]`).
- The widget borrows `entries` for lifetime `'a`; it does not clone or own the slice.

## Dependencies

- `lajjzy_core::types::OpLogEntry` — provides `id: String`, `timestamp: String`, `description: String` fields consumed during rendering.
- `ratatui` — `Buffer`, `Rect`, `Widget`, `Block`, `Borders`, `Style`, `Color`, `Modifier`, `Line`, `Span`.
