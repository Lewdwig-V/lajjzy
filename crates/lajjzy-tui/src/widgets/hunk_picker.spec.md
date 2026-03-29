---
managed_file: crates/lajjzy-tui/src/widgets/hunk_picker.rs
intent: >
  Renders a scrollable, flat list of files and their hunks for interactive hunk selection. Each file shows a header with path and selected/total hunk count. Each hunk shows a checkbox reflecting selection state, its diff-range header, and all constituent diff lines with per-line-kind foreground coloring. The cursor position and hunk selection state drive background tinting and REVERSED modifier highlighting. When no files are present, renders a single empty-state message.
intent-approved: false
intent-hash: c04090c91527
distilled-from:
  - path: crates/lajjzy-tui/src/widgets/hunk_picker.rs
    hash: 238836f2c6ec
non-goals:
  - Does not handle keyboard input or mutate picker state — rendering only
  - Does not persist or scroll the cursor; scroll offset is read from HunkPicker, not computed here
  - Does not apply syntax highlighting beyond the four DiffLineKind color slots
depends-on:
  - crates/lajjzy-tui/src/app.spec.md
  - crates/lajjzy-core/src/types.spec.md
---

## Purpose

`HunkPickerWidget` is a pure ratatui `Widget` that renders the current state of a `HunkPicker` into a `Buffer`. Callers construct it with `HunkPickerWidget::new(&picker)` and pass it to any ratatui layout that allocates a `Rect`. The widget produces no side effects and returns no values; all observable output is pixels written into the provided `Buffer`.

## Behavior

1. **Empty state.** When `picker.files` is empty, renders exactly one row containing the text `"(no hunks to pick)"` in `DarkGray`, at the top-left of the allocated area, and returns immediately.

2. **Flat render list.** For non-empty pickers, builds an ordered flat list of `PickerItem` variants:
   - One `FileHeader` per file, assigned the next sequential flat index.
   - One `Hunk` per hunk within that file, assigned the next flat index.
   - One `DiffLine` per diff line within that hunk, carrying the owning hunk's flat index and selection state (no flat index of their own — they are not cursor-landable).

3. **Scroll window.** Renders only the slice `items[scroll .. scroll + height]`, where `height = area.height`. Rows outside this window are never written.

4. **File header rows.**
   - Text: `"▸ <path>  [<selected>/<total>]"` where `<selected>` is the count of hunks with `selected == true` and `<total>` is the full hunk count for that file.
   - Style: `Cyan | BOLD`. When `flat_idx == picker.cursor`, adds `REVERSED`.

5. **Hunk rows.**
   - Text: `"  [✓] <header>"` when selected, `"  [ ] <header>"` when not selected.
   - Background: `Rgb(0, 40, 40)` when selected, `Reset` when not selected.
   - The entire row width is filled with the background style before the text line is written.
   - When `flat_idx == picker.cursor`, adds `REVERSED`.

6. **Diff line rows.**
   - Prefix character prepended to the content: `"+"` for `Added`, `"-"` for `Removed`, `" "` for `Context`, `""` (empty) for `Header`.
   - Full text: `"    <prefix><content>"` (four leading spaces). When prefix is empty the format is `"    <content>"` (no extra space).
   - Foreground: `Green` for `Added`, `Red` for `Removed`, `Reset` for `Context`, `DarkGray` for `Header`.
   - Background: `Rgb(0, 40, 40)` when `hunk_selected`, `Reset` otherwise.
   - The entire row width is filled with the computed style before the text line is written.
   - When the owning `hunk_flat_idx == picker.cursor`, adds `REVERSED` to the style (cursor highlights the whole hunk including its diff lines).

7. **Cursor semantics.** The cursor is a flat index into the combined file-header + hunk sequence. Diff lines do not have their own flat index; their highlighting derives entirely from their owning hunk's flat index.

## Constraints

- The flat index counter increments by 1 for each file header and by 1 for each hunk; diff lines do not increment the counter. This must remain consistent with the dispatch model that advances `picker.cursor`.
- Row coordinate arithmetic casts `row: usize` to `u16`; correctness relies on `row < area.height` (a `u16`), enforced structurally by `take(height)` — no explicit bounds check is needed.
- The widget is a pure read of `&HunkPicker`; it never mutates picker state.
- Rendering is stateless between frames: every call to `render` rebuilds the flat list from scratch.

## Dependencies

- **`crate::app::{HunkPicker, PickerFile, PickerHunk}`** — source data model; the widget holds a `&HunkPicker`.
- **`lajjzy_core::types::DiffLineKind`** — discriminant for diff line coloring and prefix selection.
- **`ratatui`** — `Buffer`, `Rect`, `Widget`, `Line`, `Style`, `Color`, `Modifier`.
