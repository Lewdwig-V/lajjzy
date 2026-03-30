---
managed-file: crates/lajjzy-tui/src/modal.rs
version: 1
test_policy: "No tests — enum definitions only"
---

# Modal enum and HelpContext

## Purpose

Define the modal dialog state variants. Each variant carries its own cursor,
scroll, and content state. Displayed as overlays in the render pass.

## Dependencies

- `crate::action::CompletionItem`
- `lajjzy_core::types::OpLogEntry`
- `tui_textarea::TextArea`

## Types

### Modal

```rust
#[derive(Debug, Clone)]
pub enum Modal {
    OpLog { entries: Vec<OpLogEntry>, cursor: usize, scroll: usize },
    BookmarkPicker { bookmarks: Vec<(String, String)>, cursor: usize },
    Omnibar { query: String, matches: Vec<usize>, cursor: usize, completions: Vec<CompletionItem>, completion_cursor: usize },
    Help { context: HelpContext, scroll: usize },
    Describe { change_id: String, editor: Box<TextArea<'static>> },
    BookmarkInput { change_id: String, input: String, completions: Vec<String>, cursor: usize },
}
```

Note: derives `Debug, Clone` only (no `PartialEq` — `TextArea` doesn't implement it).

- `BookmarkPicker.bookmarks`: tuples of `(bookmark_name, change_id)`
- `Omnibar.matches`: graph line indices from `node_indices`
- `Describe.editor`: boxed to keep enum size reasonable

### HelpContext

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpContext { Graph, DetailFileList, DetailDiffView, ConflictView }
```

**Method:** `line_count(self) -> usize` — returns the number of help lines for scroll bounds:
- `Graph` → 28
- `DetailFileList` → 4
- `DetailDiffView` → 3
- `ConflictView` → 7
