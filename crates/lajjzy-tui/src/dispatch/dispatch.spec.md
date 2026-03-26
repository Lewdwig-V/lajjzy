---
managed_file: crates/lajjzy-tui/src/dispatch/mod.rs
version: 1
test_policy: "Tests exist in protected #[cfg(test)] region — Builder must preserve verbatim"
depends-on:
  - crates/lajjzy-tui/src/action.spec.md
  - crates/lajjzy-tui/src/app.spec.md
  - crates/lajjzy-tui/src/effect.spec.md
  - crates/lajjzy-tui/src/modal.spec.md
  - crates/lajjzy-core/src/types.spec.md
protected-regions:
  - marker: "#[cfg(test)]"
    position: tail
    semantics: test-suite
---

# Dispatch — pure state machine core

## Purpose

Single entry point `dispatch(&mut AppState, Action) -> Vec<Effect>` that
processes every user action and async result. Pure: mutates `AppState` in
place, returns effects for I/O. No backend calls, no subprocess spawning.

## Dependencies

- `std::collections::{HashMap, HashSet}`
- `lajjzy_core::types::{ConflictData, ConflictRegion, FileHunkSelection, FileStatus, GraphData}`
- `nucleo_matcher` — fuzzy matching for omnibar
- `crate::action::*` — Action, enums
- `crate::app::*` — AppState, supporting types
- `crate::effect::Effect`
- `crate::modal::{HelpContext, Modal}`
- `crate::dispatch::omnibar::{compute_completions, extract_current_word}`

## Module structure

```rust
mod omnibar;  // items imported via private use, not pub mod
```

## Public API

```rust
#[expect(clippy::too_many_lines)]
pub fn dispatch(state: &mut AppState, action: Action) -> Vec<Effect>
```

## Private helpers

### Concurrency

- `clear_op_gate(state, op: MutationKind)` — exhaustive match clears the correct gate (local mutation or background push/fetch)

### Navigation

- `move_cursor_down(state) -> bool` — next valid node, respects picking exclusions
- `move_cursor_up(state) -> bool` — previous valid node, respects picking exclusions
- `snap_to_node(graph, line_index) -> Option<usize>` — nearest node at or above index, forward-scan fallback, empty-graph guard
- `picking_valid_nodes(state) -> Vec<usize>` — node indices filtered by exclusion set and pick filter query
- `change_matches_filter(cid, graph, query) -> bool` — case-insensitive substring against change ID, author, description, bookmarks
- `jump_to_first_matching(state)` — cursor to first valid node after filter change

### Conflict resolution

- `build_resolved_content(data, resolutions) -> Result<Vec<u8>, &'static str>` — assemble file bytes from conflict regions + per-hunk resolutions; Err if any unresolved or index mismatch
- `exit_conflict_view(state)` — clear conflict_view, reset detail_mode to FileList

### Hunk picker

- `picker_item_count(picker) -> usize` — total flat items (file headers + hunks)
- `picker_item_at(picker, flat_index) -> Option<(file_idx, Option<hunk_idx>)>` — decode flat index
- `picker_next_file_index(picker) -> Option<usize>` — next file header after cursor
- `picker_prev_file_index(picker) -> Option<usize>` — previous file header before cursor
- `picker_cursor_render_row(picker) -> usize` — visual row for scroll computation
- `picker_ensure_cursor_visible(picker, viewport_height)` — adjust scroll to keep cursor in view
- `build_hunk_picker(operation, file_diffs) -> HunkPicker` — convert FileDiff vec to picker state

### Graph

- `compute_descendants(source, graph) -> HashSet<String>` — BFS through reversed parent→child edges
- `fuzzy_match(query, graph) -> Vec<usize>` — nucleo-powered fuzzy search, returns node indices sorted by score descending

## Dispatch behavior by action category

### Preamble

On every action except `RevsetLoaded`: clear `omnibar_fallback_idx` to prevent stale cursor jumps.

### Navigation (MoveUp, MoveDown, JumpToTop, JumpToBottom, JumpToWorkingCopy, Quit, Refresh)

- Move up/down: delegate to `move_cursor_up`/`move_cursor_down`, reset detail
- Jump to top/bottom: first/last node index, reset detail
- JumpToWorkingCopy: cursor to `working_copy_index` if Some
- Quit: set `should_quit`
- Refresh: clear error, emit `LoadGraph` with active revset

### GraphLoaded { generation, result }

- **Staleness rejection**: if `generation < graph_generation`, discard
- **Success**: replace graph, try to restore cursor by change ID, fall back to working copy index, then first node, then 0. If `cursor_follows_working_copy`, use working copy index directly
- **Error**: set `state.error`
- **Post-load picking validation**: if source change gone, cancel picking with status message and restore cursor. If WithDescendants mode, recompute excluded set
- **Post-load conflict validation**: if file no longer conflicted, exit conflict view with message. If still conflicted, restore `detail_mode = ConflictView`

### Focus (TabFocus, BackTabFocus)

Toggle between Graph and Detail.

### Detail panel (DetailMoveDown/Up, DetailEnter, DetailBack, DiffScroll*, DiffNext/PrevHunk)

- Move down/up: if HunkPicker mode, navigate picker; else navigate file list
- DetailEnter: if conflicted file → working-copy gate → LoadConflictData; if renamed → extract dest path; else → LoadFileDiff
- DetailBack: DiffView→FileList, FileList→focus Graph, ConflictView→exit conflict view
- DiffScroll: bounded by total diff lines
- DiffNext/PrevHunk: jump by hunk header offset

### Modals (ToggleOpLog, OpLogLoaded, OpenBookmarks, OpenOmnibar, OpenHelp, ModalDismiss, ModalMove*, ModalEnter, Omnibar*)

- ToggleOpLog: toggle or emit LoadOpLog
- OpenBookmarks: collect all bookmarks from graph into BookmarkPicker
- OpenOmnibar: prefill with active revset, compute fuzzy matches and completions
- OpenHelp: context-sensitive (Graph/DetailFileList/DetailDiffView/ConflictView)
- ModalDismiss: if omnibar, also clear fallback and exit picking if active
- ModalMoveDown/Up: per-modal cursor/scroll logic; Omnibar prioritizes completion_cursor when completions present
- ModalEnter: BookmarkPicker→jump cursor; Omnibar empty+active revset→clear+LoadGraph; Omnibar non-empty→EvalRevset with fallback
- OmnibarInput/Backspace: update query, recompute matches and completions
- OmnibarAcceptCompletion: replace current word with selected completion

### Mutations (Abandon, Split, SquashPartial, NewChange, EditChange, Undo, Redo)

All gated by `pending_mutation.is_some()` → return early with status message.

- Abandon: emit Effect::Abandon
- Split/SquashPartial: emit LoadChangeDiff (SquashPartial also checks for parent, errors on root)
- NewChange: set `cursor_follows_working_copy`, emit Effect::New
- EditChange: emit Effect::Edit
- Undo/Redo: emit Effect::Undo/Redo

### Mutation results (RepoOpSuccess, RepoOpFailed)

- RepoOpSuccess: install graph (if present) BEFORE clearing gate (atomic), recursive dispatch of GraphLoaded for bundled graph, clear gate, set status message
- RepoOpFailed: clear gate, set error

### Git operations (GitPush, GitFetch)

- Push: gated by `pending_background` Push flag, requires bookmark on selected change
- Fetch: gated by `pending_background` Fetch flag
- Independent lanes — push and fetch don't block each other or local mutations

### Describe (OpenDescribe, DescribeSave, DescribeEscalateEditor, EditorComplete)

- OpenDescribe: gated, opens Describe modal with TextArea
- DescribeSave: extract text from TextArea, emit Describe effect
- DescribeEscalateEditor: extract text, emit SuspendForEditor
- EditorComplete: emit Describe effect with returned text

### Revset (RevsetLoaded)

- Staleness rejection via generation
- Success with empty graph: status message, don't replace
- Success with nodes: set active_revset, recursive GraphLoaded dispatch
- Error: show error, fall back to fuzzy jump via omnibar_fallback_idx

### Bookmarks (OpenBookmarkSet, BookmarkInput*, BookmarkInputConfirm, BookmarkDelete)

- OpenBookmarkSet: gated, opens BookmarkInput modal prefilled with existing bookmark
- Input/Backspace: append/pop chars
- Confirm: gated, emit BookmarkSet if non-empty
- Delete: gated, from BookmarkPicker, emit BookmarkDelete

### Rebase picking (RebaseSingle, RebaseWithDescendants, Pick*)

- RebaseSingle: enter picking with self excluded
- RebaseWithDescendants: enter picking with self + descendants excluded
- PickConfirm: validate not excluded and not non-matching in filter mode, emit RebaseSingle/RebaseWithDescendants
- PickCancel: from Filtering→Browsing; from Browsing→exit picking, restore cursor by ID
- PickFilterChar/Backspace: update filter, jump to first matching

### Hunk picker (HunkToggle, HunkSelectAll, HunkDeselectAll, HunkNextFile, HunkPrevFile, HunkConfirm, HunkCancel)

- Toggle: individual hunk or file header (all-or-nothing)
- SelectAll/DeselectAll: set all hunks
- NextFile/PrevFile: jump cursor to adjacent file header
- Confirm: validate at least one selected, no mixed per-file selection, build FileHunkSelection vec, emit Split or SquashPartial
- Cancel: clear picker, return to FileList

### Conflict view (ConflictDataLoaded, ConflictAcceptLeft/Right, ConflictNext/PrevHunk, ConflictScroll*, ConflictConfirm, ConflictLaunchMerge, MergeTool*)

- DataLoaded: populate ConflictView if has conflict hunks, else error
- AcceptLeft/Right: set resolution at cursor
- NextHunk/PrevHunk: bounded cursor movement
- ScrollDown/Up: bounded scroll
- Confirm: gated by mutation + working-copy; all resolved → build content → emit ResolveFile; else error
- LaunchMerge: gated similarly, emit LaunchMergeTool
- MergeToolComplete: install graph, clear gate, exit conflict view
- MergeToolFailed: clear gate, set error

### Conflict navigation (NextConflictFile, PrevConflictFile)

Cycle through conflicted files in the file list with wrapping.

### Forge (FetchForgeStatus, OpenOrCreatePr, ForgeStatusLoaded, PrViewUrl, PrCreateComplete, PrCreateFailed)

- FetchForgeStatus: requires forge, debounce via pending_forge_fetch
- OpenOrCreatePr: requires bookmark; if cached PR exists with matching head_ref → PrViewUrl; else emit OpenOrCreatePr effect
- ForgeStatusLoaded: populate pr_status keyed by head_ref
- PrViewUrl: set status_message with URL
- PrCreateComplete: trigger FetchForgeStatus
- PrCreateFailed: set error

### M7 operations (Absorb, DuplicateChange, Revert)

All gated by pending_mutation. Emit corresponding effect.

### Mouse (ClickGraphNode, ClickDetailItem, ClickFocusGraph/Detail, ScrollUp/Down)

- ClickGraphNode: focus graph, snap_to_node, reset detail
- ClickDetailItem: focus detail, clamp index to file count
- ClickFocus*: set focus
- ScrollUp/Down: per-panel — Graph: repeated move_cursor, Detail: depends on detail_mode (FileList/DiffView)

### Post-dispatch invariant

Release-mode check: if cursor points to non-node line, log error and snap to first node.

## Invariants

1. `dispatch()` never performs I/O — all side effects are returned as `Vec<Effect>`
2. At most one local mutation in flight (`pending_mutation`)
3. Push and fetch have independent background gates
4. Graph generation is monotonically increasing; stale loads are rejected
5. Cursor always lands on a node line (enforced by post-dispatch check)
6. Picking mode navigation skips excluded changes
7. RepoOpSuccess installs graph BEFORE clearing mutation gate (atomicity)
