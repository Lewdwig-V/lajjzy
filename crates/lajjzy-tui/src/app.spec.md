---
managed_file: crates/lajjzy-tui/src/app.rs
version: 1
test_policy: "No tests — state struct, tested via dispatch tests"
---

# AppState and supporting types

## Purpose

Central state atom for the Elm-style TUI state machine. All TUI state lives
in `AppState`. Dispatch mutates it; render reads it. Supporting types define
layout caching, conflict resolution state, target picking, and hunk picking.

## Dependencies

- `std::collections::{HashMap, HashSet}`
- `lajjzy_core::forge::{ForgeKind, PrInfo}`
- `lajjzy_core::types::{ChangeDetail, ConflictData, ConflictRegion, DiffHunk, DiffLine, GraphData}`
- `ratatui::layout::Rect`
- `crate::action::{Action, BackgroundKind, DetailMode, HunkPickerOp, MutationKind, PanelFocus, RebaseMode}`
- `crate::modal::{HelpContext, Modal}`

## Re-exports

```rust
pub use crate::action::{Action, BackgroundKind, DetailMode, MutationKind, PanelFocus};
pub use crate::modal::{HelpContext, Modal};
```

`HunkPickerOp` and `RebaseMode` are `use`d (not `pub use`) — crate-internal only.

## Types

### LayoutRects

```rust
#[derive(Debug, Clone, Default)]
pub struct LayoutRects {
    pub graph_inner: Rect,
    pub detail_inner: Rect,
    pub graph_outer: Rect,
    pub detail_outer: Rect,
    pub modal_area: Option<Rect>,
    pub graph_scroll_offset: usize,
}
```

**Constructor:** `from_outer_rects(graph_outer: Rect, detail_outer: Rect) -> Self` derives inner
rects by shrinking each outer rect by 1 cell on all sides (matching `Borders::ALL`). Sets
`modal_area` to `None` and `graph_scroll_offset` to 0.

**Helper:** Private `fn shrink_by_border(r: Rect) -> Rect` — `Rect::new(r.x + 1, r.y + 1, r.width.saturating_sub(2), r.height.saturating_sub(2))`.

### HunkResolution

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HunkResolution { Unresolved, AcceptLeft, AcceptRight }
```

Per-hunk resolution state for the conflict view. Lives in `lajjzy-tui` (never in backend).

### PickingMode

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickingMode { Browsing, Filtering { query: String } }
```

### TargetPick

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct TargetPick {
    pub source: String,
    pub mode: RebaseMode,
    pub excluded: HashSet<String>,
    pub picking: PickingMode,
    pub original_change_id: String,  // restored on cancel, survives graph refreshes
    pub descendant_count: usize,
}
```

### HunkPicker

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct HunkPicker {
    pub operation: HunkPickerOp,
    pub files: Vec<PickerFile>,
    pub cursor: usize,
    pub scroll: usize,
    pub viewport_height: usize,  // set by event loop before dispatch
}
```

### PickerFile / PickerHunk

```rust
pub struct PickerFile { pub path: String, pub hunks: Vec<PickerHunk> }
pub struct PickerHunk { pub header: String, pub lines: Vec<DiffLine>, pub selected: bool }
```

Both derive `Debug, Clone, PartialEq`.

### ConflictView

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ConflictView {
    pub change_id: String,
    pub path: String,
    pub data: ConflictData,
    pub resolutions: Vec<HunkResolution>,  // parallel to Conflict variants only
    pub cursor: usize,      // indexes into conflict hunks (0..N)
    pub scroll: usize,
    pub viewport_height: usize,
}
```

**Constructor invariant:** `ConflictView::new(change_id, path, data)` computes `resolutions`
by counting `Conflict` regions in `data.regions` and filling with `HunkResolution::Unresolved`.
Cursor, scroll, viewport_height initialized to 0. This is the only construction site.

### AppState

```rust
pub struct AppState {
    pub graph: GraphData,
    pub(crate) cursor: usize,
    pub should_quit: bool,
    pub error: Option<String>,
    pub focus: PanelFocus,
    pub(crate) detail_cursor: usize,
    pub detail_mode: DetailMode,
    pub diff_scroll: usize,
    pub diff_data: Vec<DiffHunk>,
    pub modal: Option<Modal>,
    pub(crate) pending_mutation: Option<MutationKind>,
    pub(crate) pending_background: HashSet<BackgroundKind>,
    pub status_message: Option<String>,
    pub(crate) cursor_follows_working_copy: bool,
    pub(crate) graph_generation: u64,
    pub active_revset: Option<String>,
    pub(crate) omnibar_fallback_idx: Option<usize>,
    pub target_pick: Option<TargetPick>,
    pub hunk_picker: Option<HunkPicker>,
    pub conflict_view: Option<ConflictView>,
    pub forge: Option<ForgeKind>,
    pub pr_status: HashMap<String, PrInfo>,
    pub pending_forge_fetch: bool,
    pub layout: LayoutRects,
}
```

No derives — struct is too large for `Clone`/`Debug` to be useful.

**Visibility:** Fields that should only be mutated through dispatch use `pub(crate)`:
`cursor`, `detail_cursor`, `pending_mutation`, `pending_background`,
`cursor_follows_working_copy`, `graph_generation`, `omnibar_fallback_idx`.

**Constructor:** `AppState::new(graph: GraphData, forge: Option<ForgeKind>) -> Self`
- Cursor initialized to `graph.working_copy_index`, falling back to first node index, then 0
- All other fields to defaults/empty

**Accessors:**
- `cursor(&self) -> usize`
- `detail_cursor(&self) -> usize`
- `selected_change_id(&self) -> Option<&str>` — looks up `graph.lines[cursor].change_id`
- `selected_detail(&self) -> Option<&ChangeDetail>` — delegates to `graph.detail_at(cursor)`
- `reset_detail(&mut self)` — resets detail_cursor to 0, detail_mode to FileList, clears diff state

**Test helpers** (behind `#[cfg(test)]`):
- `set_cursor_for_test(&mut self, index: usize)`
- `set_detail_cursor_for_test(&mut self, index: usize)`
