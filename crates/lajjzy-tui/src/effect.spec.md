---
managed_file: crates/lajjzy-tui/src/effect.rs
version: 1
test_policy: "No tests — enum definition only"
---

# Effect enum

## Purpose

Effects emitted by `dispatch()`, executed in `lajjzy-cli`. Defined in `lajjzy-tui`
so dispatch can return them, but never executed here. Each variant maps to a
backend operation or terminal suspension.

## Dependencies

- `lajjzy_core::types::FileHunkSelection`
- `crate::action::HunkPickerOp`

## Enum

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Effect { ... }
```

### Read-only effects
- `LoadGraph { revset: Option<String> }` — doc: revset reserved for omnibar, currently always None for default
- `LoadOpLog`
- `LoadFileDiff { change_id: String, path: String }`

### Mutation effects
- `Describe { change_id: String, text: String }`
- `New { after: String }`
- `Edit { change_id: String }`
- `Abandon { change_id: String }`
- `LoadChangeDiff { change_id: String, operation: HunkPickerOp }`
- `Split { change_id: String, selections: Vec<FileHunkSelection> }`
- `SquashPartial { change_id: String, selections: Vec<FileHunkSelection> }`
- `Undo`
- `Redo`
- `BookmarkSet { change_id: String, name: String }`
- `BookmarkDelete { name: String }`
- `GitPush { bookmark: String }`
- `GitFetch`
- `RebaseSingle { source: String, destination: String }`
- `RebaseWithDescendants { source: String, destination: String }`
- `EvalRevset { query: String }` — doc: executor calls `load_graph(Some(&query))`

### M7 mutation effects
- `Absorb { change_id: String }`
- `Duplicate { change_id: String }`
- `Revert { change_id: String }`

### Conflict handling effects
- `LoadConflictData { change_id: String, path: String }`
- `ResolveFile { change_id: String, path: String, content: Vec<u8> }`
- `LaunchMergeTool { change_id: String, path: String }`

### Forge effects
- `FetchForgeStatus`
- `OpenOrCreatePr { bookmark: String }` — doc: executor handles routing (open browser vs suspend for `gh pr create`)

### Non-repo effects
- `SuspendForEditor { change_id: String, initial_text: String }`
