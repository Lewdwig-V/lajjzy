# M3c: Split & Partial Squash — Interactive Hunk Picker

**Date:** 2026-03-21
**Status:** Draft
**Depends on:** M3b (complete)

## Motivation

Without split and partial squash, the TUI can't decompose changes — a change that modifies three files can't be broken into "the refactor" and "the feature." `jj split` and `jj squash --interactive` are the CLI tools for this, but they launch an external diff editor. M3c builds an inline hunk picker that replaces the external editor, keeping the user in the TUI.

## Scope

### In scope (M3c)

- `s` = split (hunk picker, selected hunks → new child change)
- `S` = partial squash (hunk picker, selected hunks → parent). Replaces instant full squash.
- Hunk picker replaces detail pane (not modal/overlay). Graph stays visible.
- Single-column scrollable list with file headers and hunk selection
- File-level and hunk-level toggle
- `RepoBackend::change_diff` for loading all file hunks at once
- `RepoBackend::split` and `squash_partial` with `FileHunkSelection`
- Two-tier backend: file paths for unanimous files, `--tool` helper for mixed hunks

### Out of scope

- Line-level hunk selection (future)
- Move hunks to arbitrary target (`m` key, M6)
- jj-lib direct tree construction (deferred to C4 audit)

## Operations and Key Bindings

| Key | Operation | Hunk picker? | Direction |
|-----|-----------|-------------|-----------|
| `s` | Split | Yes | Selected hunks → new child change |
| `S` | Partial squash | Yes | Selected hunks → parent change |
| `m` | Move hunks | Target picker → hunk picker | Out of scope (M6) |

**`s` (split):** Opens hunk picker on the selected change's diff. User selects hunks. Enter emits `Effect::Split`. The selected hunks stay in the original change; the unselected hunks go to a new child. Status bar: `Split: 2/3 hunks → new change after ksqxwpml`

**`S` (partial squash):** Opens hunk picker. Selected hunks move to the parent. Enter emits `Effect::SquashPartial`. Status bar: `Squash: 2/3 hunks from ksqxwpml → into ytoqrzxn`. Full squash is `a` (select all) then Enter.

**`S` is no longer instant.** The M2 instant `Effect::Squash` is replaced by the hunk picker flow. Full squash = open picker, `a`, Enter. This is one extra step but unifies the mental model: `S` always means "push hunks to parent."

**Interaction pattern:** Mini-modal (hunk picker is a state on the detail pane).

## Hunk Picker Widget — Layout and Navigation

### Layout

The detail pane becomes the hunk picker. Graph panel stays visible on the left with the source change highlighted. Single column, scrollable.

```
┌─ Graph (1/3) ──────┬─ Hunk Picker (2/3) ─────────────────┐
│                     │ Split ksqxwpml — select hunks        │
│  ◉ ksqxwpml  ←     │                                      │
│  ◉ ytoqrzxn        │ ▸ src/lib.rs                  [2/3]  │
│  ◉ vlpmrokx        │   [✓] @@ -10,3 +10,5 @@             │
│  ◆ zzzzzzzz        │       fn setup() {                   │
│                     │      +    let config = load();       │
│                     │      +    init(config);              │
│                     │   [ ] @@ -25,2 +27,4 @@             │
│                     │       fn cleanup() {                 │
│                     │      +    flush();                   │
│                     │   [✓] @@ -40,1 +44,3 @@             │
│                     │       fn reset() {                   │
│                     │      +    clear_cache();             │
│                     │ ▸ src/config.rs               [1/1]  │
│                     │   [✓] @@ -1,0 +1,15 @@              │
│                     │       (new file)                     │
├─────────────────────┴──────────────────────────────────────┤
│ Split: 3/4 hunks selected │ Enter confirm │ Esc cancel     │
└────────────────────────────────────────────────────────────┘
```

### Visual design

- Selected hunks have a distinct background tint (subtle cyan/green)
- Unselected hunks render normally
- File headers show `[n/m]` count of selected/total hunks
- Cursor highlights the current hunk (or file header)
- Diff lines render with existing color scheme (green added, red removed, gray context)

### Navigation

| Key | Action |
|-----|--------|
| `j` / `k` | Move between hunks and file headers |
| `J` / `K` | Jump to next/previous file header |
| `Space` | Toggle selection on current hunk or file header |
| `a` | Select all hunks |
| `A` | Deselect all hunks |
| `Enter` | Confirm — emit split/squash effect |
| `Esc` | Cancel — exit hunk picker, return to file list |

**Space on a file header** toggles all hunks in that file. If any are unselected, selects all. If all are selected, deselects all.

**Starting state:** Nothing selected. The user builds the selection explicitly. This makes the "what am I moving" question always explicit — you're moving exactly the hunks you selected, nothing implicit.

**Status bar preview:**
- Split: `Split: 2/3 hunks → new change after ksqxwpml`
- Squash: `Squash: 2/3 hunks from ksqxwpml → into ytoqrzxn`

The direction of movement is unambiguous before the user presses Enter.

## Data Model and State

### New types in `lajjzy-core`

```rust
/// All hunks for a single file in a change's diff.
pub struct FileDiff {
    pub path: String,
    pub hunks: Vec<DiffHunk>,
}

/// User's hunk selection for a single file.
pub struct FileHunkSelection {
    pub path: String,
    /// Indices of selected hunks (into the file's hunk list).
    pub selected_hunks: Vec<usize>,
    pub total_hunks: usize,
}
```

### Hunk picker state on `AppState`

```rust
pub struct HunkPicker {
    /// The operation being performed
    pub operation: HunkPickerOp,
    /// All files and their hunks with selection state
    pub files: Vec<PickerFile>,
    /// Flat cursor index across all selectable items (file headers + hunks)
    pub cursor: usize,
    /// Scroll offset for the visible window
    pub scroll: usize,
}

pub enum HunkPickerOp {
    Split { source: String },
    Squash { source: String, destination: String },
}

pub struct PickerFile {
    pub path: String,
    pub hunks: Vec<PickerHunk>,
}

pub struct PickerHunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
    pub selected: bool,
}
```

On `AppState`:
```rust
pub hunk_picker: Option<HunkPicker>,
```

### Detail mode

```rust
pub enum DetailMode {
    FileList,
    DiffView,
    HunkPicker,  // new
}
```

When `detail_mode == HunkPicker`, the detail pane renders the hunk picker widget. Focus is `PanelFocus::Detail`.

### Flat cursor model

The cursor is a flat index across all selectable items. File headers and hunks are both items in the list. For a change with 2 files (3 hunks and 2 hunks):

```
Index 0: file header "src/lib.rs"
Index 1: hunk @@ -10,3 +10,5 @@
Index 2: hunk @@ -25,2 +27,4 @@
Index 3: hunk @@ -40,1 +44,3 @@
Index 4: file header "src/config.rs"
Index 5: hunk @@ -1,0 +1,15 @@
```

The widget computes which item the cursor points to by walking this flat list.

## Backend Contract

### New `RepoBackend` methods

```rust
fn change_diff(&self, change_id: &str) -> Result<Vec<FileDiff>>;
fn split(&self, change_id: &str, selections: &[FileHunkSelection]) -> Result<String>;
fn squash_partial(&self, change_id: &str, selections: &[FileHunkSelection]) -> Result<String>;
```

### `change_diff` implementation

`jj diff -r <change_id> --git --color=never`, parsed into per-file `FileDiff` groups. The existing `parse_diff_output` handles multi-file diffs — restructure to group hunks by the `diff --git a/<path> b/<path>` header lines.

### Two-tier split/squash implementation

**File-level fast path:** If all hunks in a file are selected (or all deselected), use `jj split <selected_paths>` / `jj squash <selected_paths>`. No `--tool` needed.

**Hunk-level fallback:** If some hunks within a file are selected, use `jj split --tool <helper>` / `jj squash --tool <helper>` where the helper program applies the pre-computed hunk selection by writing the desired file content.

**Future (C4 audit):** jj-lib likely exposes tree construction directly (`MergedTree`), bypassing the `--tool` mechanism entirely. The `RepoBackend` trait signature (`Vec<FileHunkSelection>`) supports both CLI and library implementations without change.

### Status messages

- Split: `Split ksqxwpml: 2 files, 3 hunks → new change`
- Squash partial: `Squashed 3 hunks from ksqxwpml into ytoqrzxn`

## Dispatch Logic

### Entering the hunk picker

Both `s` and `S` emit `Effect::LoadChangeDiff` to fetch all hunks:

```rust
Effect::LoadChangeDiff {
    change_id: String,
    operation: HunkPickerOp,
}
```

The executor calls `backend.change_diff(&change_id)`, then sends:

```rust
Action::ChangeDiffLoaded {
    operation: HunkPickerOp,
    result: Result<Vec<FileDiff>, String>,
}
```

### `ChangeDiffLoaded` handler

On success: populates `state.hunk_picker`, switches `detail_mode` to `HunkPicker`, sets focus to `PanelFocus::Detail`.

On failure: sets `state.error`.

### `S` (Squash) change

`S` no longer emits instant `Effect::Squash`. Instead:

```rust
Action::SquashPartial => {
    if state.pending_mutation.is_some() || state.hunk_picker.is_some() {
        state.status_message = Some("Operation in progress…".into());
        return vec![];
    }
    if let Some(cid) = state.selected_change_id().map(String::from) {
        let parent = state.selected_detail()
            .and_then(|d| d.parents.first().cloned());
        match parent {
            Some(dest) => return vec![Effect::LoadChangeDiff {
                change_id: cid,
                operation: HunkPickerOp::Squash { source: cid, destination: dest },
            }],
            None => state.error = Some("Cannot squash: no parent change".into()),
        }
    }
}
```

### Hunk picker actions

```rust
Action::HunkToggle        // Space
Action::HunkSelectAll     // a
Action::HunkDeselectAll   // A
Action::HunkNextFile      // J
Action::HunkPrevFile      // K
Action::HunkConfirm       // Enter
Action::HunkCancel        // Esc
```

`DetailMoveDown`/`DetailMoveUp` handle `j`/`k` movement in `HunkPicker` mode.

### `HunkConfirm`

Takes `state.hunk_picker`, builds `Vec<FileHunkSelection>`, emits effect:

```rust
HunkPickerOp::Split { source } => Effect::Split { change_id: source, selections },
HunkPickerOp::Squash { source, destination } =>
    Effect::SquashPartial { change_id: source, selections },
```

Sets `pending_mutation`. Clears `hunk_picker`, resets `detail_mode` to `FileList`.

If nothing is selected, shows error: "No hunks selected".

### `HunkCancel`

Clears `hunk_picker`, resets `detail_mode` to `FileList`. No mutation.

## Input Routing

New `DetailMode::HunkPicker` branch in `map_event` under `PanelFocus::Detail`:

```rust
DetailMode::HunkPicker => match (event.code, event.modifiers) {
    (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => Some(Action::DetailMoveDown),
    (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => Some(Action::DetailMoveUp),
    (KeyCode::Char('J'), _) => Some(Action::HunkNextFile),
    (KeyCode::Char('K'), _) => Some(Action::HunkPrevFile),
    (KeyCode::Char(' '), _) => Some(Action::HunkToggle),
    (KeyCode::Char('a'), KeyModifiers::NONE) => Some(Action::HunkSelectAll),
    (KeyCode::Char('A'), _) => Some(Action::HunkDeselectAll),
    (KeyCode::Enter, _) => Some(Action::HunkConfirm),
    (KeyCode::Esc, _) => Some(Action::HunkCancel),
    _ => None,
},
```

Graph-context mutation keys don't fire during hunk picker — user is in `PanelFocus::Detail`, and mutation keys only match `PanelFocus::Graph`. Tab/BackTab still work for panel switching.

## Testing Strategy

### Dispatch tests

1. `split_emits_load_change_diff` — `s` emits effect with Split op
2. `squash_partial_emits_load_change_diff` — `S` emits effect with Squash op
3. `split_suppressed_while_pending` — mutation gate blocks
4. `squash_partial_on_root_shows_error` — no parent
5. `change_diff_loaded_opens_hunk_picker` — detail_mode, hunk_picker populated, all unselected
6. `change_diff_loaded_error_sets_error` — error path
7. `hunk_toggle_selects_and_deselects` — Space toggles
8. `hunk_toggle_on_file_header_toggles_all` — file-level toggle
9. `hunk_select_all_and_deselect_all` — `a` and `A`
10. `hunk_next_file_and_prev_file` — `J`/`K`
11. `hunk_confirm_emits_split_effect` — correct `FileHunkSelection`
12. `hunk_confirm_emits_squash_partial_effect` — squash variant
13. `hunk_confirm_with_nothing_selected_shows_error` — empty selection rejected
14. `hunk_cancel_exits_picker` — restores FileList mode
15. `detail_move_down_up_in_hunk_picker` — flat cursor traversal

### Backend tests

16. `change_diff_returns_grouped_file_diffs` — real repo, multiple files
17. `split_on_real_repo` — file-level split, verify two changes
18. `squash_partial_on_real_repo` — file-level partial squash

### Input tests

19. `hunk_picker_key_routing` — all keys in DetailMode::HunkPicker
20. `s_key_maps_to_split` — Graph context
21. `S_key_maps_to_squash_partial` — Graph context (changed from instant)

### Widget tests

22. `hunk_picker_renders_files_and_hunks` — file headers, hunks, markers
23. `hunk_picker_selected_hunks_have_tint` — background color
24. `hunk_picker_file_header_shows_count` — `[2/3]` format

## File Changes

| File | Changes |
|------|---------|
| `crates/lajjzy-core/src/types.rs` | Add `FileDiff`, `FileHunkSelection` |
| `crates/lajjzy-core/src/backend.rs` | Add `change_diff`, `split`, `squash_partial` |
| `crates/lajjzy-core/src/cli.rs` | Implement new methods. Restructure `parse_diff_output` for per-file grouping. |
| `crates/lajjzy-tui/src/action.rs` | Add `Split`, `SquashPartial`, `HunkToggle`, `HunkSelectAll`, `HunkDeselectAll`, `HunkNextFile`, `HunkPrevFile`, `HunkConfirm`, `HunkCancel`. Add `HunkPickerOp`. Remove `Squash` action. |
| `crates/lajjzy-tui/src/app.rs` | Add `HunkPicker`, `PickerFile`, `PickerHunk`, `hunk_picker: Option<HunkPicker>`. Add `DetailMode::HunkPicker`. |
| `crates/lajjzy-tui/src/effect.rs` | Add `LoadChangeDiff`, `Split`, `SquashPartial`. Remove `Squash`. |
| `crates/lajjzy-tui/src/dispatch.rs` | Hunk picker handlers. `S` opens picker instead of instant squash. `ChangeDiffLoaded` handler. Remove old `Squash` handler. |
| `crates/lajjzy-tui/src/input.rs` | Add `DetailMode::HunkPicker` branch. Change `S` from `Squash` to `SquashPartial`. Add `s` → `Split`. |
| `crates/lajjzy-tui/src/render.rs` | Render hunk picker widget when `detail_mode == HunkPicker`. |
| `crates/lajjzy-tui/src/widgets/hunk_picker.rs` | New widget — file headers, hunks, selection markers, tinting. |
| `crates/lajjzy-tui/src/widgets/mod.rs` | Add `pub mod hunk_picker`. |
| `crates/lajjzy-tui/src/widgets/status_bar.rs` | Hunk picker status text with selection count and direction. |
| `crates/lajjzy-tui/src/widgets/help.rs` | Update `s`/`S` keys in help. |
| `crates/lajjzy-cli/src/main.rs` | Handle new effects. Add to `next_graph_generation`. Remove old `Squash` effect handling. |
