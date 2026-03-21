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

`lajjzy-tui` is pure: it contains types, dispatch logic, and rendering functions. It never performs IO of any kind — no `RepoBackend` calls, no subprocess spawning, no filesystem access. All IO (repo operations, `$EDITOR` launch, merge tool launch, thread spawning) lives in the effect executor in `lajjzy-cli`.

**Enforcement:**
- `lajjzy-tui/Cargo.toml` never lists `jj-lib` as a dependency.
- CI grep check: no `std::process::Command` usage in `lajjzy-tui`.
- CI grep check: no `std::thread::spawn` in `lajjzy-tui`.
- No exceptions. If something needs IO, it's an `Effect` that the executor handles.

### C2: No Panics on Repo Operations

Every `RepoBackend` method returns `Result`. Dispatch handles errors by updating state (e.g. setting an error message in the status bar), never by unwinding. If jj isn't installed or the workspace is invalid, the app shows a clear error and exits gracefully.

**Enforcement:**
- `#[deny(clippy::unwrap_used, clippy::expect_used)]` in `lajjzy-tui`.
- `RepoBackend` trait definition: every method returns `Result<T, RepoError>`.

### C3: Dispatch Purity (Aspirational)

For M0, dispatch takes `&dyn RepoBackend` for `Refresh`. This is a pragmatic impurity — the graph view needs repo data to render, and threading it through the effect executor before the effect system exists adds complexity without value. The `RepoBackend` call happens through the trait (no `Command` or `spawn` in `lajjzy-tui`), so C1's grep checks still pass.

Starting in M2, repo calls move to the effect executor in `lajjzy-cli` and dispatch becomes `(AppState, Action) → (AppState, Vec<Effect>)` — a pure function.

**Enforcement (from M2):**
- Dispatch function signature takes no `&dyn RepoBackend` parameter.
- All `RepoBackend` calls originate from `lajjzy-cli/src/executor.rs` only.

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

jj treats conflicts as first-class data rather than an error state. `lajjzy` leans into this with a dedicated conflict workflow that activates automatically.

**Graph view — conflict markers:**

Conflicted changes display a `⚠` marker in the graph. The marker includes a count: `⚠3` means 3 conflicted files. This is drawn from `ChangeInfo::conflicts`, loaded eagerly — no backend call to discover conflict state.

```
  ◉  ksqxwpml  martin@…  2m ago  ⚠3
  │  rebase: move provisioner onto new base
  ◉  ytoqrzxn  martin@…  15m ago  main@origin
  │  refactor: extract provisioner trait
```

**Detail pane — conflict mode:**

When the cursor lands on a conflicted change, the detail pane automatically switches to **conflict mode**. This replaces the normal file list with a conflict-focused layout:

```
┌─────────────────────────────────────────────────────────┐
│ [1] Change Graph  │ [2] Conflicts (3 files)             │
│                   │                                     │
│  ◉ ksqxwpml ⚠3   │  ✗ src/provisioner.rs    (2-way)    │
│  ◉ ytoqrzxn ...   │  ✗ src/retry.rs          (2-way)    │
│  ◉ vlpmrokx ...   │  ✓ src/config.rs         resolved   │
│                   │                                     │
│                   │                                     │
│───────────────────│─────────────────────────────────────│
│ [3] Conflicts: 2 remaining │ Op log: 5 operations       │
└─────────────────────────────────────────────────────────┘
```

The conflict file list shows:

- `✗` for unresolved files, `✓` for resolved files.
- Merge type (2-way, 3-way) from `ConflictInfo::num_sides`.
- Resolved files sort to the bottom; unresolved files are ordered by path.

**Conflict file navigation:**

| Key | Action |
|-----|--------|
| `j` / `k` | Move between conflicted files |
| `Enter` | Open 3-way merge view for selected file (in-TUI, no disk access needed) |
| `e` | Open file in `$EDITOR` (for manual conflict resolution) — **requires working copy** |
| `m` | Open configured external merge tool — **requires working copy** |
| `n` / `N` | Jump to next / previous unresolved file |
| `R` | Refresh conflict state (re-check which files are resolved) |

**Working-copy requirement for filesystem tools:**

`$EDITOR` and external merge tools operate on files on disk. Only the working-copy change (`@`) has its tree materialized. If the user presses `e` or `m` on a change that is *not* the current working copy, lajjzy must switch the working copy first:

1. Emit `Effect::Edit(change_id)` to switch `@` to the selected change.
2. Wait for `MutationComplete` — the graph refreshes, `@` visibly moves.
3. *Then* emit `Effect::SuspendForEditor` or launch the merge tool.

This is a two-step effect sequence. Dispatch emits `Edit` first; when `MutationComplete` arrives, dispatch detects the pending editor launch (stored in `AppState` as deferred intent) and emits the editor effect.

The status bar shows "Switched to ksqxwpml → opening editor…" so the user knows `@` moved. This is fully reversible — `undo` restores the previous working-copy position *and* un-edits the file.

The same rule applies to `e` (open file in `$EDITOR`) from the normal detail pane file list: if the selected change is not `@`, auto-switch first. The 3-way merge view (`Enter`) does *not* require this — it renders conflict data from `ChangeInfo` and `RepoBackend::file_diff()`, which work on any change without disk materialization.

This rule is an invariant: **no `Effect::SuspendForEditor` or external tool launch is ever emitted for a non-working-copy change.** The executor can assert this.

**3-way merge view:**

When `Enter` is pressed on a conflicted file, the detail pane switches to a 3-way merge layout — three vertical columns:

```
┌──────────────────────────────────────────────────────────┐
│ [1] Graph │ [2] Left (ours)  │ [3] Base   │ [4] Right   │
│           │                  │            │  (theirs)    │
│  ◉ ks ⚠3  │  fn provision(   │ fn setup(  │ fn provision(│
│  ◉ yt     │    &self,        │   &self,   │   &self,     │
│           │    host: &Host,  │   host: &H │   host: &Host│
│           │+   retries: u32, │            │              │
│           │  ) -> Result {   │ ) -> Res { │ ) -> Result {│
│───────────│──────────────────│────────────│──────────────│
│ [5] Conflicts: 2 remaining │ src/provisioner.rs (1/3)   │
└──────────────────────────────────────────────────────────┘
```

This is the most complex widget in the TUI and is deliberately deferred to M4. The graph panel narrows to a slim column to make room. Key bindings within the merge view:

| Key | Action |
|-----|--------|
| `j` / `k` | Scroll all three panes in sync |
| `n` / `N` | Jump to next / previous conflict hunk |
| `1` / `2` | Accept left / right for current hunk |
| `Escape` | Exit merge view → back to conflict file list |

**Resolve-and-amend flow:**

When all conflicts in a file are resolved (either via the merge view, `$EDITOR`, or external tool), the file's status flips to `✓`. When *all* files in the change are resolved, the change automatically loses its `⚠` marker — jj detects this on the next snapshot. `lajjzy` calls `load_graph()` after a resolution action to pick up the updated state.

There is no separate "mark resolved" command. Resolution is detected from the file content, not from user declaration. This matches jj's semantics exactly.

### 4.5 Operation Log and Undo

- **`O`** — toggle the operation log as a full-screen overlay, showing the history of every repo mutation.
- **`u`** — undo the last operation (equivalent to `jj op undo`). Repeatable.
- **`Ctrl-R`** — redo (restore an undone operation).
- Each op log entry shows a human-readable summary of what changed.

### 4.6 Revset Bar

One of jj's defining features is its revset query language — a way to express sets of changes like `ancestors(main) & ~immutable()` or `mine() & description(wip)`. `lajjzy` exposes this as a combined search/filter bar.

**Activation:** `/` opens the bar at the bottom of the screen (same position as the bookmark-name input — a one-line input modal).

**Dual-mode input:** The bar accepts either a revset expression or a plain text search string. The backend determines which:

1. The input is passed to `RepoBackend::load_graph(Some(input))`.
2. The `JjBackend` attempts to parse it as a revset. If it parses and evaluates successfully, the graph refilters to show only matching changes.
3. If it fails to parse as a revset, the TUI falls back to client-side fuzzy matching against change descriptions and IDs within the current graph. This means typos and partial strings still work — they just filter the current view rather than querying the repo.

This dual-mode is invisible to the user: they type, the graph updates. Revset syntax is a power-user affordance that's always available but never required.

**Interaction:**

| Key | Action |
|-----|--------|
| `/` | Open revset bar |
| `Enter` | Apply filter (revset or fuzzy match) |
| `Escape` | Close bar, restore previous graph |
| `Ctrl-U` | Clear input |
| `Ctrl-R` | Show revset history (last 10 queries) |

**Live preview (M3+ stretch):** As the user types, the graph updates incrementally. This requires debounced re-evaluation — not needed for M1, where Enter commits the query.

**Graph state:**

`AppState` tracks the current revset filter:

```rust
pub struct AppState {
    // ...
    
    /// The active revset filter, if any. None means "default revset."
    /// Displayed in the status bar so the user knows the view is filtered.
    pub active_revset: Option<String>,
}
```

When a revset is active, the status bar shows it: `revset: mine() & ~empty()`. `Escape` from the revset bar, or pressing `/` and submitting an empty string, clears the filter and restores the default view.

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
│   ├── lajjzy-tui/          # Pure: types, dispatch, rendering. No IO.
│   │   └── src/
│   │       ├── app.rs        # AppState, Action, Effect types
│   │       ├── dispatch.rs   # (AppState, Action) → (AppState, Vec<Effect>) (C3)
│   │       ├── input.rs      # Keymap routing, modal input handling
│   │       ├── panels/       # One module per panel (graph, detail, status, op_log)
│   │       └── widgets/      # Custom ratatui widgets (graph renderer, diff viewer, etc.)
│   └── lajjzy-cli/          # Impure boundary: event loop, executor, terminal, subprocesses
│       └── src/
│           ├── main.rs       # Arg parsing, terminal setup, panic handler
│           ├── event_loop.rs # Poll loop: crossterm input + mpsc channel + dispatch + render
│           └── executor.rs   # Effect executor: RepoBackend calls, thread spawning,
│                             #   $EDITOR launch, merge tool launch. All IO lives here.
├── Cargo.toml                # Workspace root
├── CLAUDE.md                 # C5: from first commit
└── README.md
```

**Rationale:** `lajjzy-tui` contains no IO — no `RepoBackend` calls, no `std::process::Command`, no thread spawning, no terminal operations. It exports pure functions: `dispatch()` and `render()`. The impure boundary — everything that touches the outside world — lives in `lajjzy-cli`. This makes `lajjzy-tui` trivially testable (no mocks for IO) and keeps C1 mechanically enforceable without exceptions.

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
    /// Per-file conflict status. Non-empty only when has_conflict is true.
    /// Loaded eagerly so the detail pane can show conflict state without
    /// a backend round-trip.
    pub conflicts: Vec<ConflictInfo>,
}

pub struct FileChange { /* path, status (M/A/D/R), size delta */ }
pub struct DiffHunk  { /* header, lines, context */ }

/// Per-file conflict metadata, loaded eagerly with the graph.
pub struct ConflictInfo {
    pub path: RepoPath,
    pub num_sides: usize,     // typically 2 (two-way merge)
    pub is_resolved: bool,    // true if conflict markers have been edited away
}

/// The facade trait. Every method returns Result<T, RepoError> (C2).
pub trait RepoBackend: Send + Sync {
    /// Discover and open a jj workspace from a directory.
    fn open_workspace(&self, path: &Path) -> Result<(), RepoError>;

    /// Load the change graph filtered by a revset expression.
    /// All visible changes with parent relationships, per-change file lists,
    /// and per-file conflict status.
    ///
    /// Returns a `GraphSnapshot` which includes the op ID of the repo state
    /// at the time the snapshot was taken. This is used by dispatch to reject
    /// stale snapshots when concurrent operations complete out of order.
    ///
    /// If `revset` is None, uses the default revset (configurable,
    /// defaults to jj's built-in default: the user's working set).
    fn load_graph(&self, revset: Option<&str>) -> Result<GraphSnapshot, RepoError>;

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

/// A versioned graph snapshot. The op_id allows dispatch to detect
/// and discard stale snapshots from out-of-order concurrent operations.
pub struct GraphSnapshot {
    pub changes: Vec<ChangeInfo>,
    /// The jj operation ID at the time this snapshot was taken.
    /// jj's repo lock serialises all mutations, so op IDs are totally
    /// ordered on the main op chain. A snapshot with a higher op_id
    /// always reflects a more recent repo state.
    pub op_id: OpId,
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
  │     └── crossterm
  └── lajjzy-core             (wires up JjBackend as the concrete impl)
        └── jj-lib            (+ jj-lib's transitive deps, including tokio)
```

The key constraint: `lajjzy-tui/Cargo.toml` lists `lajjzy-core` but **not** `jj-lib` or `tokio` (C1). Background work uses `std::thread` + `std::sync::mpsc` — no async runtime in the TUI. The binary crate (`lajjzy-cli`) constructs the concrete `JjBackend` and passes `&dyn RepoBackend` to the TUI.

Key external crates:

| Crate | Role | Depended on by |
|-------|------|----------------|
| `jj-lib` | Repo operations, change model, merge, diff | `lajjzy-core` only |
| `ratatui` | Terminal rendering framework | `lajjzy-tui` |
| `crossterm` | Cross-platform terminal backend | `lajjzy-tui` |
| `tui-textarea` | Multi-line text editing (descriptions) | `lajjzy-tui` |
| `nucleo` / `fuzzy-matcher` | Fuzzy-find for revset bar and pickers | `lajjzy-tui` |
| `similar` | Fallback inline diff if jj-lib's diff API is insufficient | `lajjzy-core` |

Note: `tokio` is a transitive dependency via `jj-lib` — it exists in the build but `lajjzy-tui` does not depend on it directly. Background work in the TUI uses `std::thread` + `std::sync::mpsc`.

### 5.4 Event Loop

The main loop lives in `lajjzy-cli/src/event_loop.rs`. It calls into `lajjzy-tui` for dispatch and rendering (pure functions), and handles all IO itself.

```
┌──────────────────────────────────────────────────────────┐
│              Event Loop (lajjzy-cli)                      │
│                                                          │
│   ┌──────────┐   ┌──────────┐   ┌────────────────┐      │
│   │ Terminal  │   │  Tick    │   │  Background    │      │
│   │  Input    │──▶│  Merge   │◀──│  Task Results  │      │
│   │ (crossterm│   │          │   │  (std mpsc)    │      │
│   └──────────┘   └────┬─────┘   └────────────────┘      │
│                       │                                  │
│                       ▼                                  │
│  ┌─────────────────────────────────────────────────┐     │
│  │  Dispatch (lajjzy-tui, C3)                      │     │
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
│  │ (lajjzy-tui) │     │ (lajjzy-cli)     │               │
│  │  state →     │     │ RepoBackend calls│               │
│  │   frame      │     │ thread::spawn    │               │
│  └──────────────┘     │ $EDITOR launch   │               │
│                       │ merge tool launch│               │
│                       └──────────────────┘               │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

- **Input** is read from crossterm's event stream in the event loop (`lajjzy-cli`).
- **Dispatch** is a pure function exported by `lajjzy-tui`. In M0–M1, it takes a `&dyn RepoBackend` for reads (C3 pragmatic impurity). From M2 onward, it returns `Vec<Effect>` and the effect executor handles all IO.
- **Effect Executor** lives in `lajjzy-cli`. It processes effects: repo mutations via `RepoBackend`, thread spawning, `$EDITOR` launch, merge tool launch. All `std::process::Command` and `std::thread::spawn` calls originate here.
- **Render** is a pure function exported by `lajjzy-tui` — standard immediate-mode ratatui. No IO.

### 5.5 Effect and Action Types (M2+)

Dispatch is a pure function: `(AppState, Action) → (AppState, Vec<Effect>)`. The effect executor consumes effects and feeds results back as actions. This section defines the full vocabulary.

#### Effects (dispatch → executor)

```rust
/// Effects returned by dispatch. The executor handles these;
/// dispatch never sees the executor or the backend.
pub enum Effect {
    // --- Repo mutations (all run on std::thread) ---
    Abandon(ChangeId),
    Squash(ChangeId),
    New { after: ChangeId },
    Edit(ChangeId),
    SetDescription(ChangeId, String),
    BookmarkSet { change: ChangeId, name: String },
    BookmarkDelete(String),
    OpUndo,
    OpRedo,

    // --- Background (std::thread, longer-lived) ---
    GitPush { bookmark: String },
    GitFetch,

    // --- TUI lifecycle ---
    RefreshGraph,
    SuspendForEditor { change: ChangeId, current_text: String },
    LaunchMergeTool { change: ChangeId, path: RepoPath },
    Quit,
}
```

#### Actions (executor → dispatch)

```rust
/// Actions fed back into dispatch from the executor.
/// These are the *only* way external results enter the state machine.
pub enum Action {
    // --- Input ---
    Key(KeyEvent),
    Resize(u16, u16),

    // --- Mutation results (atomic: status + graph in one action) ---
    MutationComplete(MutationResult),
    MutationFailed(OpFailure),

    // --- Background completion (atomic: status + graph in one action) ---
    BackgroundComplete(BackgroundResult),
    BackgroundFailed(BackgroundFailure),

    // --- Editor return ---
    EditorReturned { change: ChangeId, new_text: String },
    EditorCancelled,

    // --- Internal ---
    Tick,
}
```

**Why no separate `GraphLoaded` action:** An earlier design sent `RepoOpSuccess` and `GraphLoaded` as two actions on the channel. This creates a race: dispatch clears `pending_mutation` on `RepoOpSuccess`, but the graph still reflects pre-mutation state until `GraphLoaded` arrives. If the user presses a mutation key between the two (fast typist, slow `load_graph()`), dispatch allows it — against stale change IDs from the old graph. For `abandon` or `squash`, the selected change may no longer exist.

The fix has two layers: (1) `MutationComplete` carries both the status message and the `GraphSnapshot` atomically, so the gate never opens without a graph update in the same state transition. (2) Each `GraphSnapshot` carries an `op_id` from jj's operation log. When concurrent lanes (local mutation, push, fetch) complete out of order, dispatch compares `snapshot.op_id > state.graph_op_id` before applying — a stale snapshot from an earlier repo state is silently discarded, preventing a fetch snapshot from resurrecting changes that a later abandon removed.

#### Result types

```rust
/// Atomic mutation result — carries both the status message
/// and the versioned graph snapshot. Dispatch compares op_ids
/// before applying: a stale snapshot is silently dropped.
pub struct MutationResult {
    pub op: OpKind,
    pub description: String,       // e.g. "Abandoned ksqxwpml"
    pub snapshot: GraphSnapshot,   // versioned graph from load_graph()
}

/// Structured failure — enough context for user-facing error (C6).
/// No graph refresh on failure — the repo state hasn't changed.
pub struct OpFailure {
    pub op: OpKind,
    pub error: RepoError,
}

pub enum OpKind {
    Abandon(ChangeId),
    Squash(ChangeId),
    New(ChangeId),
    Edit(ChangeId),
    Describe(ChangeId),
    BookmarkSet(String),
    BookmarkDelete(String),
    Undo,
    Redo,
}

/// Background results also carry the versioned graph snapshot.
pub enum BackgroundResult {
    PushSuccess { bookmark: String, remote: String, snapshot: GraphSnapshot },
    FetchSuccess { new_changes: usize, snapshot: GraphSnapshot },
}

pub struct BackgroundFailure {
    pub kind: BackgroundKind,
    pub error: RepoError,
}
```

#### Executor cycle

Every repo-mutating effect follows the same cycle: the executor calls the `RepoBackend` method on a spawned thread, then calls `load_graph()`, and sends a single `MutationComplete` action back on the channel. Dispatch compares the snapshot's `op_id` against the current graph's `op_id` before applying it.

```
dispatch(state, Action::Key('d'))
  → (state with pending_mutation set, vec![Effect::Abandon(id)])

executor: std::thread::spawn → backend.abandon(&id)
  → Ok:  let snapshot = backend.load_graph(revset)?;
         tx.send(MutationComplete { op, description, snapshot })
  → Err: tx.send(MutationFailed { op, error })

dispatch(state, Action::MutationComplete(result))
  → if result.snapshot.op_id > state.graph_op_id:
      state.graph = ChangeGraph::from(result.snapshot)
      state.graph_op_id = result.snapshot.op_id
    // else: stale snapshot, silently discard the graph
    //       (a newer snapshot from another lane already landed)
  → state.pending_mutation = None
  → state.notifications.push(result.description)
  → (new_state, vec![])

dispatch(state, Action::MutationFailed(failure))
  → state.pending_mutation = None
  → state.last_error = Some(failure.error)
  → (new_state, vec![])
```

**Out-of-order arrival:** Push, fetch, and local mutations can all call `load_graph()` concurrently. jj's repo lock serialises the mutations themselves, but the threads calling `load_graph()` after their mutation may complete in any order. A fetch snapshot taken at op 5 can arrive *after* an abandon snapshot taken at op 6. Without versioning, the stale fetch snapshot would overwrite the newer abandon snapshot, temporarily resurrecting the abandoned change in the UI.

The fix is simple: `GraphSnapshot` carries the `op_id` of the repo state it was read from. Dispatch only applies a snapshot whose `op_id` is strictly greater than the current `state.graph_op_id`. Stale snapshots are silently discarded — the status message and gate-clearing still apply (the operation *did* succeed), but the graph is not regressed.

Background effects (push, fetch) work identically — `BackgroundComplete` carries a `GraphSnapshot`, and dispatch applies the same `op_id` comparison before updating the graph.

### 5.6 Mutation UX Patterns

Every M2 mutation falls into one of three interaction patterns. New mutations in M3+ must declare which pattern they use — introducing a new pattern is a design decision, not an implementation detail.

| Pattern | Trigger | UI during | Status bar after | Examples |
|---------|---------|-----------|------------------|----------|
| **Instant** | Single keypress | Graph rebuilds immediately | "Abandoned ksqxwpml" | abandon, squash, new, edit, undo, redo, bookmark delete |
| **Mini-modal** | Keypress → input → confirm | One-line input or text editor overlay | "Updated description for ksqxwpml" | describe (tui-textarea), bookmark set (name input) |
| **Background** | Keypress → spinner → callback | Spinner in status bar, UI responsive | "Pushed main → origin" | push, fetch |

**No confirmation dialogs.** jj's operation log makes every local mutation non-destructive — `undo` reverses any of them. This eliminates "are you sure?" prompts, which is a deliberate departure from lazygit's git-shaped caution.

**Per-operation detail:**

- **abandon** (`d`) — Instant. Change removed from graph. Cursor moves to parent. "Abandoned ksqxwpml."
- **squash** (`Shift-S`) — Instant. Change disappears, parent absorbs content. Cursor moves to parent. "Squashed ksqxwpml into ytoqrzxn."
- **new** (`n`) — Instant. New empty change created after selected. Cursor moves to new change. No description prompt — hit `e` to describe. "Created new change after ytoqrzxn."
- **edit** (`Ctrl-E`) — Instant. Working-copy marker (`@`) moves. "Now editing ksqxwpml."
- **undo** (`u`) / **redo** (`Ctrl-R`) — Instant. Graph rebuilds from op log. "Undid: abandon ksqxwpml."
- **bookmark set** (`B`) — Mini-modal. One-line input at bottom of screen, pre-filled if bookmark exists, with fuzzy completion against existing names. Enter confirms, Escape cancels.
- **bookmark delete** — Instant (from bookmark picker: `b` → select → `d`). "Deleted bookmark main."
- **describe** (`e`) — Mini-modal. `tui-textarea` overlay replaces detail pane content (maintains spatial context). Pre-filled with current description. `Ctrl-S` saves, `Escape` discards, `Shift-E` escalates to `$EDITOR`.
- **push** (`P`) — Background. "Pushing…" spinner. On success: "Pushed main → origin." On failure: error in status bar (C6). Note: `undo` after push undoes the local op log entry but cannot un-push from the remote — status bar notes "Undid push (remote unchanged)."
- **fetch** (`f`) — Background. "Fetching…" spinner. On completion: "Fetched 3 new changes from origin." If fetch surfaces conflicts, they appear in the graph immediately.

### 5.7 Mutation Gating

All mutations run on `std::thread` — no inline execution, uniform code path. This means concurrent mutations are possible at the thread level. The state machine prevents this with gating fields in `AppState`.

Three independent lanes, never blocking each other:

| Lane | Gate field | Concurrent with |
|------|-----------|-----------------|
| Local mutations | `pending_mutation: Option<OpKind>` | Push, Fetch |
| Push | `pending_background: HashSet{Push}` | Fetch, local mutations |
| Fetch | `pending_background: HashSet{Fetch}` | Push, local mutations |

**Dispatch logic:** When a mutation-triggering key is pressed and the corresponding gate is occupied, dispatch swallows the keypress and optionally flashes "Operation in progress…" in the status bar. Navigation, rendering, and ungated lanes remain responsive.

```rust
// Local mutation — gated by pending_mutation only
Action::Key('d') if state.pending_mutation.is_some() => {
    (state, vec![])  // swallow
}

// Fetch — gated independently, does not check pending_mutation
Action::Key('f') if state.pending_background.contains(&Fetch) => {
    (state, vec![])  // already fetching
}
```

On completion, the corresponding gate clears. The graph is only updated if the snapshot is newer than what's currently displayed:

```rust
Action::MutationComplete(result) => {
    if result.snapshot.op_id > state.graph_op_id {
        new_state.graph = ChangeGraph::from(result.snapshot);
        new_state.graph_op_id = result.snapshot.op_id;
    }
    // Gate always clears and status always shows,
    // even if graph was stale (the op did succeed).
    new_state.pending_mutation = None;
    new_state.notifications.push_back(/* ... */);
}
Action::MutationFailed(failure) => {
    new_state.pending_mutation = None;
    new_state.last_error = Some(failure.error);
}
Action::BackgroundComplete(FetchSuccess { snapshot, .. }) => {
    if snapshot.op_id > state.graph_op_id {
        new_state.graph = ChangeGraph::from(snapshot);
        new_state.graph_op_id = snapshot.op_id;
    }
    new_state.pending_background.remove(&Fetch);
}
```

**Why push and fetch are independent of local mutations:** `jj-lib` serialises on its repo lock — concurrent calls won't corrupt anything. When multiple lanes complete, each `BackgroundComplete` or `MutationComplete` carries a graph snapshot taken at the moment `load_graph()` ran; the last one to be processed by dispatch wins and always reflects the most complete state. Blocking fetch during a local mutation would mean "sorry, can't check for remote changes because you're abandoning a change." That's hostile.

### 5.8 $EDITOR and External Tool Materialization

#### Working-copy invariant

`$EDITOR` and external merge tools operate on files on disk. Only the working-copy change (`@`) has its tree materialized. **No `Effect::SuspendForEditor` or external tool launch is ever emitted for a non-working-copy change.** The executor can assert this.

When the user presses `e` (open in editor) or `m` (merge tool) on a change that is not `@`, dispatch executes a two-step sequence:

```
// User presses 'e' on file in non-@ change ksqxwpml
dispatch(state, Action::Key('e'))
  → state.deferred_intent = Some(DeferredIntent::OpenEditor { path })
  → (state, vec![Effect::Edit(ksqxwpml)])  // switch @ first

// Edit completes, @ is now on ksqxwpml
dispatch(state, Action::MutationComplete(result))
  → state.graph = result.graph
  → state.pending_mutation = None
  → // Check deferred_intent:
  → state.deferred_intent = None
  → (state, vec![Effect::SuspendForEditor { ... }])  // now safe

// Status bar: "Switched to ksqxwpml → opening editor…"
```

If the `Edit` fails (e.g. change has been abandoned), `MutationFailed` clears both the gate and `deferred_intent`, and shows the error. The editor never opens.

This does not apply to the 3-way merge view (`Enter` in the conflict file list), which renders from `ChangeInfo` and `RepoBackend::file_diff()` — no disk access needed, works on any change.

#### Describe modal ($EDITOR escalation)

The `Shift-E` escalation from the describe modal is different — it edits a *tempfile* containing the description text, not a working-tree file. No working-copy switch is needed.

```
dispatch(state, Action::Key('E'))  // Shift-E from within describe modal
  → (state with modal closed, vec![Effect::SuspendForEditor { change, current_text }])

executor receives SuspendForEditor:
  1. Writes current_text to a tempfile.
  2. Suspends TUI (ratatui LeaveAlternateScreen).
  3. Runs $EDITOR on tempfile via std::process::Command. Blocks.
  4. Reads tempfile contents on editor exit.
  5. Resumes TUI (EnterAlternateScreen).
  6. Sends Action::EditorReturned { change, new_text }.

dispatch(state, Action::EditorReturned { change, new_text })
  → (state with describe modal re-opened, pre-filled with new_text, vec![])
  // User reviews editor output, then Ctrl-S to save or Escape to discard.
```

**C1 compliance:** `$EDITOR` and merge tool launches are `Effect` variants executed by `lajjzy-cli/src/executor.rs`, not by `lajjzy-tui`. Dispatch emits `Effect::SuspendForEditor` or `Effect::LaunchMergeTool`; the executor in `lajjzy-cli` handles `std::process::Command`, TUI suspend/resume, and tempfile management. No subprocess code exists in `lajjzy-tui`.

**Note:** The `RepoBackend` lock is *not* held during editing. The editor edits a tempfile (describe) or the working tree (conflict resolution); `Effect::SetDescription` or graph refresh fires only after the user finishes. No lock contention.

### 5.9 Repo Access Model

`jj-lib` requires careful handling: repo operations take a mutable lock, and some (like `rebase`) can be expensive.

- **Read path:** The graph view is built from `RepoBackend::load_graph()`, which returns a `GraphSnapshot` — the visible changes *with their file lists* plus the `op_id` of the repo state at read time. Cursor navigation never triggers a backend call. Internally, `load_graph()` takes a read-only snapshot — cheap thanks to jj's copy-on-write store. The only lazy read is `file_diff()`, called when the user drills into a specific file's hunks.
- **Write path:** Mutations go through `RepoBackend` methods, which internally acquire the repo lock, perform the operation, record an op, and return. The executor always calls `load_graph()` after a successful mutation and packages the result into a `MutationComplete` action. Dispatch compares the snapshot's `op_id` against `state.graph_op_id` — stale out-of-order snapshots are silently discarded, ensuring the graph never regresses to an older state.
- **Background tasks:** Fetch/push run on a spawned `std::thread` with results sent back via `mpsc`. Each result carries a `GraphSnapshot` with its `op_id`, subject to the same freshness check on arrival.

### 5.10 Forge Integration (Gerrit / GitHub / GitLab)

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
| `/` | Revset bar (revset expression or fuzzy-find) |
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
| `e` | Open file in `$EDITOR` — auto-switches working copy if change is not `@` (see §4.4) |
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

    /// The op_id of the repo state that produced the current graph.
    /// Dispatch only applies a GraphSnapshot whose op_id is strictly
    /// greater than this — stale out-of-order snapshots are discarded.
    pub graph_op_id: OpId,

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

    /// Active revset filter. None = default revset. Displayed in status bar
    /// so the user knows the view is filtered.
    pub active_revset: Option<String>,

    /// Gates local repo mutations — at most one in flight.
    /// Navigation and background ops are unaffected.
    pub pending_mutation: Option<OpKind>,

    /// Gates background operations independently. Push and fetch can
    /// each be in flight simultaneously, but not two pushes or two fetches.
    pub pending_background: HashSet<BackgroundKind>,

    /// Deferred intent: an action to perform after the current mutation
    /// completes. Used for two-step sequences like "auto-edit then open
    /// editor" — dispatch emits Edit first, and when MutationComplete
    /// arrives, checks this field to emit the follow-up effect.
    pub deferred_intent: Option<DeferredIntent>,
}
```

```rust
pub enum DeferredIntent {
    /// After switching working copy, open this file in $EDITOR.
    OpenEditor { path: RepoPath },
    /// After switching working copy, open this file in the merge tool.
    OpenMergeTool { path: RepoPath },
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
  - C1 enforcement: `lajjzy-tui/Cargo.toml` does not contain `jj-lib`; `grep -r 'std::process::Command' crates/lajjzy-tui/` returns nothing; `grep -r 'std::thread::spawn' crates/lajjzy-tui/` returns nothing.
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
- Revset bar (`/`): accepts revset expressions or falls back to fuzzy matching on descriptions/IDs. Calls `load_graph(Some(input))` on Enter. Status bar shows active filter.
- Conflict mode in detail pane: when cursor lands on a conflicted change, the detail pane automatically switches to show the conflict file list (from eager `ChangeInfo::conflicts`). No merge view yet — that's M4.
- Status bar shows error context from failed backend calls (C6).

### M2 — Core Mutations (weeks 5–7)

**Gate:** C4 — jj-lib API audit completed and committed before design work begins.

**Constraints active:** all M0/M1 constraints + C3 (dispatch becomes pure), C4.

- Dispatch refactored: `(AppState, Action) → (AppState, Vec<Effect>)` (C3).
- Effect executor in `lajjzy-cli`: `std::thread::spawn` + `mpsc` for all mutations. Three gating lanes: `pending_mutation`, `pending_background{Push}`, `pending_background{Fetch}`.
- `RepoBackend` extended with mutation methods (signatures validated by C4 audit).
- Instant mutations: `describe`, `new`, `edit`, `abandon`, `squash` (full change only), `undo`, `redo`, `bookmark set`, `bookmark delete`.
- Background mutations: `git push`, `git fetch` (proves the async effect path under real network latency).
- Describe UX: inline `tui-textarea` modal with `Shift-E` escalation to `$EDITOR`. Editor launch handled by executor in `lajjzy-cli` (C1 stays clean).
- Inline status notifications for success and failure (C6).
- No confirmation dialogs — undo is the safety net.

**Not in M2 scope:** `split` (needs interactive hunk picker widget), `rebase` (needs target picker modal), partial squash (needs hunk selection). These require new UI primitives and belong in M3.

### M3 — Stack Workflows (weeks 8–10)

- Stack detection and visual grouping in graph view.
- Stack mode (`S`): zoomed view of a single stack with reorder (`Shift-J`/`Shift-K`), bulk squash, bulk rebase.
- Rebase with target picker modal (fuzzy-find change to rebase onto).
- Split with interactive hunk picker (the hardest widget — select hunks to extract into a new change).
- Partial squash (squash selected files/hunks, not the whole change).
- Push stack to remote (git-native, no forge API yet).

### M4 — Conflict Handling (weeks 11–12)

- Conflict file navigation in detail pane: `j`/`k` between files, `n`/`N` to jump between unresolved files, `R` to refresh resolution state.
- 3-way merge view widget: three-column layout (left/base/right) with synchronised scrolling, hunk-level conflict navigation (`n`/`N`), and accept-left/accept-right (`1`/`2`) per hunk. Does *not* require working copy — renders from backend data.
- External merge tool launch (`m` from conflict file list) and `$EDITOR` escape hatch (`e`): both require working-copy materialization. If the conflicted change is not `@`, dispatch auto-switches via `Effect::Edit` with `deferred_intent` before opening the tool (see §5.8).
- Resolve-and-amend flow: resolution detected from file content (no "mark resolved" command), graph refreshes automatically via `load_graph()`.
- Graph narrows to slim column when merge view is active, restores on `Escape`.

### M5 — Forge Integration: Foundations (weeks 13–16)

- `Forge` trait defined in `lajjzy-core` with implementations for Gerrit, GitHub, GitLab.
- Forge detection from remote URL patterns, manual override via config.
- Auth: delegate to `ssh-agent` (Gerrit), `gh auth` (GitHub), `glab auth` (GitLab). No credential management in lajjzy.
- Review status annotations in graph view (per-change badges: pending, approved, changes-requested).
- Push-for-review via `Forge::push_for_review()`: Gerrit implementation uses `jj gerrit upload` semantics (which handles Change-Id footer insertion automatically from the jj change ID); GitHub creates one PR per stack change.
- Review comments in detail pane (read-only, stretch).

### M6 — Polish and Release (ongoing)

- Configurable keymap (TOML/YAML).
- Theming (base16 / terminal colours).
- Mouse support (optional, not primary input).
- Packaging: `cargo install`, AUR, Homebrew, Nix flake.
- Effect cancellation: `TaskHandle` with `cancel()` method, `Effect::Cancel(TaskId)` for long-running background tasks.
- Live revset preview (debounced graph update as user types in revset bar).

### M7 — Parallel Branches (git-butler model)

**Prerequisite:** M3 (stack workflows) and tree-shaped stack support resolved as an open question.

jj's DAG natively supports multiple concurrent lines of work branching off trunk. What it lacks — and what git-butler provides as a GUI — is a visual metaphor for managing them simultaneously. M7 adds this.

- **Lane view:** A horizontal split showing parallel branches as vertical lanes (like kanban columns). Each lane is a bookmark or stack rooted on trunk. The user sees all their active work at once, not just one stack.
- **Hunk-level move between lanes:** Select hunks in the working-copy change and assign them to different lanes. This is jj's `jj split` + `jj squash` composed into a drag-and-drop interaction. The hardest UX problem in this milestone — it needs a hunk picker that shows the target lane, not just "new change."
- **Cross-lane conflict detection:** Background task that checks whether any pair of active lanes would conflict if merged into trunk. Surfaces early warnings in the lane view: "Lane A and Lane B both modify src/config.rs." This is a `load_graph()` call with a synthetic merge revset, run periodically on a background thread.
- **Lane composition for testing:** Temporarily merge selected lanes into a synthetic working-copy change to test them together. This is `jj new lane_a lane_b` — a multi-parent change. Clearly marked as ephemeral; abandon it when done.

**What this is NOT:** git-butler's "virtual branches" are an overlay on top of git. lajjzy's lanes are a *view* of jj's native DAG — no custom metadata, no shadow state. Every operation is a standard jj operation (new, squash, split, rebase), composed and visualised in a way that makes parallel work feel natural.

### M8 — Gerrit Depth

**Prerequisite:** M5 (forge foundations, Gerrit implementation).

M5 establishes connectivity to Gerrit. M8 adds the workflow depth that makes lajjzy a serious Gerrit client.

- **Patchset comparison:** Gerrit tracks versions of the same change (patchsets). When viewing a change, show a patchset selector: "v1 → v2 → v3 (current)." Selecting two patchsets shows their interdiff — what changed between review rounds. This requires a Gerrit REST API call (`/changes/{id}/revisions`) and a diff view that understands "patchset A vs patchset B" rather than "change vs parent."
- **Review actions:** Submit a review score from within lajjzy: Code-Review +1/+2, Verified +1, Submit. These are Gerrit REST API calls. Displayed as a mini-modal: select a label, select a score, optional comment, confirm.
- **Inline comments:** View review comments positioned next to the relevant diff hunks. Write new comments from the diff view. Comment threads with reply support.
- **Topic support:** Gerrit topics group related changes across repos. Show topic membership in the graph view; push-for-review assigns a topic.
- **Change-Id mapping:** As of jj 0.30, jj writes a `change-id` header into every Git commit object by default (`git.write-change-id-header = true`). jj, GitButler, and Gerrit have agreed on a shared format: a 32-character reverse-hex ID stored as a Git commit header. `jj gerrit upload` already bridges the gap today by adding a Gerrit-style `Change-Id` footer derived from the jj change ID. Gerrit has an accepted design doc for native header support, pending upstream Git adoption of the header (Git currently strips unknown headers on rewrite operations like `git rebase`). For lajjzy, this means: use `jj gerrit upload` semantics (or the equivalent jj-lib API) rather than inventing a custom mapping. The graph view can show Gerrit review state by looking up the change via its jj change ID → Gerrit Change-Id mapping, which `jj gerrit upload` maintains automatically.

### M9 — GitHub/GitLab Stacked PRs (Graphite model)

**Prerequisite:** M5 (forge foundations, GitHub/GitLab implementation), M3 (stack workflows).

M5 establishes basic PR creation. M9 adds the Graphite-style stack-aware workflow.

- **Stack-aware PR creation:** `Push stack` creates one PR per change in the stack, with each PR targeting the previous PR's branch (not main). PR descriptions are auto-generated from change descriptions, with a stack overview table injected at the top: "This is change 2/4 in a stack. ← parent PR | child PR →."
- **Automatic restacking:** When a change in the middle of a stack is amended, jj automatically rebases descendants. M9 detects this and force-pushes all affected PR branches, updating PR descriptions to reflect the new stack state.
- **Stack submission:** A single "submit stack" action that merges PRs bottom-up: merge the base PR, wait for CI, merge the next, etc. This requires polling the forge API for merge status. Displayed as a multi-step progress view in the status bar.
- **Stack-aware review navigation:** When viewing a stack, show per-change PR status (draft, review-requested, approved, merged). Navigate to the next change needing review with a single keypress.
- **PR interdiff:** Like Gerrit's patchset comparison — show what changed between force-pushes of the same PR. The shared `change-id` header (now written by jj by default) means forges can track change identity across force-pushes natively as they adopt the header. Until then, lajjzy can compute interdiffs from jj's local evolution log (`jj evolog`), which tracks all previous versions of a change regardless of forge support.

**Out of scope:** Merge queues are a forge-side feature (GitHub's merge queue, Graphite's merge queue service). lajjzy can *trigger* a merge queue entry by merging a PR, but the queue itself is not implemented in the TUI.

---

## 10. Open Questions

1. **jj-lib stability.** `jj-lib` doesn't promise a stable API yet. Strategy: pin to a specific jj release per `lajjzy` release; the `RepoBackend` trait absorbs API churn so only `jj_backend.rs` needs updating.
2. **Working copy integration.** jj snapshots the working copy on every operation. How much of this should `lajjzy` surface vs. keep implicit? Lazygit shows the working tree diff prominently; jj's model is different — there's always a "working-copy change" that auto-snapshots.
3. **Tree-shaped stacks.** M3 starts with linear stacks. M7 (parallel branches) requires tree-shaped stack support. When does the graph widget learn to render and navigate branching stacks? Early investment (M3) vs. deferred complexity (M7)?
4. **Forge auth.** M5 delegates to `ssh-agent`/`gh`/`glab`. Is this sufficient for CI/headless environments, or does lajjzy eventually need its own token management?
5. **Backend swappability.** The `RepoBackend` trait opens the door to non-jj backends (e.g. raw git via `git2`). Is this a goal or an accidental affordance? For now, treat it as an accidental affordance — the trait exists for testability and facade isolation, not as a plugin system.
6. **Lane view layout (M7).** Horizontal lanes (columns) or vertical lanes (rows)? Horizontal is the git-butler metaphor but constrains terminal width. Vertical stacks more naturally in a TUI. Needs prototyping.
7. **Interdiff computation (M8/M9).** Patchset comparison requires diffing two versions of the same change. jj's `evolog` tracks local evolution; Gerrit's REST API exposes patchset diffs; GitHub has no native equivalent yet. For the local case, `jj-lib` likely exposes enough via evolog to compute interdiffs without forge API calls. The C4 audit should verify this if M8 is in scope.
8. **Stack PR branch naming (M9).** What naming scheme for the one-branch-per-change model? Graphite uses `gt/user/stack-name/N`. lajjzy needs a convention that doesn't collide with user bookmarks and is recognisable as machine-managed.
9. **Git header adoption timeline.** The `change-id` header is written by jj and agreed upon by jj/GitButler/Gerrit, but Git itself doesn't preserve unknown headers on rewrite operations yet. Some forges (e.g. Harness) fail on the header entirely. This affects M9 (GitHub/GitLab stacked PRs): until forges read the header, interdiff and change-tracking across force-pushes must fall back to local computation via `jj evolog`. Monitor the Git mailing list thread and forge adoption.

### Resolved Questions

- ~~**Naming.**~~ `lajjzy`.
- ~~**Async runtime.**~~ `std::thread` + `std::sync::mpsc`. No tokio in the TUI.
- ~~**M2 scope.**~~ Describe, new, edit, abandon, squash, undo/redo, bookmark set/delete, push, fetch. Dispatch becomes pure. Effect executor in `lajjzy-cli` with three gating lanes.
- ~~**Describe UX.**~~ Inline `tui-textarea` with `Shift-E` escalation to `$EDITOR`. Editor launch is an `Effect` handled by the executor in `lajjzy-cli` (C1: no exceptions).
- ~~**Eager vs lazy loading.**~~ File lists eager (in `load_graph()`), hunk diffs lazy (`file_diff()`).
- ~~**Gerrit Change-Id mapping (M8).**~~ Solved by the jj/GitButler/Gerrit standardization effort. As of jj 0.30, jj writes a `change-id` header into Git commit objects by default. `jj gerrit upload` handles the jj-change-id → Gerrit-Change-Id footer mapping automatically. Gerrit has an accepted design doc for native header support, pending upstream Git adoption. lajjzy should use `jj gerrit upload` semantics, not invent a custom mapping.
### Why There Is No "M7: jj Branchless Workflow" Milestone

An earlier draft proposed a milestone for "jj's branchless and safe history editing" covering automatic commits, stable Change IDs, first-class conflicts, and operation log undo. This was absorbed into M0–M4 because these are not features to *add* — they are jj's native semantics that the TUI *already surfaces*:

- Automatic commits / no staging area → the working-copy change is visible in the graph from M0. There is no "add" or "stage" operation to implement.
- Stable Change IDs → the graph uses change IDs as the primary identifier from M0. This is the data model, not a feature.
- First-class conflicts → M4 implements conflict navigation and resolution UI.
- Operation log and undo → M2 implements undo/redo as instant actions.

A separate milestone for these would be a restatement of work already scoped. The genuinely new workflow that extends beyond jj's CLI is M7 (parallel branches), which adds a *visual metaphor* for capabilities jj already has but that are hard to use without spatial navigation.

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
