---
managed-file: crates/lajjzy-core/src/types.rs
version: 1
test_policy: "Write or extend tests for GraphData accessors and Display impls"
---

# Core domain types

## Purpose

Define the shared data types for jj repository state. These types cross the
crate boundary — `lajjzy-core` produces them, `lajjzy-tui` consumes them.
No TUI dependencies allowed.

## Dependencies

- `std::collections::HashMap` — for `GraphData.details`

## Types

### OpLogEntry

```
pub struct OpLogEntry { pub id, pub description, pub timestamp: String }
```

Derives: `Debug, Clone, PartialEq`

### GraphData

```
pub struct GraphData {
    pub lines: Vec<GraphLine>,
    pub details: HashMap<String, ChangeDetail>,
    pub working_copy_index: Option<usize>,
    cached_node_indices: Vec<usize>,  // private
    pub op_id: String,
}
```

Derives: `Debug, Clone, PartialEq`

**Constructor invariant:** `GraphData::new(lines, details, working_copy_index, op_id)` computes
`cached_node_indices` by collecting indices of all `lines` where `change_id.is_some()`.
This is the only way to construct a `GraphData` — the field is private.
All parameters are stored as-is — in particular, `working_copy_index` must be passed through,
not discarded or defaulted.

**Accessors:**
- `node_indices(&self) -> &[usize]` — returns the cached indices
- `detail_at(&self, index: usize) -> Option<&ChangeDetail>` — looks up `lines[index].change_id` then `details[id]`. Returns `None` for out-of-bounds, connector lines, or missing details.

### GraphLine

```
pub struct GraphLine {
    pub raw: String,
    pub change_id: Option<String>,
    pub glyph_prefix: String,
}
```

Derives: `Debug, Clone, PartialEq`

- `raw`: display string with graph glyphs, delimiter stripped
- `change_id`: `Some` for node lines (first line of a change block), `None` for connectors
- `glyph_prefix`: everything before first alphanumeric; for connectors, equals `raw`

### ChangeDetail

```
pub struct ChangeDetail {
    pub commit_id: String,
    pub author: String,
    pub email: String,
    pub timestamp: String,
    pub description: String,
    pub bookmarks: Vec<String>,
    pub is_empty: bool,
    pub conflict_count: usize,
    pub files: Vec<FileChange>,
    pub parents: Vec<String>,
}
```

Derives: `Debug, Clone, PartialEq`

### FileChange

```
pub struct FileChange { pub path: String, pub status: FileStatus }
```

Derives: `Debug, Clone, PartialEq`

### FileStatus

```
pub enum FileStatus { Added, Modified, Deleted, Renamed, Conflicted, Unknown(char) }
```

Derives: `Debug, Clone, Copy, PartialEq, Eq`

**Display impl:** `A`, `M`, `D`, `R`, `C`, or the raw char for `Unknown(c)`.

- `Renamed` doc comment: path contains `{old => new}` format from jj

### FileDiff

```
pub struct FileDiff { pub path: String, pub hunks: Vec<DiffHunk> }
```

Derives: `Debug, Clone, PartialEq`

### FileHunkSelection

```
pub struct FileHunkSelection {
    pub path: String,
    pub selected_hunks: Vec<usize>,
    pub total_hunks: usize,
}
```

Derives: `Debug, Clone, PartialEq`

Doc: used by backend `split`/`squash_partial`. Fully selected = `selected_hunks.len() == total_hunks`.

### ConflictData

```
pub struct ConflictData { pub regions: Vec<ConflictRegion> }
```

Derives: `Debug, Clone, PartialEq`

### ConflictRegion

```
pub enum ConflictRegion {
    Resolved(String),
    Conflict { base: String, left: String, right: String },
}
```

Derives: `Debug, Clone, PartialEq`

Doc: empty string for any side means that side deleted the region.

### DiffHunk

```
pub struct DiffHunk { pub header: String, pub lines: Vec<DiffLine> }
```

Derives: `Debug, Clone, PartialEq`

### DiffLine

```
pub struct DiffLine { pub kind: DiffLineKind, pub content: String }
```

Derives: `Debug, Clone, PartialEq`

### DiffLineKind

```
pub enum DiffLineKind { Context, Added, Removed, Header }
```

Derives: `Debug, Clone, Copy, PartialEq, Eq`

## Tests

Test `GraphData` via a `sample_graph()` helper building a 5-line graph (3 nodes, 2 connectors):

1. `node_indices_returns_only_change_nodes` — asserts `[0, 2, 4]`
2. `detail_at_returns_detail_for_node_line` — index 0 returns the correct author
3. `detail_at_returns_none_for_connector_line` — index 1 returns `None`
4. `detail_at_returns_none_for_out_of_bounds` — index 99 returns `None`

Test `FileStatus::Display` for all variants:

5. `file_status_display_added` — "A"
6. `file_status_display_modified` — "M"
7. `file_status_display_deleted` — "D"
8. `file_status_display_renamed` — "R"
9. `file_status_display_conflicted` — "C"
10. `file_status_display_unknown` — Unknown('X') → "X"

Test `GraphData::new` constructor pass-through:

11. `graph_constructor_stores_working_copy_index_some` — Some(2) preserved
12. `graph_constructor_stores_working_copy_index_none` — None preserved
