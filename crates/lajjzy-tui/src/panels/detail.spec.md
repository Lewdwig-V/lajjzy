---
managed-file: crates/lajjzy-tui/src/panels/detail.rs
intent: >
  Renders the detail panel, a bordered region that switches its title and inner widget based on the current DetailMode (FileList, DiffView, HunkPicker, ConflictView). When focused, the border is Blue; when unfocused, DarkGray. Each mode composes a context-aware title from AppState (change description, file path, hunk operation source/destination, conflict path and hunk progress) and delegates content rendering to the appropriate sub-widget (FileListWidget, DiffViewWidget, HunkPickerWidget, ConflictViewWidget).
intent-approved: false
intent-hash: 55bb44134014
distilled-from:
  - path: crates/lajjzy-tui/src/panels/detail.rs
    hash: 24d00718d8af
non-goals:
  - Does not own or mutate AppState; it is a pure read-only render function
  - Does not handle keyboard input or dispatch Actions; input routing is the responsibility of the input layer
  - Does not perform any layout splitting; it receives a single Rect and renders entirely within it
depends-on:
  - crates/lajjzy-tui/src/app.spec.md
  - crates/lajjzy-tui/src/action.spec.md
  - crates/lajjzy-tui/src/widgets/file_list.spec.md
  - crates/lajjzy-tui/src/widgets/diff_view.spec.md
  - crates/lajjzy-tui/src/widgets/hunk_picker.spec.md
  - crates/lajjzy-tui/src/widgets/conflict_view.spec.md
---

## Purpose

Callers invoke `render(frame, state, area)` to paint the detail panel into a given `Rect`. The panel presents the secondary-detail layer of the UI — whichever of four sub-views is active — inside a titled, bordered block whose visual state (border color, title text) reflects both the current focus and the selected change/file/operation.

## Behavior

1. **Focus border color.** When `state.focus == PanelFocus::Detail` the border is `Color::Blue`; otherwise `Color::DarkGray`.

2. **FileList mode title.** When `state.detail_mode == DetailMode::FileList`:
   - If a selected detail exists, the title is `"Files — {description}"` where `{description}` is the change's description string, or `"(no description)"` if the description is empty.
   - If no selected detail exists, the title is `"Files"`.

3. **DiffView mode title.** When `state.detail_mode == DetailMode::DiffView`:
   - If a selected detail exists and `state.detail_cursor()` resolves to a file, the title is `"Diff — {path}"`.
   - Otherwise the title is `"Diff"`.

4. **HunkPicker mode title.** When `state.detail_mode == DetailMode::HunkPicker`:
   - If `state.hunk_picker` is `Some` with a `Split` operation, the title is `"Split — {source}"`.
   - If `state.hunk_picker` is `Some` with a `Squash` operation, the title is `"Squash — {source} → {destination}"`.
   - If `state.hunk_picker` is `None`, the title is `"Hunk Picker"`.

5. **ConflictView mode title.** When `state.detail_mode == DetailMode::ConflictView`:
   - If `state.conflict_view` is `Some`, the title is `"Conflict — {path} (hunk {cursor+1}/{total})"` where `{total}` is `resolutions.len()`.
   - If `state.conflict_view` is `None`, the title is `"Conflict"`.

6. **Content rendering.** After rendering the border block, the inner `Rect` is passed to the appropriate sub-widget:
   - `FileList` → `FileListWidget::new(files, cursor, focused)` using the selected detail's file slice (empty slice if no detail).
   - `DiffView` → `DiffViewWidget::new(&state.diff_data, state.diff_scroll)`.
   - `HunkPicker` → `HunkPickerWidget::new(hp)` only when `state.hunk_picker` is `Some`; nothing rendered if `None`.
   - `ConflictView` → `ConflictViewWidget::new(cv)` only when `state.conflict_view` is `Some`; nothing rendered if `None`.

## Constraints

- `render` is a pure rendering function: it reads `AppState` and writes to `Frame`; it never mutates state or emits side effects.
- If `state.hunk_picker` or `state.conflict_view` is `None` while the corresponding mode is active, the inner area is left blank (no panic, no fallback widget).
- The border is always drawn before the inner widget; inner content is clipped to `block.inner(area)`.
- `detail_cursor()` is used for both the file-path title lookup and the `FileListWidget` cursor argument, ensuring the highlighted row matches the title.

## Dependencies

- `ratatui` — `Frame`, `Rect`, `Block`, `Borders`, `Style`, `Color` (runtime rendering primitives)
- `crate::app` — `AppState`, `DetailMode`, `PanelFocus` (read-only state access)
- `crate::action` — `HunkPickerOp` variants `Split` and `Squash` (pattern-matched for HunkPicker title)
- `crate::widgets::file_list::FileListWidget` — renders file list in FileList mode
- `crate::widgets::diff_view::DiffViewWidget` — renders diff content in DiffView mode
- `crate::widgets::hunk_picker::HunkPickerWidget` — renders hunk selection UI in HunkPicker mode
- `crate::widgets::conflict_view::ConflictViewWidget` — renders conflict resolution UI in ConflictView mode
