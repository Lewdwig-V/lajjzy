# M0 Skeleton Design — lajjzy

**Date:** 2026-03-20
**Scope:** M0 — Read-only graph view, cursor navigation, status bar
**Status:** Draft

> **Note on naming:** The README uses the working title "jjui" with crate names `jjui-*`. The project has been renamed to `lajjzy` with crates `lajjzy-*`. The README will be updated to match.

> **Note on data source:** The README's Principle 2 states "Library-first, no shelling out to jj CLI" and the README's M0 milestone says "jj-lib linked." M0 departs from this: we shell out to `jj` CLI behind a `RepoBackend` trait, deferring jj-lib linkage until the API stabilizes. The trait boundary ensures the swap is mechanical, not architectural.

---

## 1. Overview

M0 delivers the minimum viable lajjzy: a terminal UI that displays the jj change graph, lets the user navigate between changes with `j`/`k`, and shows metadata for the selected change in a status bar. No mutations, no detail pane, no async operations.

---

## 2. Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| **Data source** | Shell out to `jj` CLI with templates | Avoids jj-lib API instability; structured output via templates minimizes parsing; `RepoBackend` trait enables future jj-lib swap |
| **Crate structure** | Three crates from day one | Enforces facade boundary at compile time; `lajjzy-tui` can never import jj-lib directly |
| **Graph rendering** | Use jj's built-in graph output | Good enough for read-only M0; custom renderer deferred to M3 (stack mode) |
| **Event loop** | Simplified Elm-style (no effects) | `Action -> new state` is sufficient for M0; extends to `Action -> (state, effects)` in M2 |
| **Async runtime** | None for M0 | No background tasks; synchronous crossterm polling; tokio arrives in M2/M3 |
| **Selection UX** | Highlight + status bar metadata | Cursor highlights full change block; bottom bar shows full detail |

---

## 3. Crate Structure

```
lajjzy/
├── crates/
│   ├── lajjzy-core/        # RepoBackend trait + jj CLI implementation
│   │   └── src/
│   │       ├── lib.rs       # Re-exports
│   │       ├── backend.rs   # RepoBackend trait definition
│   │       ├── cli.rs       # JjCliBackend: shells out to jj with templates
│   │       └── types.rs     # ChangeInfo, GraphLine — shared data types
│   ├── lajjzy-tui/          # ratatui rendering, input handling, state
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── app.rs       # AppState, Action enum, dispatch (state machine)
│   │       ├── input.rs     # Keymap: crossterm events -> Actions
│   │       ├── render.rs    # Renders AppState -> ratatui Frame
│   │       └── widgets/
│   │           ├── mod.rs
│   │           ├── graph.rs     # Graph panel widget
│   │           └── status_bar.rs # Bottom status bar widget
│   └── lajjzy-cli/          # Binary entry point
│       └── src/
│           └── main.rs      # Terminal setup, event loop, panic handler
├── Cargo.toml               # Workspace root
└── README.md
```

### Dependencies (M0)

| Crate | `lajjzy-core` | `lajjzy-tui` | `lajjzy-cli` |
|-------|--------------|-------------|-------------|
| `lajjzy-core` | — | yes | — |
| `lajjzy-tui` | — | — | yes |
| `ratatui` | — | yes | — |
| `crossterm` | — | yes (event types, mapping) | yes (raw mode, alternate screen, event polling) |
| `serde` + `serde_json` | yes | — | — |
| `anyhow` | yes | yes | yes |

---

## 4. Core Layer (`lajjzy-core`)

### 4.1 RepoBackend Trait

```rust
pub trait RepoBackend {
    /// Load the full graph (jj log output) for display.
    /// Returns change blocks (groups of GraphLines) with details pre-loaded.
    fn load_graph(&self) -> Result<GraphData>;
}
```

`change_detail()` is removed from the trait. Details are loaded as part of `load_graph()` in a single `jj` invocation to avoid per-keypress subprocess spawning.

### 4.2 Data Types

```rust
/// Complete graph data returned by load_graph().
pub struct GraphData {
    /// All lines of graph output, grouped into change blocks.
    pub lines: Vec<GraphLine>,
    /// Details for each change, keyed by change ID.
    pub details: HashMap<String, ChangeDetail>,
    /// Index of the working-copy change's node line in `lines`.
    pub working_copy_index: Option<usize>,
}

/// One line of jj's graph output.
pub struct GraphLine {
    /// The display string (graph glyphs + text), delimiter stripped.
    pub raw: String,
    /// The change ID if this is a node line (first line of a change block).
    /// None for continuation/connector lines.
    pub change_id: Option<String>,
}

/// A change block is a node line (change_id: Some) followed by zero or more
/// continuation lines (change_id: None) until the next node line or end of graph.

/// Detailed info for the status bar.
pub struct ChangeDetail {
    pub change_id: String,
    pub commit_id: String,
    pub author: String,
    pub email: String,
    pub timestamp: String,
    pub description: String,
    pub bookmarks: Vec<String>,
    pub is_empty: bool,
    pub has_conflict: bool,
    pub is_working_copy: bool,
}
```

### 4.3 JjCliBackend

**Construction:** Takes a workspace path. Validates with `jj root` on creation; returns error if jj is not installed or path is not a jj workspace.

**`load_graph()`:** A single `jj log` invocation with `--color=never` that produces both graph output and structured detail data.

Template approach: The template emits a delimiter-tagged line once per change (not once per line). The template outputs the visual content followed by `\x1F` (ASCII unit separator) and then delimiter-separated fields for `ChangeDetail`:

```
jj log --color=never -T '
  change_id.short() ++ " " ++ author.name() ++ " " ++ committer.timestamp().ago()
  ++ "\n" ++ coalesce(description.first_line(), "(no description)")
  ++ "\x1F"
  ++ change_id ++ "\x1E"
  ++ commit_id ++ "\x1E"
  ++ author.name() ++ "\x1E"
  ++ author.email() ++ "\x1E"
  ++ committer.timestamp().ago() ++ "\x1E"
  ++ description ++ "\x1E"
  ++ bookmarks ++ "\x1E"
  ++ empty ++ "\x1E"
  ++ conflict ++ "\x1E"
  ++ working_copies
'
```

The exact template will be validated during implementation (jj template syntax may require adjustments). The key design points:
- `--color=never` avoids ANSI escape sequences; the TUI applies its own colors via ratatui styles.
- `\x1F` (unit separator) marks the boundary between display text and metadata.
- `\x1E` (record separator) delimits fields within the metadata, avoiding JSON escaping issues with descriptions containing quotes/newlines.
- The metadata is parsed into `ChangeDetail` structs and stored in a `HashMap` keyed by change ID.
- The working-copy change is identified by the `working_copies` field being non-empty.

**Parsing:** Each line of output is checked for `\x1F`. Lines containing it are node lines — the display portion before `\x1F` becomes `GraphLine.raw` with `change_id: Some(...)`, and the metadata after `\x1F` is parsed into `ChangeDetail`. Lines without `\x1F` are continuation/connector lines with `change_id: None`.

---

## 5. TUI Layer (`lajjzy-tui`)

### 5.1 AppState

```rust
pub struct AppState {
    /// All graph lines from jj log.
    pub graph: GraphData,
    /// Index into graph lines (cursor position, always on a node line).
    pub cursor: usize,
    /// Whether the app should quit.
    pub should_quit: bool,
    /// Error message to display in status bar (clears on next successful action).
    pub error: Option<String>,
}
```

**Initial cursor placement:** Set to `graph.working_copy_index` if available, otherwise the first node line. This matches user expectation — you start where you're working.

**Selected detail lookup:** `graph.details.get(change_id)` using the change ID from the current cursor's `GraphLine`. No subprocess call needed.

### 5.2 Actions

```rust
pub enum Action {
    MoveUp,
    MoveDown,
    Quit,
    Refresh,
    JumpToTop,
    JumpToBottom,
}
```

### 5.3 Dispatch

```rust
fn dispatch(state: &mut AppState, action: Action, backend: &dyn RepoBackend)
```

- `MoveUp` / `MoveDown`: Move cursor to the previous/next node line (a `GraphLine` with `change_id: Some(...)`), skipping continuation/connector lines. Detail lookup is a HashMap get, not a subprocess call.
- `Refresh`: Re-runs `backend.load_graph()`. Attempts to preserve the selected change: finds the previously selected change ID in the new graph. If not found (change was abandoned/rebased away), falls back to working-copy change, then first node.
- `Quit`: Sets `should_quit = true`.
- `JumpToTop` / `JumpToBottom`: Move cursor to first/last node line.

Errors from `backend.load_graph()` during Refresh are caught and stored in `state.error`. The previous graph data is preserved so the UI remains usable.

**M0 impurity note:** Dispatch takes `&dyn RepoBackend` for `Refresh`. In M2, repo calls move to an effect executor and dispatch becomes pure: `(AppState, Action) -> (AppState, Vec<Effect>)`.

### 5.4 Input Mapping

```rust
fn map_event(event: crossterm::event::KeyEvent) -> Option<Action>
```

| Key | Action |
|-----|--------|
| `j` / `Down` | MoveDown |
| `k` / `Up` | MoveUp |
| `q` / `Ctrl-C` | Quit |
| `R` | Refresh |
| `g` | JumpToTop |
| `G` | JumpToBottom |

### 5.5 Rendering

#### Layout

```
┌─────────────────────────────────────┐
│ Graph Panel              (all - 3)  │
│                                     │
│  ◉  ksqxwpml  martin@…  2m ago     │  ┐
│  │  refactor: extract provisioner   │  ┘ <- highlighted (full change block)
│  ◉  ytoqrzxn  martin@…  15m ago    │
│  │  feat: add retry logic           │
│                                     │
├─────────────────────────────────────┤
│ Status Bar (2-3 lines)              │
│ ksqxwpml abc123de  martin@foo.com   │
│ refactor: extract provisioner trait  │
└─────────────────────────────────────┘
```

- **Graph panel** takes all available height minus 2–3 lines for the status bar.
- **Scrolling:** viewport keeps selected change visible (vim-style `scrolloff` behavior).
- **Highlighting:** the entire change block (node line + all continuation lines until the next node line) is rendered with reversed foreground/background. This groups the change visually as a unit.
- **Status bar:** shows the selected change's full metadata from `ChangeDetail`. If `state.error` is `Some`, the status bar displays the error message instead (styled distinctly, e.g. red).

---

## 6. Event Loop (`lajjzy-cli`)

```
1. Initialize terminal (crossterm raw mode, alternate screen)
2. Set up panic handler (restore terminal on panic)
3. Construct JjCliBackend (discover workspace via jj root)
   - On failure: print error, exit 1
4. Load initial graph via backend
   - On failure: print error, exit 1
5. Build initial AppState (cursor on working-copy change)
6. Loop:
   a. Render frame from AppState
   b. Poll for crossterm event (blocking with timeout)
   c. Map event -> Action via input::map_event
   d. If Some(action), dispatch(state, action, backend)
   e. If state.should_quit, break
7. Restore terminal
```

---

## 7. Architectural Constraints

### C1: Facade boundary
`lajjzy-tui` depends on `lajjzy-core` only. It never shells out to `jj` or any external process. All repo access goes through `RepoBackend`. Enforced by Cargo dependency graph.

### C2: No panics on repo operations
Every `RepoBackend` method returns `Result`. Dispatch handles errors by setting `state.error`, never by unwinding. If `jj` is not installed or the workspace is invalid, the app exits with a clear error message at startup.

### C3: Dispatch purity (aspirational)
For M0, dispatch takes `&dyn RepoBackend` — a pragmatic impurity. Starting in M2, repo calls move to an effect executor and dispatch becomes `(AppState, Action) -> (AppState, Vec<Effect>)`.

### C4: jj-lib gate
Before designing M2 (mutations), audit jj-lib's public API surface to verify: repo locking, transactional operations, diff computation, and conflict representation. Do not design the jj-lib backend from assumptions.

### C5: CLAUDE.md from first commit
The repo's `CLAUDE.md` includes: build/test commands, crate structure, architectural constraints, and the convention that all jj interaction is mediated through `lajjzy-core`.

### C6: Error messages are user-facing
When `jj` CLI calls fail, stderr is captured and displayed in the status bar or as an exit message. No silent error swallowing.

---

## 8. Testing Strategy

### `lajjzy-core`
- **Integration tests:** Create a temp directory, `jj git init`, make changes, call `load_graph()`, assert on parsed output and detail data. Requires `jj` in PATH.
- **Trait enables mocking:** TUI tests use `MockBackend` with canned `GraphData`, no filesystem needed.

### `lajjzy-tui`
- **State transition tests:** Call `dispatch()` with `MockBackend`, assert on `AppState`. E.g., `MoveDown` skips connector lines, `JumpToTop` lands on first node line, cursor initializes on working-copy change.
- **Snapshot tests:** Render a frame from known `AppState` using ratatui's `TestBackend`, compare against stored snapshot.

### `lajjzy-cli`
- No dedicated tests for M0. Tested implicitly by running the binary.

### CI
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test` (requires `jj` in PATH for core integration tests)

---

## 9. Out of Scope for M0

- Detail pane (file list, diff view) — M1
- Any mutations (edit, amend, squash, split) — M2
- Stack detection and grouping — M3
- Conflict resolution UI — M4
- Forge integration — M5
- Configurable keymap, theming, mouse support — M6
- Async runtime (tokio) — M2/M3
- Custom graph renderer — M3
