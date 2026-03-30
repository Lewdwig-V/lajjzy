---
managed-file: crates/lajjzy-core/src/backend.rs
version: 1
test_policy: "No tests — trait definition only, tested via implementors"
---

# RepoBackend trait

## Purpose

Define the facade trait through which all jj repository operations are accessed.
The TUI layer (`lajjzy-tui`) never shells out to jj or imports jj-lib directly;
everything routes through a `RepoBackend` implementor provided at startup.

## Dependencies

- `crate::types::{ConflictData, FileDiff, GraphData}` — return types
- `crate::types::FileHunkSelection` — parameter type for split/squash
- `anyhow::Result` — all methods are fallible

## Contract

```
pub trait RepoBackend: Send + Sync
```

All methods return `Result<T>`. No method may panic on repo errors.

### Graph loading

| Method | Signature | Notes |
|--------|-----------|-------|
| `load_graph` | `(&self, revset: Option<&str>) -> Result<GraphData>` | Bulk load; `revset` maps to `-r` flag |

### Per-file lazy loading

| Method | Signature | Notes |
|--------|-----------|-------|
| `file_diff` | `(&self, change_id: &str, path: &str) -> Result<Vec<DiffHunk>>` | Called on drill-in only |
| `change_diff` | `(&self, change_id: &str) -> Result<Vec<FileDiff>>` | All files in a change |
| `conflict_sides` | `(&self, change_id: &str, path: &str) -> Result<ConflictData>` | Err for n-way or binary |

### Mutations

| Method | Signature |
|--------|-----------|
| `describe` | `(&self, change_id: &str, text: &str) -> Result<String>` |
| `new_change` | `(&self, after: &str) -> Result<String>` |
| `edit_change` | `(&self, change_id: &str) -> Result<String>` |
| `abandon` | `(&self, change_id: &str) -> Result<String>` |

### Undo / Redo

| Method | Signature | Notes |
|--------|-----------|-------|
| `undo` | `(&self) -> Result<String>` | `jj op restore @-` |
| `redo` | `(&self) -> Result<String>` | `jj op revert @` |

### Bookmarks

| Method | Signature |
|--------|-----------|
| `bookmark_set` | `(&self, change_id: &str, name: &str) -> Result<String>` |
| `bookmark_delete` | `(&self, name: &str) -> Result<String>` |

### Git operations

| Method | Signature |
|--------|-----------|
| `git_push` | `(&self, bookmark: &str) -> Result<String>` |
| `git_fetch` | `(&self) -> Result<String>` |

### Rebasing

| Method | Signature | Notes |
|--------|-----------|-------|
| `rebase_single` | `(&self, source: &str, destination: &str) -> Result<String>` | Reparents descendants |
| `rebase_with_descendants` | `(&self, source: &str, destination: &str) -> Result<String>` | Moves subtree |

### Partial operations (split / squash)

| Method | Signature | Notes |
|--------|-----------|-------|
| `split` | `(&self, change_id: &str, selections: &[FileHunkSelection]) -> Result<String>` | Selected hunks → child; rest stays |
| `squash_partial` | `(&self, change_id: &str, selections: &[FileHunkSelection]) -> Result<String>` | Selected hunks → parent; uses `-u` |

### Other mutations

| Method | Signature | Notes |
|--------|-----------|-------|
| `absorb` | `(&self, change_id: &str) -> Result<String>` | Attribute hunks to ancestors |
| `duplicate` | `(&self, change_id: &str) -> Result<String>` | Identical sibling on same parents |
| `revert` | `(&self, change_id: &str) -> Result<String>` | Inverse-apply as child of `@` |

### Conflict resolution

| Method | Signature | Notes |
|--------|-----------|-------|
| `resolve_file` | `(&self, change_id: &str, path: &str, content: Vec<u8>) -> Result<String>` | Change must be `@` (enforced by dispatch) |

## Doc-comment contract

The trait-level doc comment must state:
1. Implementations must return a `GraphData` where every `GraphLine` with a `change_id` has a corresponding entry in `details`
2. `working_copy_index` (if `Some`) points to a node line

## Style

- Each method has a `///` doc comment explaining its purpose
- `split` and `squash_partial` doc comments explain the `selections` parameter semantics (fully-selected = all hunks selected)
- No default method implementations
