# M1a Design — Detail Pane and Panel Focus

**Date:** 2026-03-20
**Scope:** M1a — Split layout, file list, hunk diff view, panel focus system
**Status:** Draft
**Depends on:** M0 (complete)

---

## 1. Overview

M1a adds the detail pane — a lazygit-style right panel that always shows the file list for the selected change. The layout splits into 1/3 graph + 2/3 detail. A panel focus system lets the user navigate the file list and drill into diff hunks. File data is loaded eagerly with the graph (one `jj` call). Hunk diffs are loaded lazily on demand.

---

## 2. Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| **Detail pane visibility** | Always visible, auto-follows graph cursor | Lazygit model — no explicit expand/collapse |
| **Layout split** | 1/3 graph, 2/3 detail | Matches lazygit; diffs need horizontal space |
| **File data loading** | Eager via `jj log --summary` | One command, no per-cursor-move backend call |
| **Hunk diff loading** | Lazy via `jj diff --git` | Only on explicit drill-down; user expects brief load |
| **Panel architecture** | Panel modules with shared flat state | Keeps Elm-style dispatch, organizes code by panel |
| **Diff display** | Replaces file list in detail pane | Lazygit model; `Esc` returns to file list |
| **Description in graph** | Removed from template output | M0 template had a description continuation line; M1a removes it because the description is now shown in the detail pane header and status bar. This avoids description lines being confused with `--summary` file lines during parsing. |

---

## 3. Core Layer Changes (`lajjzy-core`)

### 3.1 New Types

```rust
/// A file changed in a change (parsed from jj log --summary).
#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: String,
    pub status: FileStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    /// Rename: path contains `{old => new}` format from jj.
    Renamed,
}

/// A hunk from a file diff (parsed from jj diff --git).
#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
    Header,
}
```

**Note on `Renamed`:** jj 0.39.0 outputs `R {foo.txt => renamed_foo.txt}` in `--summary` output. The `FileChange.path` for renames contains the full `{old => new}` format. Display can show this as-is or split it for a cleaner presentation.

### 3.2 ChangeDetail Changes

`ChangeDetail` gains a `files: Vec<FileChange>` field. Populated eagerly during `load_graph()`.

### 3.3 RepoBackend Extended

```rust
pub trait RepoBackend: Send + Sync {
    fn load_graph(&self) -> Result<GraphData>;

    /// Compute diff hunks for a specific file in a change.
    /// Lazy — called only when user drills into a file.
    fn file_diff(&self, change_id: &str, path: &str) -> Result<Vec<DiffHunk>>;
}
```

**Data flow for `file_diff()` calls:** The detail panel obtains the change ID via `state.selected_change_id()` (existing `AppState` accessor that reads from `GraphLine.change_id` at the cursor position). The file path comes from the `FileChange` at `detail_cursor` in the selected change's file list (accessed via `state.selected_detail().files[detail_cursor]`).

### 3.4 JjCliBackend Changes

**`load_graph()` template change from M0:** The M0 template included a description continuation line (`++ "\n" ++ coalesce(description.first_line(), "(no description)")`). **This line is removed in M1a.** The description is now shown in the detail pane header and status bar, not in the graph output. This prevents description text from being confused with `--summary` file lines during parsing (e.g., a description starting with "A " or "M " would false-positive as a file change).

The graph view now shows only the header line per change (change ID + author + timestamp). This is a deliberate UX trade-off: less information density in the graph, but the description is always visible in the detail pane.

Updated template:
```
jj log --summary --color=never -T '
  change_id.short() ++ " " ++ coalesce(author.name(), "anonymous")
  ++ " " ++ committer.timestamp().ago()
  ++ "\x1f" ++ change_id.short()
  ++ "\x1e" ++ commit_id.short()
  ++ "\x1e" ++ coalesce(author.name(), "")
  ++ "\x1e" ++ coalesce(author.email(), "")
  ++ "\x1e" ++ committer.timestamp().ago()
  ++ "\x1e" ++ coalesce(description.first_line(), "")
  ++ "\x1e" ++ bookmarks
  ++ "\x1e" ++ empty
  ++ "\x1e" ++ conflict
  ++ "\x1e" ++ if(self.current_working_copy(), "@", "")
  ++ "\n"
'
```

The trailing `\n` separates the metadata line from the first `--summary` file line.

**Graph glyph stripping:** File summary lines arrive with graph prefix glyphs (e.g., `│  A bar.txt`). The parser strips leading graph characters (`│`, `○`, `◆`, `@`, `◉`, `├`, `─`, and spaces) then checks if the remainder matches `^[AMDR] ` or `^R {` (for renames). Matched lines are parsed as `FileChange` entries and associated with the most recent node. Non-matching continuation lines remain as display-only graph lines.

**`file_diff()`:** New method. Runs `jj diff -r <change_id> --git --color=never <path>`. Parses standard git-format diff output:
- Lines starting with `diff --git`, `index`, `---`, `+++` become `DiffLineKind::Header`
- `@@` lines become hunk headers (start a new `DiffHunk`)
- `+` lines become `Added`, `-` lines become `Removed`, ` ` lines become `Context`

**Note:** For renamed files, `path` should be the new filename (after the `=>`). The `jj diff` command uses the current filename.

---

## 4. TUI Layer Changes (`lajjzy-tui`)

### 4.1 AppState Additions

```rust
pub struct AppState {
    // existing
    pub graph: GraphData,
    cursor: usize,                                  // private (M0)
    pub should_quit: bool,
    pub error: Option<String>,
    // new
    pub focus: PanelFocus,
    detail_cursor: usize,                           // private, like cursor
    pub detail_mode: DetailMode,
    pub diff_scroll: usize,
    pub diff_data: Option<(String, Vec<DiffHunk>)>, // (file_path, hunks)
}
```

`detail_cursor` is private with a getter (`pub fn detail_cursor(&self) -> usize`), following the same pattern as `cursor`.

`diff_data` stores the file path alongside the hunks to prevent stale data from being rendered for the wrong file.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelFocus {
    Graph,
    Detail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailMode {
    FileList,
    DiffView,
}
```

**Auto-follow:** When the graph cursor moves (any `MoveUp`, `MoveDown`, `JumpToTop`, `JumpToBottom`, `JumpToWorkingCopy`, `Refresh`), `detail_cursor` resets to 0, `detail_mode` resets to `FileList`, and `diff_data` is cleared.

### 4.2 New Actions

```rust
pub enum Action {
    // existing
    MoveUp, MoveDown, Quit, Refresh, JumpToTop, JumpToBottom,
    // new
    TabFocus,
    BackTabFocus,
    DetailMoveUp,
    DetailMoveDown,
    DetailEnter,
    DetailBack,
    DiffScrollUp,
    DiffScrollDown,
    DiffNextHunk,
    DiffPrevHunk,
    JumpToWorkingCopy,
}
```

**Note:** `TabFocus` and `BackTabFocus` are identical with only two panels (both toggle `Graph <-> Detail`). They are distinct actions for forward-compatibility with M1b, which adds more panels.

### 4.3 Input Routing

`map_event` takes focus and detail mode as context:

```rust
pub fn map_event(event: KeyEvent, focus: PanelFocus, detail_mode: DetailMode) -> Option<Action>
```

**Breaking change from M0:** This adds two parameters. The call site in `main.rs` must be updated to pass `state.focus` and `state.detail_mode`.

Key routing:
- `Tab` / `Shift-Tab` → `TabFocus` / `BackTabFocus` (always)
- `q` / `Ctrl-C` → `Quit` (always)
- `R` → `Refresh` (always)
- `@` → `JumpToWorkingCopy` (always)
- When `focus == Graph`: `j`/`k` → `MoveDown`/`MoveUp`, `g`/`G` → `JumpToTop`/`JumpToBottom`
- When `focus == Detail` and `detail_mode == FileList`: `j`/`k` → `DetailMoveDown`/`DetailMoveUp`, `Enter` → `DetailEnter`, `Esc` → `DetailBack`
- When `focus == Detail` and `detail_mode == DiffView`: `j`/`k` → `DiffScrollDown`/`DiffScrollUp`, `n`/`N` → `DiffNextHunk`/`DiffPrevHunk`, `Esc` → `DetailBack`

### 4.4 Panel Modules

New directory `panels/`:

Each panel module has the signature:
```rust
pub fn handle(state: &mut AppState, action: Action, backend: &dyn RepoBackend)
```

**`panels/graph.rs`:** Handles `MoveUp`, `MoveDown`, `JumpToTop`, `JumpToBottom`, `JumpToWorkingCopy`. Resets detail state on cursor move (sets `detail_cursor = 0`, `detail_mode = FileList`, clears `diff_data`). Renders the graph panel with focus border.

`JumpToWorkingCopy`: Sets cursor to `graph.working_copy_index`. No-op if `working_copy_index` is `None`.

**`panels/detail.rs`:** Handles `DetailMoveUp`, `DetailMoveDown`, `DetailEnter`, `DetailBack`, `DiffScrollUp`, `DiffScrollDown`, `DiffNextHunk`, `DiffPrevHunk`. Renders file list or diff view depending on `detail_mode`, with focus border.

`DetailBack` behavior depends on `detail_mode`:
- `DiffView` → sets `detail_mode = FileList`, clears `diff_data`
- `FileList` → sets `state.focus = PanelFocus::Graph` (the detail panel module is allowed to modify focus state)

`DetailEnter` calls `backend.file_diff(state.selected_change_id(), path)` and stores the result in `state.diff_data`. On error, sets `state.error` with context.

`DetailMoveUp` / `DetailMoveDown`: No-op at boundaries (same as graph cursor). Clamped to `0..files.len()`.

`DiffNextHunk` / `DiffPrevHunk`: Sets `diff_scroll` to the line offset of the next/previous `DiffHunk.header`, positioning the hunk header at the top of the viewport. No-op if already at the first/last hunk.

Main `dispatch` delegates:
```rust
match action {
    // Global
    Action::TabFocus | Action::BackTabFocus => { /* toggle focus */ }
    Action::Quit => { state.should_quit = true; }
    Action::Refresh => { /* existing refresh logic + reset detail */ }
    // Graph panel
    Action::MoveUp | Action::MoveDown | Action::JumpToTop
    | Action::JumpToBottom | Action::JumpToWorkingCopy => {
        panels::graph::handle(state, action, backend);
    }
    // Detail panel
    Action::DetailMoveUp | Action::DetailMoveDown | Action::DetailEnter
    | Action::DetailBack | Action::DiffScrollUp | Action::DiffScrollDown
    | Action::DiffNextHunk | Action::DiffPrevHunk => {
        panels::detail::handle(state, action, backend);
    }
}
```

### 4.5 Rendering

```rust
pub fn render(frame: &mut Frame, state: &AppState) {
    let outer = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(2),
    ]).split(frame.area());

    let main = Layout::horizontal([
        Constraint::Ratio(1, 3),
        Constraint::Ratio(2, 3),
    ]).split(outer[0]);

    panels::graph::render(frame, state, main[0]);
    panels::detail::render(frame, state, main[1]);

    // status bar unchanged
    let change_id = state.selected_change_id();
    let detail = state.selected_detail();
    let error = state.error.as_deref();
    let status_widget = StatusBarWidget::new(change_id, detail, error);
    frame.render_widget(status_widget, outer[1]);
}
```

Each panel renders a `Block` with a border whose color indicates focus (bright for focused, dim for unfocused). Panel title shows context info (e.g., "Files — mpvponzr add bar" or "Diff — foo.txt").

### 4.6 New Widgets

**`widgets/file_list.rs` — `FileListWidget`:**
- Takes `&[FileChange]`, cursor index, whether focused
- Renders one line per file: status letter (colored by status) + path
- Status colors: green=Added, yellow=Modified, red=Deleted, cyan=Renamed
- Cursor line highlighted with reverse video (same pattern as graph)

**`widgets/diff_view.rs` — `DiffViewWidget`:**
- Takes `&[DiffHunk]`, scroll offset
- Renders diff lines with colors: green for added, red for removed, default for context, dim for headers
- Scrollable via offset (lines above offset are not rendered)

---

## 5. Keybindings (M1a additions)

| Key | Context | Action |
|-----|---------|--------|
| `Tab` | Global | Cycle focus: Graph → Detail → Graph |
| `Shift-Tab` | Global | Cycle focus reverse |
| `@` | Global | Jump graph cursor to working-copy change |
| `j` / `k` | Detail (file list) | Move file cursor |
| `Enter` | Detail (file list) | Open diff view for selected file |
| `Esc` | Detail (diff view) | Return to file list |
| `Esc` | Detail (file list) | Return focus to graph |
| `j` / `k` | Detail (diff view) | Scroll diff |
| `n` / `N` | Detail (diff view) | Jump to next / previous hunk |

---

## 6. Testing Strategy

### `lajjzy-core` tests

- **Parser:** `parse_graph_output` with `--summary` output — verify `FileChange` entries, status mapping (A/M/D/R), graph glyph stripping
- **Parser:** changes with no files produce empty `files` vec
- **Parser:** description text starting with "A " is not mistaken for a file change (no false positives — verified by removal of description continuation line from template)
- **Diff parser:** git-format diff → `Vec<DiffHunk>` — single hunk, multi-hunk, new file, deleted file
- **Integration:** `load_graph()` on real repo with file changes — verify files present
- **Integration:** `file_diff()` on real repo — verify hunks returned

### `lajjzy-tui` tests

- **State transitions:** `TabFocus` cycles focus, graph cursor move resets detail, `DetailEnter` loads diff, `DetailBack` from DiffView returns to FileList, `DetailBack` from FileList returns focus to Graph
- **Bounds:** `DetailMoveDown` at end of file list is no-op, `DetailMoveUp` at 0 is no-op
- **Input routing:** same key (`j`) produces different actions per focus/mode
- **Widget snapshots:** `FileListWidget` and `DiffViewWidget` render correctly with `TestBackend`

---

## 7. Architectural Constraints

All M0 constraints remain active. Additional notes for M1a:

- **C1 (Facade boundary):** `file_diff()` added to `RepoBackend` trait. The TUI calls it through the trait, never shells out directly.
- **C2 (No panics):** `file_diff()` returns `Result`. Errors stored in `state.error` and displayed in status bar.
- **C3 (Dispatch impurity):** `dispatch` still takes `&dyn RepoBackend` for M1a. `DetailEnter` calls `backend.file_diff()` through the panel module. Purity deferred to M2.
- **C6 (Error messages):** Failed `file_diff()` calls show "Failed to load diff for <path>: <cause>" in the status bar.

---

## 8. Out of Scope for M1a

- Op log viewer — M1b
- Bookmark list — M1b
- Fuzzy-find — M1b
- Help overlay — M1b
- Any mutations — M2
- Async runtime — M2/M3
- Custom graph renderer — M3
