---
managed_file: crates/lajjzy-tui/src/widgets/describe.rs
intent: >
  Renders a bordered, yellow-outlined modal editor pane that displays multi-line commit-message text from a tui_textarea::TextArea, draws a cursor highlight at the active cursor position, and shows keybinding hints (Alt-Enter/Ctrl-S save, Esc cancel, Shift-E editor) in the border title.
intent-approved: false
intent-hash: d92e408fd579
distilled-from:
  - path: crates/lajjzy-tui/src/widgets/describe.rs
    hash: 53bd00329440
non-goals:
  - Does not handle keyboard input or mutate TextArea state — input is processed upstream
  - Does not manage scroll offset or virtual viewport — only lines that fit in the inner area are rendered
  - Does not implement the tui-textarea Widget trait directly — rendering is manual to bridge the ratatui 0.29/0.30 version mismatch
depends-on:
  - crates/lajjzy-tui/src/widgets/status_bar.spec.md
---

## Purpose

`DescribeWidget` is a read-only rendering adapter for `tui_textarea::TextArea`. Callers supply a borrowed `TextArea` reference; the widget renders it inside a yellow-bordered block with a keybinding title bar each frame. It exists because tui-textarea 0.7 targets ratatui 0.29 while lajjzy-tui uses ratatui 0.30, making the upstream `Widget` impl link-incompatible.

## Behavior

1. **Border and title** — renders `Borders::ALL` with `Color::Yellow` border style. The title string is fixed: `" Describe (Alt-Enter/Ctrl-S save | Esc cancel | Shift-E editor) "`.
2. **Content rendering** — iterates `TextArea::lines()` row by row, writing each line into the inner area starting at `(inner.x, inner.y)`. Lines beyond `inner.height` are silently clipped.
3. **Cursor highlight** — for the row matching `TextArea::cursor().0` (cursor row), the cell at column `cursor.1` is styled `fg=Black / bg=White / BOLD`. The cursor column is clamped to `inner.width - 1` so it never escapes the inner area.
4. **Width clipping** — each line is written with `buf.set_line(..., inner.width)` which ratatui truncates at the inner width boundary.
5. **Zero-size safety** — `inner.width.saturating_sub(1)` prevents underflow when the inner area is zero-width.

## Constraints

- `TextArea<'static>` lifetime is required by `tui_textarea`; the widget borrows it for the duration of `render`.
- `render` is non-mutating with respect to `AppState`; it only reads `lines()` and `cursor()`.
- The title string is a compile-time constant — it must not be localised or parameterised.
- Cursor column clamping uses `u16` arithmetic; lines wider than `u16::MAX` columns are unsupported.

## Dependencies

- `ratatui` — `Buffer`, `Rect`, `Block`, `Borders`, `Style`, `Color`, `Modifier`, `Line`, `Widget`
- `tui_textarea::TextArea` — provides `lines()` and `cursor()` read accessors
