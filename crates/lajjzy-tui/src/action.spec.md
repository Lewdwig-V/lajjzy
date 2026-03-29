---
managed-file: crates/lajjzy-tui/src/action.rs
version: 1
test_policy: "No tests — enum definitions only"
---

# Action enum and supporting types

## Purpose

Define every possible user intent and async result that the dispatch function
can handle. The `Action` enum is the input type for `dispatch(&mut AppState, Action)`.

## Dependencies

- `lajjzy_core::forge::PrInfo`
- `lajjzy_core::types::{ConflictData, DiffHunk, FileDiff, GraphData, OpLogEntry}`

## Supporting enums and structs

### Arity

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arity { Nullary, Optional, Required }
```

Doc comments: `Nullary` = insert "()" complete, `Optional` = insert "(" user can close or add arg, `Required` = insert "(" needs argument.

### CompletionItem

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct CompletionItem {
    pub insert_text: String,   // e.g. "ancestors(", "mine()", "main"
    pub display_text: String,  // e.g. "ksqxwpml — refactor: extract trait"
}
```

### RebaseMode

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebaseMode { Single, WithDescendants }
```

### PanelFocus

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelFocus { Graph, Detail }
```

### DetailMode

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailMode { FileList, DiffView, HunkPicker, ConflictView }
```

### MutationKind

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MutationKind {
    Describe, New, Edit, Abandon, Split, SquashPartial,
    Undo, Redo, BookmarkSet, BookmarkDelete, GitPush, GitFetch,
    RebaseSingle, RebaseWithDescendants, ResolveConflict,
    Absorb, Duplicate, Revert,
}
```

### BackgroundKind

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackgroundKind { Push, Fetch }
```

### HunkPickerOp

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum HunkPickerOp {
    Split { source: String },
    Squash { source: String, destination: String },
}
```

## Action enum

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Action { ... }
```

Variants organized by category with `//` comment headers:

### Navigation
`MoveUp`, `MoveDown`, `Quit`, `Refresh`, `JumpToTop`, `JumpToBottom`,
`TabFocus`, `BackTabFocus`, `DetailMoveUp`, `DetailMoveDown`, `DetailEnter`,
`DetailBack`, `DiffScrollUp`, `DiffScrollDown`, `DiffNextHunk`, `DiffPrevHunk`,
`JumpToWorkingCopy`, `ToggleOpLog`, `OpenBookmarks`, `OpenOmnibar`, `OpenHelp`,
`ModalDismiss`, `ModalMoveUp`, `ModalMoveDown`, `ModalEnter`,
`OmnibarInput(char)`, `OmnibarBackspace`, `OmnibarAcceptCompletion`

### Effect result actions
- `GraphLoaded { generation: u64, result: Result<GraphData, String> }` — doc: generation is monotonic, dispatch rejects stale
- `OpLogLoaded(Result<Vec<OpLogEntry>, String>)`
- `FileDiffLoaded(Result<Vec<DiffHunk>, String>)`
- `ChangeDiffLoaded { operation: HunkPickerOp, result: Result<Vec<FileDiff>, String> }`
- `RepoOpSuccess { op: MutationKind, message: String, graph: Option<(u64, Result<GraphData, String>)> }` — doc: graph bundled so gate clears atomically
- `RepoOpFailed { op: MutationKind, error: String }`
- `EditorComplete { change_id: String, text: String }`
- `RevsetLoaded { query: String, generation: u64, result: Result<GraphData, String> }`

### Mutation trigger actions
`Abandon`, `Split`, `SquashPartial`, `NewChange`, `EditChange`, `OpenDescribe`,
`Undo`, `Redo`, `OpenBookmarkSet`, `BookmarkInputChar(char)`, `BookmarkInputBackspace`,
`BookmarkInputConfirm`, `BookmarkDelete`, `GitPush`, `GitFetch`, `DescribeSave`,
`DescribeEscalateEditor`, `RebaseSingle`, `RebaseWithDescendants`, `Absorb`,
`DuplicateChange`, `Revert`, `PickConfirm`, `PickCancel`,
`PickFilterChar(char)`, `PickFilterBackspace`

### Hunk picker actions
`HunkToggle`, `HunkSelectAll`, `HunkDeselectAll`, `HunkNextFile`, `HunkPrevFile`,
`HunkConfirm`, `HunkCancel`

### Conflict view actions
`ConflictAcceptLeft`, `ConflictAcceptRight`, `ConflictConfirm`, `ConflictLaunchMerge`,
`ConflictNextHunk`, `ConflictPrevHunk`, `ConflictScrollDown`, `ConflictScrollUp`

### File list conflict navigation
`NextConflictFile`, `PrevConflictFile`

### Conflict effect results
- `ConflictDataLoaded { change_id: String, path: String, result: Result<ConflictData, String> }`
- `MergeToolComplete { path: String, graph: Option<(u64, Result<GraphData, String>)> }`
- `MergeToolFailed { path: String, error: String }`

### Forge actions
`FetchForgeStatus`, `OpenOrCreatePr`,
`ForgeStatusLoaded(Result<Option<Vec<PrInfo>>, String>)`,
`PrViewUrl { url: String }`, `PrCreateComplete`, `PrCreateFailed { error: String }`

### Mouse actions
- `ClickGraphNode { line_index: usize }`
- `ClickDetailItem { index: usize }`
- `ClickFocusGraph`, `ClickFocusDetail`
- `ScrollUp { count: usize, panel: PanelFocus }`
- `ScrollDown { count: usize, panel: PanelFocus }`
