# M0 Skeleton Design — lajjzy

**Date:** 2026-03-20
**Scope:** M0 — Read-only graph view, cursor navigation, status bar
**Status:** Draft

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
| **Selection UX** | Highlight + status bar metadata | Cursor highlights change node; bottom bar shows full detail |

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
| `crossterm` | — | yes | yes |
| `serde` + `serde_json` | yes | — | — |
| `anyhow` | yes | yes | yes |

---

## 4. Core Layer (`lajjzy-core`)

### 4.1 RepoBackend Trait

```rust
pub trait RepoBackend {
    /// Load the full graph (jj log output) for display.
    fn load_graph(&self) -> Result<Vec<GraphLine>>;

    /// Load detailed metadata for a single change (for status bar).
    fn change_detail(&self, change_id: &str) -> Result<ChangeDetail>;
}
```

### 4.2 Data Types

```rust
/// One line of jj's graph output — the raw visual string plus parsed metadata.
pub struct GraphLine {
    /// The full rendered line from jj log (graph glyphs + text), delimiter stripped.
    pub raw: String,
    /// The change ID if this line represents a change node, None for connector lines.
    pub change_id: Option<String>,
}

/// Detailed info for the status bar, fetched on cursor change.
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
}
```

### 4.3 JjCliBackend

- **Construction:** Takes a workspace path. Validates with `jj root` on creation; returns error if jj is not installed or path is not a jj workspace.
- **`load_graph()`:** Runs `jj log` with a template that appends `\x1F<change_id>` (ASCII unit separator) to each change node line. Parses each line: if it contains the delimiter, splits to extract the change ID and strips the delimiter from the display string. Connector lines pass through as-is with `change_id: None`.
- **`change_detail()`:** Runs `jj log -r <change_id> --no-graph -T <json_template>` where the template outputs a JSON object with all `ChangeDetail` fields. Parsed with serde_json.

---

## 5. TUI Layer (`lajjzy-tui`)

### 5.1 AppState

```rust
pub struct AppState {
    /// All graph lines from jj log.
    pub graph: Vec<GraphLine>,
    /// Index into graph lines (cursor position, always on a change node).
    pub cursor: usize,
    /// Cached detail for the currently selected change.
    pub selected_detail: Option<ChangeDetail>,
    /// Whether the app should quit.
    pub should_quit: bool,
}
```

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

- `MoveUp` / `MoveDown`: Move cursor to the previous/next `GraphLine` with `change_id: Some(...)`, skipping connector lines. Updates `selected_detail` via `backend.change_detail()`.
- `Refresh`: Re-runs `backend.load_graph()`, re-clamps cursor.
- `Quit`: Sets `should_quit = true`.
- `JumpToTop` / `JumpToBottom`: Move cursor to first/last change node.

**M0 impurity note:** Dispatch takes `&dyn RepoBackend` to call `change_detail()` and `load_graph()`. In M2, these calls move to an effect executor and dispatch becomes pure: `(AppState, Action) -> (AppState, Vec<Effect>)`.

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
│  ◉  ksqxwpml  martin@…  2m ago     │  <- highlighted
│  │  (empty) (no description set)    │
│  ◉  ytoqrzxn  martin@…  15m ago    │
│  │  refactor: extract provisioner   │
│                                     │
├─────────────────────────────────────┤
│ Status Bar (2-3 lines)              │
│ ksqxwpml abc123de  martin@foo.com   │
│ (empty) (no description set)        │
└─────────────────────────────────────┘
```

- Graph panel takes all available height minus status bar lines.
- Scrolling: viewport keeps selected change visible (vim-style `scrolloff` behavior).
- Highlighting: selected change's line rendered with reversed foreground/background.
- Status bar: full metadata from `ChangeDetail` for the selected change.

---

## 6. Event Loop (`lajjzy-cli`)

```
1. Initialize terminal (crossterm raw mode, alternate screen)
2. Set up panic handler (restore terminal on panic)
3. Construct JjCliBackend (discover workspace via jj root)
4. Load initial graph via backend
5. Build initial AppState
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
Every `RepoBackend` method returns `Result`. Dispatch handles errors by updating state (e.g., error message in status bar), never by unwinding. If `jj` is not installed or the workspace is invalid, the app exits with a clear error message.

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
- **Integration tests:** Create a temp directory, `jj git init`, make changes, call `load_graph()` and `change_detail()`, assert on parsed output. Requires `jj` in PATH.
- **Trait enables mocking:** TUI tests use `MockBackend` with canned data, no filesystem needed.

### `lajjzy-tui`
- **State transition tests:** Call `dispatch()` with `MockBackend`, assert on `AppState`. E.g., `MoveDown` skips connector lines, `JumpToTop` lands on first change node.
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
