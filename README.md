# lajjzy: High-Level Design Document

## 1. Vision

A keyboard-driven, lazygit-style terminal UI for [Jujutsu (jj)](https://github.com/jj-vcs/jj) that makes stacked-change and code-review workflows — in the style of git-butler, Gerrit, and Graphite — feel effortless. The goal is not to replicate lazygit with a different backend, but to build a TUI native to jj's data model: immutable changes, automatic rebasing, first-class conflicts, and an operation log that makes every action reversible.

---

## 2. Design Principles

| # | Principle | Implication |
|---|-----------|-------------|
| 1 | **jj-native, not git-shaped** | The UI is organised around jj's change graph, not branches. Branches are labels you can attach, not the primary navigation axis. |
| 2 | **Trait-mediated, not library-coupled** | All repo access goes through the `RepoBackend` trait in `lajjzy-core`. The TUI crate never touches `jj-lib` or shells out to any process. This enables testing with mock backends and absorbs jj-lib API churn. |
| 3 | **Reversible by default** | Surface jj's operation log as a first-class undo stack. Every mutating action records an op; `u` undoes it. No confirmation dialogs for non-destructive operations. |
| 4 | **Progressive disclosure** | Common workflows (amend, squash, rebase, push) are one or two keystrokes. Advanced operations (split, partial squash, conflict editing) are available but not in the way. |
| 5 | **Async and non-blocking** | Network operations (fetch, push) and expensive computations (large diffs, conflict detection) run in background tasks with progress indication. The UI never freezes. |
| 6 | **Errors are data, not panics** | Every fallible operation returns `Result`. The TUI renders errors in the status bar or as a graceful exit message. No unwinding on repo failures. |

---

## 3. Architectural Constraints

These are enforced from M0 onward. They are non-negotiable and mechanically checked where possible.

### C1: Facade Boundary

`lajjzy-tui` depends on `lajjzy-core` only. It never shells out to `jj` or any external process directly. All repo access goes through the `RepoBackend` trait.

**Enforcement:**
- `lajjzy-tui/Cargo.toml` never lists `jj-lib` as a dependency.
- CI grep check: no `std::process::Command` usage in `lajjzy-tui`.

### C2: No Panics on Repo Operations

Every `RepoBackend` method returns `Result`. Dispatch handles errors by updating state (e.g. setting an error message in the status bar), never by unwinding. If jj isn't installed or the workspace is invalid, the app shows a clear error and exits gracefully.

**Enforcement:**
- `#[deny(clippy::unwrap_used, clippy::expect_used)]` in `lajjzy-tui`.
- `RepoBackend` trait definition: every method returns `Result<T, RepoError>`.

### C3: Dispatch Purity (Aspirational)

For M0, dispatch takes `&dyn RepoBackend` for `Refresh`. This is a pragmatic impurity — the graph view needs repo data to render, and threading it through an effect executor before the effect system exists adds complexity without value.

Starting in M2, repo calls move to an effect executor and dispatch becomes `(AppState, Action) → (AppState, Vec<Effect>)` — a pure function. The effect executor handles `RepoOp`, `SpawnTask`, `Quit`, etc.

**Enforcement (from M2):**
- Dispatch function signature takes no `&dyn RepoBackend` parameter.
- All `RepoBackend` calls originate from the effect executor module only.

### C4: jj-lib Gate

Before designing M2 (mutations), audit `jj-lib`'s public API surface to verify: repo locking semantics, transactional operations, diff computation, and conflict representation. Do not design the `jj-lib` backend from assumptions.

**Enforcement:**
- M2 planning begins with a written API audit document committed to the repo.
- Each `RepoBackend` method documents which `jj-lib` API it wraps (or notes "not yet verified").

### C5: CLAUDE.md from First Commit

The repo's `CLAUDE.md` includes: build/test commands, crate structure, these architectural constraints, and the convention that all jj interaction is mediated through `lajjzy-core`.

**Enforcement:**
- CI check: `CLAUDE.md` exists at repo root.

### C6: Error Messages Are User-Facing

When operations fail, the error context is captured and displayed in the status bar or as an exit message. No silent swallowing of errors. Errors from the jj backend should include enough context for the user to act (e.g. "workspace not found at /foo/bar" not "operation failed").

**Enforcement:**
- `RepoError` variants carry structured context (path, operation name, underlying cause).
- Review checklist item: every `?` propagation in `lajjzy-tui` must land in user-visible UI.

---

## 4. Core Concepts Mapped to UI

### 4.1 The Change Graph (primary view)

jj's repo model is a DAG of **changes**, each of which is an immutable snapshot with a unique change ID. The primary view is a scrollable, ASCII-art (or Unicode box-drawing) graph — similar to `jj log` — but interactive:

```
  ◉  ksqxwpml  martin@…  2m ago          ← cursor highlights
  │  (empty) (no description set)            the full block
  ◉  ytoqrzxn  martin@…  15m ago  main@origin
  │  refactor: extract provisioner trait
  ◉  vlpmrokx  martin@…  1h ago
  │  feat: add retry logic to IPMI calls
  ◉  zzzzzzzz  root()
```

- **Initial cursor** lands on the working-copy change (`@`), not the first node in the graph. This is the change the user is most likely to act on. `@` also jumps back to it from anywhere.
- **Block highlight:** The cursor highlights the entire change block (ID line + description lines + any bookmark/conflict annotations), not a single line. A change with a multi-line description occupies multiple rows; the highlight covers all of them. This is what makes `j`/`k` feel like moving between *changes* rather than between *lines* — the unit of navigation matches the unit of work.
- **Cursor movement** (`j`/`k`) moves between changes (blocks), not lines.
- **Enter** expands a change to show its file list in the detail pane (no backend call — file data is loaded eagerly with the graph).
- **Stacks** are visually grouped: contiguous linear chains rooted on a trunk branch are highlighted as a unit, with a gutter annotation showing review state if connected to a forge.

### 4.2 Panels

The UI is divided into context-sensitive panels, loosely following lazygit's layout but adapted for jj:

```
┌─────────────────────────────────────────────────────────┐
│ [1] Change Graph  │ [2] Detail Pane                     │
│                   │                                     │
│  ◉ ksqxwpml ...   │  Files changed:                     │
│  ◉ ytoqrzxn ...   │    M src/provisioner.rs             │
│  ◉ vlpmrokx ...   │    A src/retry.rs                   │
│                   │    D src/old_ipmi.rs                 │
│                   │                                     │
│                   │                                     │
│───────────────────│─────────────────────────────────────│
│ [3] Status / Op Log / Conflicts                         │
│  Op log: 4 operations │ Conflicts: 0 │ Bookmarks: 3    │
└─────────────────────────────────────────────────────────┘
```

| Panel | Content |
|-------|---------|
| **Change Graph** | The interactive DAG. Always visible. |
| **Detail Pane** | Context-sensitive: file list (from eager graph data), diff hunks (lazy on drill-down), change description editor, conflict markers. |
| **Status Bar** | Op log summary, conflict count, current bookmarks, background task progress, error messages (C6). |

Panel focus follows a `Tab` / `Shift-Tab` cycle. Each panel has its own local keymap layered atop global bindings.

### 4.3 Stacked Changes (git-butler / Graphite model)

A "stack" is a contiguous linear chain of changes. `lajjzy` treats stacks as a first-class grouping:

- **`S`** — enter **Stack Mode**: zooms the graph view to a single stack, shows per-change review status, and enables bulk operations.
- **Reorder** — `Shift-J` / `Shift-K` to reorder changes within a stack (executes rebase via `RepoBackend`, with automatic conflict detection).
- **Split** — `s` on a change opens an interactive hunk-picker to split it into two changes.
- **Squash up/down** — `Shift-S` squashes the selected change into its parent; or with a target picker, into any ancestor in the stack.
- **Push stack** — `P` pushes all bookmarked changes in the stack to their respective remotes. For Gerrit workflows, this means updating patchsets; for GitHub/GitLab, force-pushing review branches.

### 4.4 Conflict Resolution

jj treats conflicts as first-class data rather than an error state. `lajjzy` leans into this:

- Conflicts appear as annotated nodes in the graph with a `⚠` marker.
- Selecting a conflicted change shows the conflict hunks inline in the detail pane.
- A built-in 3-way merge view (two sides + base) allows resolution without leaving the TUI.
- Alternatively, `e` opens the configured external merge tool.
- Resolving and amending is a single operation — no separate "mark resolved" step.

### 4.5 Operation Log and Undo

- **`O`** — toggle the operation log as a full-screen overlay, showing the history of every repo mutation.
- **`u`** — undo the last operation (equivalent to `jj op undo`). Repeatable.
- **`Ctrl-R`** — redo (restore an undone operation).
- Each op log entry shows a human-readable summary of what changed.

---

## 5. Architecture

### 5.1 Crate Structure

```
lajjzy/
├── crates/
│   ├── lajjzy-core/         # RepoBackend trait + jj-lib implementation
│   │   └── src/
│   │       ├── lib.rs        # Re-exports, RepoBackend trait definition
│   │       ├── backend.rs    # RepoBackend trait and domain types
│   │       ├── jj_backend.rs # jj-lib implementation of RepoBackend
│   │       ├── graph.rs      # Change graph model (backend-agnostic)
│   │       ├── stack.rs      # Stack detection, ordering
│   │       ├── diff.rs       # Diff types (backend-agnostic)
│   │       ├── error.rs      # RepoError with structured context (C6)
│   │       └── mock.rs       # Mock backend for testing (cfg(test) or feature-gated)
│   ├── lajjzy-tui/          # Terminal UI: ratatui widgets, input handling, state machine
│   │   └── src/
│   │       ├── app.rs        # Top-level app state and event loop
│   │       ├── dispatch.rs   # Action → State+Effects (C3)
│   │       ├── effects.rs    # Effect executor (from M2)
│   │       ├── input.rs      # Keymap routing, modal input handling
│   │       ├── panels/       # One module per panel (graph, detail, status, op_log)
│   │       ├── widgets/      # Custom ratatui widgets (graph renderer, diff viewer, etc.)
│   │       └── tasks.rs      # Async background task management
│   └── lajjzy-cli/          # Binary crate: arg parsing, terminal setup, panic handler
│       └── src/
│           └── main.rs
├── Cargo.toml                # Workspace root
├── CLAUDE.md                 # C5: from first commit
└── README.md
```

### 5.2 The RepoBackend Trait

The central abstraction. All repo access — reads and writes — goes through this trait. `lajjzy-tui` depends only on this trait and the domain types defined alongside it, never on `jj-lib` directly (C1).

```rust
/// Domain types — defined in lajjzy-core, used by lajjzy-tui.
/// These are jj-lib-independent representations.
pub struct ChangeInfo {
    pub change_id: ChangeId,
    pub commit_id: CommitId,
    pub description: String,
    pub author: Signature,
    pub timestamp: DateTime<Utc>,
    pub is_working_copy: bool,
    pub is_empty: bool,
    pub has_conflict: bool,
    pub bookmarks: Vec<String>,
    pub parent_change_ids: Vec<ChangeId>,
    /// Files touched by this change — loaded eagerly with the graph.
    /// This avoids a backend round-trip on every cursor move.
    pub files: Vec<FileChange>,
}

pub struct FileChange { /* path, status (M/A/D/R), size delta */ }
pub struct DiffHunk  { /* header, lines, context */ }

/// The facade trait. Every method returns Result<T, RepoError> (C2).
pub trait RepoBackend: Send + Sync {
    /// Discover and open a jj workspace from a directory.
    fn open_workspace(&self, path: &Path) -> Result<(), RepoError>;

    /// Load the change graph: all visible changes with parent relationships
    /// and per-change file lists. This is the only call required for initial
    /// render — cursor movement within the graph never triggers a backend call.
    fn load_graph(&self) -> Result<Vec<ChangeInfo>, RepoError>;

    /// Compute diff hunks for a file in a change. This is the one read-path
    /// call that remains lazy — hunk-level detail is only needed when the
    /// user drills into a specific file (Enter on file list).
    fn file_diff(&self, id: &ChangeId, path: &RepoPath) -> Result<Vec<DiffHunk>, RepoError>;

    /// Load the operation log.
    fn op_log(&self) -> Result<Vec<OpLogEntry>, RepoError>;

    // --- Mutations (M2+, pending C4 audit) ---

    /// Edit a change's description.
    fn set_description(&self, id: &ChangeId, desc: &str) -> Result<(), RepoError>;

    /// Create a new empty change after the given parent.
    fn new_change(&self, after: &ChangeId) -> Result<ChangeId, RepoError>;

    /// Abandon a change.
    fn abandon(&self, id: &ChangeId) -> Result<(), RepoError>;

    /// Squash a change into its parent.
    fn squash(&self, id: &ChangeId) -> Result<(), RepoError>;

    /// Rebase a change onto a new parent.
    fn rebase(&self, id: &ChangeId, new_parent: &ChangeId) -> Result<(), RepoError>;

    /// Undo the last operation.
    fn op_undo(&self) -> Result<(), RepoError>;

    /// Redo a previously undone operation.
    fn op_redo(&self) -> Result<(), RepoError>;
}
```

**Design note — eager file lists:** The previous design had a separate `change_files()` method called when the cursor moved to a change. This spawns a backend call per keypress, which is a latency problem — holding `j` down would queue calls and the UI would stutter or lag. Moving file lists into `load_graph()` means the entire graph + detail data arrives in a single batch. The tradeoff is a larger initial payload, but jj's diff computation is fast for the common case (tens of changes, each touching a handful of files), and the alternative is an app that feels sluggish on the most basic interaction.

`file_diff()` remains lazy because hunk-level detail is expensive and only needed on explicit drill-down (Enter on a file). This is a deliberate choice: the user has signalled intent, so a brief load is acceptable and expected.

**Note (C4):** The mutation methods above are *aspirational interface sketches*. Their signatures will be revised after the jj-lib API audit that gates M2. The read-only methods (`load_graph`, `file_diff`, `op_log`) are implemented first and validated against jj-lib during M0/M1.

### 5.3 Dependency Map

```
lajjzy-cli
  ├── lajjzy-tui
  │     ├── lajjzy-core       (RepoBackend trait + domain types ONLY)
  │     ├── ratatui
  │     ├── crossterm
  │     └── tokio
  └── lajjzy-core             (wires up JjBackend as the concrete impl)
        └── jj-lib            (+ jj-lib's transitive deps)
```

The key constraint: `lajjzy-tui/Cargo.toml` lists `lajjzy-core` but **not** `jj-lib` (C1). The binary crate (`lajjzy-cli`) constructs the concrete `JjBackend` and passes `&dyn RepoBackend` to the TUI.

Key external crates:

| Crate | Role | Depended on by |
|-------|------|----------------|
| `jj-lib` | Repo operations, change model, merge, diff | `lajjzy-core` only |
| `ratatui` | Terminal rendering framework | `lajjzy-tui` |
| `crossterm` | Cross-platform terminal backend | `lajjzy-tui` |
| `tokio` | Async runtime for background tasks | `lajjzy-tui`, `lajjzy-core` |
| `tui-textarea` | Multi-line text editing (descriptions) | `lajjzy-tui` |
| `nucleo` / `fuzzy-matcher` | Fuzzy-find for change/file/bookmark picker | `lajjzy-tui` |
| `similar` | Fallback inline diff if jj-lib's diff API is insufficient | `lajjzy-core` |

### 5.4 Event Loop

The main loop follows the standard ratatui pattern, extended with an async task channel:

```
┌──────────────────────────────────────────────────────────┐
│                      Event Loop                          │
│                                                          │
│   ┌──────────┐   ┌──────────┐   ┌────────────────┐      │
│   │ Terminal  │   │  Tick    │   │  Background    │      │
│   │  Input    │──▶│  Merge   │◀──│  Task Results  │      │
│   │ (crossterm│   │          │   │  (tokio mpsc)  │      │
│   └──────────┘   └────┬─────┘   └────────────────┘      │
│                       │                                  │
│                       ▼                                  │
│  ┌─────────────────────────────────────────────────┐     │
│  │  Dispatch (C3)                                  │     │
│  │                                                 │     │
│  │  M0–M1: fn dispatch(&AppState, Action,          │     │
│  │                      &dyn RepoBackend)          │     │
│  │           → AppState                            │     │
│  │                                                 │     │
│  │  M2+:   fn dispatch(&AppState, Action)          │     │
│  │           → (AppState, Vec<Effect>)             │     │
│  └───────────────────┬─────────────────────────────┘     │
│                      │                                   │
│           ┌──────────┴──────────┐                        │
│           ▼                     ▼                        │
│  ┌──────────────┐     ┌──────────────────┐               │
│  │   Render     │     │ Effect Executor  │  (M2+)        │
│  │  (state →    │     │ RepoOp(...)      │               │
│  │   frame)     │     │ SpawnTask(...)   │               │
│  └──────────────┘     │ Quit             │               │
│                       └──────────────────┘               │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

- **Input** is read from crossterm's async event stream.
- **Dispatch** maps `(state, action)` to new state. In M0–M1, it takes a `&dyn RepoBackend` for reads (C3 pragmatic impurity). From M2 onward, it returns `Vec<Effect>` and the effect executor handles repo calls.
- **Effect Executor** (M2+) processes effects: repo mutations via `RepoBackend`, spawning background tasks, or triggering a re-render.
- **Render** is a pure function of `AppState` — standard immediate-mode ratatui.

### 5.5 Repo Access Model

`jj-lib` requires careful handling: repo operations take a mutable lock, and some (like `rebase`) can be expensive.

- **Read path:** The graph view is built from data returned by `RepoBackend::load_graph()`, which returns all visible changes *with their file lists* in a single batch. This means cursor navigation never triggers a backend call. Internally, `load_graph()` takes a read-only snapshot — cheap thanks to jj's copy-on-write store. The only lazy read is `file_diff()`, called when the user drills into a specific file's hunks.
- **Write path:** Mutations go through `RepoBackend` methods, which internally acquire the repo lock, perform the operation, record an op, and return. The TUI then calls `load_graph()` again to rebuild its model from the new state.
- **Background tasks:** Fetch/push run on a separate tokio task. On completion, they send a message to the event loop, which triggers a `load_graph()` refresh.

### 5.6 Forge Integration (Gerrit / GitHub / GitLab)

A `Forge` trait abstracts code-review backends. This is a separate trait from `RepoBackend` — forges are about review workflows, not repo operations.

```rust
#[async_trait]
pub trait Forge: Send + Sync {
    /// Discover review state for a set of changes (batch).
    async fn review_status(&self, changes: &[ChangeId]) -> Result<Vec<ReviewState>>;

    /// Push a stack for review. Returns updated review metadata.
    async fn push_for_review(&self, stack: &Stack) -> Result<Vec<ReviewUpdate>>;

    /// Fetch review comments for a change.
    async fn fetch_comments(&self, change: ChangeId) -> Result<Vec<Comment>>;
}
```

Concrete implementations:

| Forge | Mechanism |
|-------|-----------|
| **Gerrit** | SSH push with `refs/for/<branch>`, Gerrit REST API for review status. Change-Id trailer mapping to jj change IDs. |
| **GitHub** | One branch per change in stack, pushed to a fork or review remote. GitHub API (via `octocrab` or raw HTTP) for PR status, comments. |
| **GitLab** | Similar to GitHub model, with GitLab MR API. |

Forge detection is automatic from remote URL patterns, with manual override via config.

---

## 6. Keymap (Draft)

Global:

| Key | Action |
|-----|--------|
| `q` / `Ctrl-C` | Quit |
| `Tab` / `Shift-Tab` | Cycle panel focus |
| `?` | Show contextual help overlay |
| `u` | Undo last operation |
| `Ctrl-R` | Redo |
| `/` | Fuzzy-find changes |
| `b` | Bookmark picker |
| `O` | Toggle operation log |
| `R` | Refresh (re-read repo) |
| `f` | Fetch from remote (background) |

Graph panel:

| Key | Action |
|-----|--------|
| `j` / `k` | Move cursor down / up |
| `Enter` | Expand change → show file list in detail pane |
| `e` | Edit change description |
| `a` | Amend working copy into selected change |
| `n` | New empty change after selected |
| `s` | Split change (interactive hunk picker) |
| `r` | Rebase selected change (target picker) |
| `Shift-S` | Squash into parent |
| `S` | Enter stack mode |
| `d` | Abandon (delete) change |
| `P` | Push selected / stack |
| `@` | Jump to working-copy change |

Detail pane (file list):

| Key | Action |
|-----|--------|
| `j` / `k` | Move cursor |
| `Enter` | Open hunk diff view for file |
| `e` | Open file in `$EDITOR` |
| `Space` | Toggle file selection (for partial squash/split) |

Detail pane (diff view):

| Key | Action |
|-----|--------|
| `j` / `k` | Scroll |
| `n` / `N` | Next / previous hunk |
| `Space` | Toggle hunk selection |
| `]` / `[` | Expand / collapse context lines |

---

## 7. State Model (Sketch)

```rust
pub struct AppState {
    /// Which panel has focus.
    pub focus: PanelId,

    /// The change graph as returned by RepoBackend::load_graph(),
    /// with layout information computed for rendering.
    pub graph: ChangeGraph,

    /// Index of the working-copy change (@) in the graph.
    /// Cursor initialises here on load; `@` key jumps back to it.
    pub working_copy_idx: usize,

    /// Cursor positions per panel. Graph panel cursor initialised
    /// to working_copy_idx, not 0.
    pub cursors: HashMap<PanelId, usize>,

    /// Currently expanded change (if any) with its file list.
    /// File list is drawn from the eagerly-loaded ChangeInfo::files —
    /// no backend call on expand.
    pub expanded: Option<ExpandedChange>,

    /// Active modal (help overlay, picker, text input, etc.).
    pub modal: Option<Modal>,

    /// Background task tracker.
    pub tasks: TaskTracker,

    /// User-visible messages: errors (C6), success confirmations.
    pub notifications: VecDeque<Notification>,

    /// Undo/redo cursor into the op log.
    pub op_cursor: OpCursor,

    /// Last error from a RepoBackend call, displayed in status bar (C2, C6).
    pub last_error: Option<RepoError>,
}
```

`AppState` is the single source of truth. Rendering is a pure function of it. The state contains no repo handles, no `jj-lib` types, and no references to the backend — only the domain types defined in `lajjzy-core` (C1).

---

## 8. Build and Development

- **Minimum Rust version:** latest stable (no nightly features required).
- **Dev workflow:** `cargo run -p lajjzy-cli` from workspace root. `bacon` or `cargo watch` for incremental rebuild during TUI iteration.
- **Testing strategy:**
  - `lajjzy-core`: unit and integration tests against temporary jj repos (jj-lib provides test helpers for in-memory repos). Also tests against the mock backend.
  - `lajjzy-tui`: snapshot tests of rendered frames using `ratatui`'s `TestBackend`, driven by the mock `RepoBackend`. State transition tests via `(AppState, Action) → AppState` without a real terminal or real repo.
  - End-to-end: a small suite of scripted scenarios using `expect`-style terminal automation (stretch goal).
- **CI checks:**
  - `cargo clippy`, `cargo test`, `cargo fmt --check`
  - C1 enforcement: `lajjzy-tui/Cargo.toml` does not contain `jj-lib`; `grep -r 'std::process::Command' crates/lajjzy-tui/` returns nothing.
  - C2 enforcement: `#[deny(clippy::unwrap_used, clippy::expect_used)]` on `lajjzy-tui`.
  - C5 enforcement: `test -f CLAUDE.md`

---

## 9. Milestones

### M0 — Skeleton (weeks 1–2)

**Constraints active:** C1, C2, C3 (pragmatic impurity), C5, C6.

- Workspace setup with three crates. `CLAUDE.md` committed (C5).
- `RepoBackend` trait defined with read-only methods: `open_workspace`, `load_graph` (with eager file lists per change).
- `JjBackend` implementation: links `jj-lib`, implements `open_workspace` + `load_graph`.
- `MockBackend` with hardcoded graph data for TUI development.
- Read-only graph view rendering in ratatui:
  - Cursor navigation (`j`/`k`) moves between change blocks, not lines.
  - Initial cursor lands on the working-copy change (`@`).
  - Block highlight covers the full change (ID line + description + annotations).
  - `Enter` expands file list in detail pane from eager data (no backend call).
- Graceful error on invalid workspace (C2, C6).
- CI pipeline with C1/C2/C5 checks.
- No mutations.

### M1 — Read-Only Explorer (weeks 3–4)

**Constraints active:** all M0 constraints.

- `RepoBackend` extended: `file_diff`, `op_log`.
- Detail pane: hunk diff view when drilling into a specific file (lazy `file_diff` call — the one read-path call that remains on-demand).
- Op log viewer (read-only).
- Bookmark list.
- Fuzzy-find across changes.
- Status bar shows error context from failed backend calls (C6).

### M2 — Core Mutations (weeks 5–7)

**Gate:** C4 — jj-lib API audit completed and committed before design work begins.

**Constraints active:** all M0/M1 constraints + C3 (dispatch becomes pure), C4.

- Dispatch refactored: `(AppState, Action) → (AppState, Vec<Effect>)` (C3).
- Effect executor handles `RepoOp`, `SpawnTask`, `Quit`.
- `RepoBackend` extended with mutation methods (signatures validated by C4 audit).
- Edit description, amend, new change, abandon.
- Squash (full and partial via hunk selection).
- Split (interactive).
- Undo / redo via op log.
- Inline status notifications for success and failure.

### M3 — Stack Workflows (weeks 8–10)

- Stack detection and visual grouping.
- Stack mode: reorder, bulk squash, bulk rebase.
- Push stack to remote (git-native, no forge API yet).

### M4 — Conflict Handling (weeks 11–12)

- Conflict markers in graph view.
- Inline 3-way merge view.
- External merge tool launch.
- Resolve-and-amend flow.

### M5 — Forge Integration (weeks 13–16)

- `Forge` trait and Gerrit implementation.
- GitHub PR integration (status display, push-for-review).
- Review comments in detail pane (stretch).

### M6 — Polish and Release (ongoing)

- Configurable keymap (TOML/YAML).
- Theming (base16 / terminal colours).
- Mouse support (optional, not primary input).
- Packaging: `cargo install`, AUR, Homebrew, Nix flake.

### M7 - implement **complete** UI/UX for jj's branchless and safe history editing

* Automatic Commits & No Index: There is no git add or staging area. Every change in your working directory is automatically recorded as a "working copy" commit.
* Stable Change IDs: Unlike Git hashes that change when you rebase or amend, jj uses permanent Change IDs. This allows the tool to automatically rebase all descendant changes whenever a parent is modified.
* First-Class Conflicts: Conflicts do not stop your workflow. They are stored in the commit tree as metadata, allowing you to resolve them whenever it is convenient rather than immediately during a merge.
* Operation Log & Undo: Every command can be undone. jj maintains a full history of operations (like a supercharged reflog), making experimentation and history rewriting fearless. [5, 8, 9, 10, 11, 12, 13, 14]

### M8 - implement UI/UX for GitButler-inspired Simultaneous Parallel Work using jj

* Virtual Branch Lanes: You can work on multiple independent features simultaneously in the same working directory. Changes can be moved into different "lanes" (branches) like a Kanban board.
* Early Conflict Detection: Conflicts are surfaced as soon as you apply a teammate's branch locally, rather than waiting until the final merge into the main branch.
* Branch Composition: You can apply and test multiple branches at once without leaving your current coding context or performing complex checkouts. [16, 17]

### M9 - implement UI/UX for Gerrit-inspired Atomic, Commit-Level Reviews using jj

* Commit-by-Commit Review: Every single commit is treated as a separate reviewable unit. This encourages small, self-contained changes rather than monolithic pull requests.
* Automated Review Branches: When you push a commit, Gerrit automatically creates temporary, "invisible" branches to hold the change for review.
* Change Tracking Across Force Pushes: Gerrit excels at showing the difference between different versions of the same commit (patchsets), even if you've rebased or amended it.

### M10 - Implement UI/UX for Graphite-inspired Automated Stacked Pull Requests

* Automatic Restacking: If you modify a commit in the middle of a "stack" of dependent branches, Graphite automatically rebases all "downstack" (child) branches for you.
* One Command Submission: Use gt stack submit to push an entire sequence of dependent branches and create individual GitHub PRs for each, all at once.
* Stack-Aware Review UI: It provides a custom web interface that understands the relationship between PRs, allowing reviewers to navigate through the logical sequence of a feature.
* Merge Queues: A built-in merge queue batches multiple PRs together to test them in parallel, ensuring they land on the main branch without conflicts or breaking CI.

---

## 10. Open Questions

1. **jj-lib stability.** `jj-lib` doesn't promise a stable API yet. Strategy: pin to a specific jj release per `lajjzy` release; the `RepoBackend` trait absorbs API churn so only `jj_backend.rs` needs updating.
2. **Working copy integration.** jj snapshots the working copy on every operation. How much of this should `lajjzy` surface vs. keep implicit? Lazygit shows the working tree diff prominently; jj's model is different — there's always a "working-copy change" that auto-snapshots.
3. **Tree-shaped stacks.** git-butler supports "virtual branches" (parallel stacks). jj's model naturally supports this via the DAG. Should stack mode handle tree-shaped stacks from day one, or start linear-only?
4. **Forge auth.** Gerrit uses SSH keys; GitHub/GitLab need tokens or OAuth. Defer to `gh`/`glab` CLI auth? Or manage credentials independently?
5. **Backend swappability.** The `RepoBackend` trait opens the door to non-jj backends (e.g. raw git via `git2`). Is this a goal or an accidental affordance? For now, treat it as an accidental affordance — the trait exists for testability and facade isolation, not as a plugin system.

---

## 11. Prior Art and Differentiation

| Tool | Relationship |
|------|-------------|
| **lazygit** | Primary UX inspiration. But git-native and shells out to CLI. `lajjzy` is jj-native and accesses the repo through a typed trait boundary. |
| **lazyjj** | Existing jj TUI. Lighter-weight, focused on basic operations. `lajjzy` aims for deeper stack/review workflows and forge integration. |
| **gg (git-butler)** | GUI, not TUI. Virtual-branch model is an inspiration for stack management, but `lajjzy` uses jj's native DAG rather than a custom overlay. |
| **Gerrit** | Review model inspiration. `lajjzy` targets Gerrit as a first-class forge backend because jj's change-ID model maps cleanly to Gerrit's. |
| **Graphite** | Stack-aware CLI/web review tool for GitHub. `lajjzy` borrows the "push a stack" UX but doesn't depend on Graphite's service. |
| **jj CLI** | Always available as an escape hatch. `lajjzy` should never be *required* — it accelerates workflows that are possible on the CLI. |
