---
managed-file: crates/lajjzy-tui/src/widgets/file_list.rs
intent: >
  Renders a scrollable list of file changes for a jj change, displaying each
  file with a status indicator and path. Highlights the cursor row with
  REVERSED style when focused or BOLD when unfocused. Shows a dimmed
  placeholder message when the file list is empty. Uses status-specific
  foreground colors: green for Added, yellow for Modified, red for Deleted,
  cyan for Renamed, light-red for Conflicted, and magenta for Unknown. Renders
  at most area.height entries, clipping any remainder.
intent-approved: false
intent-hash: 1a84f2810a8a
distilled-from:
  - path: crates/lajjzy-tui/src/widgets/file_list.rs
    hash: 87d7970fc388
non-goals:
  - Scrolling or viewport offset management — the caller is responsible for
    slicing the file slice before passing it in
  - Sorting or filtering files by status or path
  - Handling mouse events or focus changes — focus state is passed in as a
    boolean and not mutated
depends-on:
  - crates/lajjzy-core/src/types.spec.md
---

## Purpose

`FileListWidget` is a stateless ratatui `Widget` that renders a fixed-height
panel of file-change entries. Callers create it with a file slice, a cursor
index, and a focus flag, then call `render(area, buf)` exactly once. The widget
writes directly into the provided `Buffer` and returns nothing.

## Behavior

1. **Empty list.** When `files` is empty, renders `"(no files changed)"` in
   `Color::DarkGray` at `(area.x, area.y)` and returns immediately without
   rendering any rows.

2. **Row layout.** Each `FileChange` at index `i` is rendered on row
   `area.y + i` starting at column `area.x`. The row text is
   `"  <status_char> <path>"` (two leading spaces, one space between status
   character and path).

3. **Status character.** For `FileStatus::Conflicted` the status character is
   `"⚠"`. For all other statuses the character is produced by
   `FileStatus::to_string()` (e.g. `"A"`, `"M"`, `"D"`, `"R"`).

4. **Status color.** Each row is styled with a status-specific foreground
   color:
   - `Added` → `Color::Green`
   - `Modified` → `Color::Yellow`
   - `Deleted` → `Color::Red`
   - `Renamed` → `Color::Cyan`
   - `Conflicted` → `Color::LightRed`
   - `Unknown(_)` → `Color::Magenta`

5. **Cursor highlight.** The row whose index equals `cursor` receives an
   additional modifier on top of the status color:
   - When `focused` is `true`: `Modifier::REVERSED`
   - When `focused` is `false`: `Modifier::BOLD`
   All other rows use the plain status color with no modifier.

6. **Height clipping.** Rows are rendered only while `i < area.height as
   usize`. Entries beyond that index are silently skipped; no wrapping or
   scrolling occurs.

7. **Width clipping.** Each row is written with `buf.set_line(…, area.width)`,
   so ratatui truncates text that exceeds the available column width.

## Constraints

- `cursor` is a `usize` and is not bounds-checked against `files.len()`. If
  `cursor >= files.len()` no row receives the cursor highlight, which is
  silently safe.
- The widget borrows `files` for lifetime `'a`; it holds no owned state and is
  consumed on `render`.
- No I/O, no allocation beyond the formatted line string, and no side effects
  outside writing to `buf`.

## Dependencies

- `ratatui` — `Buffer`, `Rect`, `Style`, `Color`, `Modifier`, `Line`, `Widget`
- `lajjzy_core::types::FileChange` — `path: String`, `status: FileStatus`
- `lajjzy_core::types::FileStatus` — enum with `Added`, `Modified`, `Deleted`,
  `Renamed`, `Conflicted`, `Unknown(String)` variants; must implement
  `Display` (used via `to_string()` for non-Conflicted statuses)
