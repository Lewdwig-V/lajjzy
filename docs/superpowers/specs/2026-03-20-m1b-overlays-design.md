# M1b Design — Overlays, Pickers, and Graph Visual Improvements

**Date:** 2026-03-20
**Scope:** M1b — Op log overlay, bookmark picker, fuzzy-find, help overlay, graph compaction, graph colorization
**Status:** Draft
**Depends on:** M1a (complete)

---

## 1. Overview

M1b adds four modal overlays (op log, bookmark picker, fuzzy-find, help), compacts the graph pane by hiding file summary lines, and colorizes the graph using structured metadata. All overlays follow a single `Option<Modal>` pattern in `AppState`. The fuzzy-find uses `nucleo-matcher` for real fuzzy matching.

---

## 2. Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| **Modal system** | `Option<Modal>` enum on AppState | Simple, flat, follows lazygit. Stack when needed (YAGNI). |
| **Bookmark data** | From existing GraphData | No new backend call — bookmarks already in ChangeDetail |
| **Fuzzy matching** | `nucleo-matcher` crate | Real fuzzy match (typos, out-of-order). Lightweight, powers Helix. |
| **Graph files** | Hidden from graph pane | Lazygit model — compact commit list, files in detail pane only |
| **Graph color** | Self-rendered from metadata | Full control, consistent theming, no jj color dependency |

---

## 3. Modal System

### 3.1 AppState Addition

```rust
pub struct AppState {
    // ... existing fields ...
    pub modal: Option<Modal>,
}
```

### 3.2 Modal Enum

```rust
pub enum Modal {
    OpLog {
        entries: Vec<OpLogEntry>,
        cursor: usize,
        scroll: usize,
    },
    BookmarkPicker {
        bookmarks: Vec<(String, String)>,  // (bookmark_name, change_id)
        cursor: usize,
    },
    FuzzyFind {
        query: String,
        matches: Vec<usize>,  // indices into graph node_indices
        cursor: usize,
    },
    Help {
        context: HelpContext,
        scroll: usize,
    },
}

pub enum HelpContext {
    Graph,
    DetailFileList,
    DetailDiffView,
}
```

**`HelpContext` mapping** from existing state:
- `PanelFocus::Graph` → `HelpContext::Graph`
- `PanelFocus::Detail` + `DetailMode::FileList` → `HelpContext::DetailFileList`
- `PanelFocus::Detail` + `DetailMode::DiffView` → `HelpContext::DetailDiffView`

### 3.3 Input Routing

When `state.modal.is_some()`, input routing is handled by a separate function:

```rust
pub fn map_modal_event(event: KeyEvent, modal: &Modal) -> Option<Action>
```

This is called **before** `map_event` when a modal is active. If it returns `Some(action)`, that action is dispatched and `map_event` is not called. If it returns `None`, the key is ignored (modals fully capture input).

**Key routing in modals:**

For non-text-input modals (OpLog, BookmarkPicker, Help):
- `Esc` → `ModalDismiss`
- `q` → `ModalDismiss` (overrides the global `q → Quit` — modals capture all input)
- `j`/`k` or `Up`/`Down` → `ModalMoveUp`/`ModalMoveDown`
- `Enter` → `ModalEnter` (bookmark picker, no-op for op log/help)
- The trigger key (`O` for op log, `?` for help) → `ModalDismiss` (toggle behavior)

For fuzzy-find (text input modal):
- `Esc` → `ModalDismiss` (only way to dismiss — `q` is text input)
- `Up`/`Down` or `Ctrl-N`/`Ctrl-P` → `ModalMoveUp`/`ModalMoveDown`
- `Enter` → `ModalEnter`
- `Backspace` → `FuzzyBackspace`
- Any printable character → `FuzzyInput(char)` (including `q`, `j`, `k`, `/`)

**Toggle pattern:** Op log (`O`) and help (`?`) can be dismissed by pressing their trigger key again. Bookmark picker (`b`) and fuzzy-find (`/`) cannot — `b` is unmapped in the picker, and `/` is text input in fuzzy-find.

All modal selection actions (bookmark jump, fuzzy-find jump) are handled in `dispatch()`, which has direct access to `AppState.cursor`. The modal handler functions live in a modal module but return `Action`s that `dispatch` processes.

### 3.4 Rendering

When a modal is active, the normal panels render underneath (dimmed — apply a dim modifier to the background), and the modal renders on top as a centered overlay.

Overlay sizes:
- Op log: full area (takes over the main content, status bar still visible)
- Bookmark picker: centered, ~60% width, ~80% height
- Fuzzy-find: centered, ~60% width, ~80% height
- Help: centered, ~50% width, ~60% height

---

## 4. Op Log Overlay

### 4.1 RepoBackend Extension

```rust
pub trait RepoBackend: Send + Sync {
    fn load_graph(&self) -> Result<GraphData>;
    fn file_diff(&self, change_id: &str, path: &str) -> Result<Vec<DiffHunk>>;
    fn op_log(&self) -> Result<Vec<OpLogEntry>>;  // NEW
}
```

**Migration note:** `MockBackend` and `FailingBackend` in `app.rs` tests must implement `op_log()`. MockBackend returns `Ok(vec![])`, FailingBackend returns `Err`.

### 4.2 New Type

```rust
pub struct OpLogEntry {
    pub id: String,
    pub description: String,
    pub timestamp: String,
}
```

### 4.3 JjCliBackend Implementation

Shells out to `jj op log --no-graph --color=never` with a template using `\x1F`/`\x1E` delimiters (same approach as `load_graph`). Template outputs: operation ID (short), timestamp, description.

### 4.4 Behavior

- `O` calls `backend.op_log()`, opens `Modal::OpLog` with entries
- `j`/`k` or `Up`/`Down` navigates, scrollable
- `Esc`, `O`, or `q` dismisses
- Read-only — undo/redo is M2
- Error from `op_log()` sets `state.error`, does not open modal

### 4.5 Rendering

Full-area overlay with border and title "Operation Log". Each entry: `{id}  {timestamp}  {description}`. Cursor highlight with reverse video.

---

## 5. Bookmark Picker

### 5.1 Data Source

No new backend method. Collects bookmark data by iterating the loaded graph:

```
for idx in graph.node_indices():
    change_id = graph.lines[idx].change_id  // from GraphLine, NOT ChangeDetail
    detail = graph.details[&change_id]       // ChangeDetail has no change_id field
    for bookmark in detail.bookmarks:
        collect (bookmark, change_id)
```

**Note:** `ChangeDetail` deliberately has no `change_id` field (removed in M0 review). The change ID is the HashMap key in `GraphData.details` and is also stored in `GraphLine.change_id`. Both the bookmark picker and fuzzy-find use `GraphLine.change_id` to get the ID, then look up the detail.

### 5.2 Behavior

- `b` scans graph data, opens `Modal::BookmarkPicker`
- `j`/`k` or `Up`/`Down` navigates
- `Enter` dismisses and jumps graph cursor to the selected bookmark's change
- `Esc` or `q` dismisses without moving
- Empty bookmark list: show "(no bookmarks)" message, `Esc` to dismiss

### 5.3 Rendering

Centered overlay with border and title "Bookmarks". Each entry: `{bookmark_name}  {change_description}` (description truncated to fit). Cursor highlight.

---

## 6. Fuzzy-Find

### 6.1 Dependency

Add `nucleo-matcher` to `lajjzy-tui/Cargo.toml`.

### 6.2 Data Source

Searches across all changes in the loaded graph. For each node index in `graph.node_indices()`, build searchable text by looking up `graph.lines[idx].change_id` and then `graph.details[&change_id]` to get `"{change_id} {author} {description}"`. No backend call.

`FuzzyFind.matches` stores `Vec<usize>` where each entry is a graph line index (from `node_indices()`), not an index into a separate list.

### 6.3 Behavior

- `/` opens `Modal::FuzzyFind` with empty query, all changes listed
- Typing filters results in real-time using nucleo's fuzzy matcher
- Results sorted by match score (best match first)
- `Up`/`Down` (or `Ctrl-N`/`Ctrl-P`) navigates filtered results
- `Enter` dismisses and jumps graph cursor to selected change
- `Esc` dismisses without moving (`q` is text input, not dismiss)
- `Backspace` deletes from query

**Text input vs control keys:** Since this modal accepts text, `j`, `k`, `q`, and `/` are text input, not navigation/quit/toggle. Navigation uses `Up`/`Down` arrows or `Ctrl-N`/`Ctrl-P`. Only `Esc` dismisses.

### 6.4 Rendering

Centered overlay with:
- Title bar: "Find Change"
- Top line: text input showing `/ {query}|` (pipe as cursor indicator)
- Below: filtered results list with cursor highlight
- Each result: `{change_id}  {author}  {description}`

---

## 7. Help Overlay

### 7.1 Data Source

Static keybinding data. Context-sensitive: captures `focus` and `detail_mode` when `?` is pressed.

### 7.2 Content

**Graph panel:**
```
j/k       Move between changes
g/G       Jump to top/bottom
@         Jump to working copy
Tab       Switch to detail pane
R         Refresh
/         Fuzzy-find
b         Bookmarks
O         Operation log
?         This help
q         Quit
```

**Detail pane (file list):**
```
j/k       Move between files
Enter     Open diff view
Esc       Return to graph
Tab       Switch to graph pane
```

**Detail pane (diff view):**
```
j/k       Scroll diff
n/N       Next/previous hunk
Esc       Return to file list
```

### 7.3 Behavior

- `?` opens overlay with keybindings for current context
- Scrollable if content exceeds height
- `Esc`, `?`, or `q` dismisses

### 7.4 Rendering

Centered overlay (~50% width, ~60% height) with border and title "Help — {context}". Two-column layout: key left, description right.

---

## 8. Graph Compaction

### 8.1 Parser Change

In `parse_graph_output()`, when a continuation line matches `parse_file_line()`, it is parsed into `ChangeDetail.files` but **not** added to `GraphData.lines`. This removes file summary lines from the graph display.

**Before (M1a):**
```
@  mpvponzr Lewdwig 1m ago
│  A bar.txt                    ← these lines
│  M foo.txt                    ← removed from graph
○  mrvmvrsz Lewdwig 2m ago
```

**After (M1b):**
```
@  mpvponzr Lewdwig 1m ago
○  mrvmvrsz Lewdwig 2m ago
```

Files are still in `ChangeDetail.files` and shown in the detail pane.

### 8.2 Impact

- `GraphData.lines` becomes shorter — fewer connector lines
- `node_indices()` cache stays correct (computed from the new shorter `lines`)
- `GraphWidget` block highlighting still works — `block_end()` scans for next `change_id.is_some()` line, and file lines (which had `change_id: None`) are simply gone

**Tests requiring update:**
- `cli.rs::parse_graph_output_with_file_summary` — currently asserts file lines appear in `GraphData.lines` and counts them in `lines.len()`. After compaction, lines.len() decreases and file lines are absent from `lines` (still in `ChangeDetail.files`).
- `cli.rs::parse_graph_output_rename` — same issue: the `R {foo.txt => bar.txt}` line will not be in `lines`.

---

## 9. Graph Colorization

### 9.1 Rendering Change

When rendering a node line in `panels/graph.rs`, instead of rendering `GraphLine.raw` as plain text, construct colored `Span`s from the structured metadata in `ChangeDetail`.

### 9.2 Color Scheme

| Element | Color |
|---------|-------|
| Change ID (short) | Yellow |
| Author | Blue |
| Timestamp | Cyan |
| Bookmarks | Magenta, in brackets |
| Working copy indicator (`@`) | Green, bold |
| Connector glyphs (`│`, `○`, `◆`) | Dark gray |

### 9.3 Implementation

**Node line format:** `{glyph_prefix}  {change_id}  {author}  {timestamp}  {bookmarks}`

The description is NOT shown in the compacted graph node line — it is shown in the detail pane header and status bar. This matches lazygit's compact commit list.

**Glyph prefix extraction:** Store the graph glyph prefix as a separate field on `GraphLine` during parsing:

```rust
pub struct GraphLine {
    pub raw: String,
    pub glyph_prefix: String,     // NEW: e.g., "@  ", "○  ", "│  "
    pub change_id: Option<String>,
}
```

The parser extracts the glyph prefix by finding the position of the `\x1F` delimiter (for node lines) and taking everything before the first semantic content. For connector lines, `glyph_prefix` is the entire `raw` string.

This avoids fragile string scanning at render time and keeps the extraction in one place (the parser).

**Rendering logic in `panels/graph.rs`:**
- For node lines: render `glyph_prefix` in dark gray, then colored spans for change ID/author/timestamp/bookmarks from `ChangeDetail`
- For connector lines: render `raw` in dark gray

### 9.4 Working Copy Highlight

The working copy change (`@`) gets special treatment: the glyph prefix is rendered in green bold instead of dark gray. Identified via `GraphData.working_copy_index`.

---

## 10. New Actions

```rust
// Modal triggers (global, when no modal is active)
ToggleOpLog,       // O
OpenBookmarks,     // b
OpenFuzzyFind,     // /
OpenHelp,          // ?

// Modal navigation (when modal is active)
ModalDismiss,      // Esc (or q in non-text-input modals, or trigger key for toggle modals)
ModalMoveUp,       // k or Up (non-text-input), Up or Ctrl-P (text-input)
ModalMoveDown,     // j or Down (non-text-input), Down or Ctrl-N (text-input)
ModalEnter,        // Enter — select in bookmark/fuzzy-find

// Fuzzy-find specific
FuzzyInput(char),  // printable character
FuzzyBackspace,    // Backspace
```

**Note:** `FuzzyInput(char)` means `Action` can no longer derive `Copy` (it still can — `char` is `Copy`). It does mean `Action` needs custom handling in tests since `FuzzyInput('a') != FuzzyInput('b')`.

---

## 11. Keybindings (M1b additions)

| Key | Context | Action |
|-----|---------|--------|
| `O` | Global (no modal) | Toggle op log overlay |
| `b` | Global (no modal) | Open bookmark picker |
| `/` | Global (no modal) | Open fuzzy-find |
| `?` | Global (no modal) | Open help overlay |
| `Esc` | Any modal | Dismiss modal |
| `q` | Non-text-input modal | Dismiss modal |
| `j`/`k` | Non-text-input modal | Navigate up/down |
| `Up`/`Down` | Any modal | Navigate up/down |
| `Ctrl-N`/`Ctrl-P` | Fuzzy-find | Navigate results |
| `Enter` | Bookmark/Fuzzy-find | Select and dismiss |
| Printable chars | Fuzzy-find | Add to query |
| `Backspace` | Fuzzy-find | Remove from query |
| `O` | Op log modal | Dismiss (toggle) |
| `?` | Help modal | Dismiss (toggle) |

---

## 12. Testing Strategy

### `lajjzy-core` tests
- Op log parser: structured output → `Vec<OpLogEntry>`
- Integration: `op_log()` on real repo
- Graph compaction: file lines not in `GraphData.lines`, still in `ChangeDetail.files`

### `lajjzy-tui` tests
- **MockBackend/FailingBackend** updated with `op_log()` impl
- Modal open/dismiss lifecycle for each modal type
- `q` in non-text-input modal dismisses modal, not app
- `q` in fuzzy-find is text input
- Input blocked from panels while modal active
- Bookmark picker: collects from graph data, Enter jumps cursor, empty list handled
- Fuzzy-find: empty query shows all, typing filters, Enter selects, Backspace works
- Help: content varies by context (Graph vs DetailFileList vs DetailDiffView)
- Graph colorization: node lines render with colored spans (buffer style checks)
- Graph compaction: `GraphWidget` renders compact graph correctly

---

## 13. Architectural Constraints

All M0/M1a constraints remain active.

- **C1 (Facade):** `op_log()` added to `RepoBackend`. Fuzzy-find and bookmark picker use existing graph data, no new backend calls.
- **C2 (No panics):** `op_log()` returns `Result`. Modal open failures set `state.error`.
- **C3 (Dispatch impurity):** `op_log()` call in dispatch is another pragmatic impurity. Noted in CLAUDE.md.

---

## 14. Dependencies

| Crate | Added to | Purpose |
|-------|----------|---------|
| `nucleo-matcher` | `lajjzy-tui` | Fuzzy matching for change search |

---

## 15. Out of Scope

- Undo/redo from op log — M2
- Forge integration in bookmark list — M5
- Configurable keybindings — M6
- Theming/custom colors — M6
