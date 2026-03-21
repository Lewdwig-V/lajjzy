# M2: Elm-Style State Transitions + Core Mutations

**Date:** 2026-03-21
**Status:** Draft
**Depends on:** M1b (complete), tilde glyph fix (committed, not yet merged)

## Motivation

M0-M1 built a read-only TUI with pragmatic impurities: `dispatch()` calls `backend.load_graph()`, `backend.file_diff()`, and `backend.op_log()` directly. M2 makes dispatch pure and adds the first jj mutations, turning lajjzy from a viewer into a usable daily-driver tool.

The effect system and mutations ship together because a refactored dispatch without mutations is untestable in the ways that matter. The mutations *are* the test of whether the architecture works.

## Scope

### In scope (M2)

- Pure `dispatch()` returning `Vec<Effect>` — no backend parameter
- Effect executor with `std::thread::spawn` + `mpsc` channel
- Poll-based event loop replacing synchronous dispatch
- 10 jj mutations (see Mutations section)
- `app.rs` decomposition into focused modules
- `tui-textarea` integration for describe modal
- `$EDITOR` suspend/resume for long-form editing

### Out of scope (M3+)

Operations requiring new UI primitives:
- `rebase` (needs target picker)
- `split` (needs interactive hunk selection)
- Partial squash (needs hunk picker)
- `bookmark move`
- Stack-aware bulk operations

## Effect System Architecture

### Core Types

Defined in `lajjzy-tui`, never executed there:

```rust
enum Effect {
    // Read-only
    LoadGraph,
    LoadOpLog,
    LoadFileDiff { change_id: String, path: String },

    // Mutations
    Describe { change_id: String, text: String },
    New { after: String },
    Edit { change_id: String },
    Abandon { change_id: String },
    Squash { change_id: String },
    Undo,
    Redo,
    BookmarkSet { change_id: String, name: String },
    BookmarkDelete { name: String },
    GitPush { bookmark: String },
    GitFetch,

    // Non-repo
    SuspendForEditor { change_id: String, initial_text: String },
}
```

Result actions added to the `Action` enum:

```rust
enum Action {
    // ... existing UI actions unchanged ...

    // Effect results
    GraphLoaded(Result<GraphData>),
    OpLogLoaded(Result<Vec<OpLogEntry>>),
    FileDiffLoaded(Result<Vec<DiffHunk>>),
    RepoOpSuccess { op: MutationKind, message: String },
    RepoOpFailed { op: MutationKind, error: String },
    EditorComplete { change_id: String, text: String },

    // BookmarkInput modal actions (analogous to FuzzyInput/FuzzyBackspace)
    BookmarkInputChar(char),
    BookmarkInputBackspace,
    BookmarkInputConfirm,
}

enum MutationKind {
    Describe, New, Edit, Abandon, Squash,
    Undo, Redo, BookmarkSet, BookmarkDelete,
    GitPush, GitFetch,
}
```

### Dispatch Signature

```rust
// Before (M0-M1):
fn dispatch(state: &mut AppState, action: Action, backend: &dyn RepoBackend)

// After (M2):
fn dispatch(state: &mut AppState, action: Action) -> Vec<Effect>
```

Dispatch is a pure function. It never performs I/O, never calls backend methods, never panics on repo operations. All repo interaction flows through the returned `Vec<Effect>`.

### Executor

Lives in `lajjzy-cli`. Owns the backend and the channel sender.

```rust
struct EffectExecutor {
    backend: Arc<JjCliBackend>,
    tx: mpsc::Sender<Action>,
}
```

**`Arc<JjCliBackend>` is safe for concurrent use.** `JjCliBackend` holds only an immutable `PathBuf` (workspace root) and all trait methods take `&self`. Each method spawns a separate `jj` subprocess — concurrent calls from multiple threads are independent OS processes, serialised by jj's own repo lock. No mutable state, no `Mutex` needed.

**Every mutation runs on a thread.** No inline fast path for "probably quick" operations. The reason isn't latency speculation — it's that operation cost depends on repo state (`abandon` on a stack base triggers automatic rebasing of all descendants). One code path, uniform execution, no surprise jank.

**Mutation effects always refresh the graph.** After a successful mutation, the executor calls `load_graph()` and sends both `RepoOpSuccess` and `GraphLoaded` into the channel. Dispatch never does surgery on its graph model — it replaces the whole thing. Simple, impossible to get into an inconsistent state.

**Batched result delivery.** The executor sends `RepoOpSuccess` + `GraphLoaded` in sequence on the same channel. The event loop drains all pending actions before the next render, so both are processed in the same frame. No intermediate "graph stale but status bar says success" state.

## Concurrency Model

### Three Independent Lanes

Every mutation fires on a background thread. Concurrency is controlled by state-machine gates, not locks.

| Lane | Gate | Concurrent with |
|------|------|-----------------|
| Local mutations | `pending_mutation: Option<MutationKind>` | Fetch, Push |
| Push | `pending_background{Push}` | Fetch, local mutations |
| Fetch | `pending_background{Fetch}` | Push, local mutations |

```rust
pub struct AppState {
    // ...
    pub pending_mutation: Option<MutationKind>,
    pub pending_background: HashSet<BackgroundKind>,
}

#[derive(Hash, Eq, PartialEq)]
pub enum BackgroundKind { Push, Fetch }
```

### Why Three Lanes

- **Local mutation + fetch:** User abandons a change, then fetches. Both complete (serialised by jj's repo lock). The second `GraphLoaded` reflects both operations. Blocking fetch during a local mutation would be hostile UX.
- **Local mutation + push:** Same reasoning. Push operates on remote state, local mutation on local state.
- **Push + fetch:** Independent operations on independent state. No conflict.
- **Push + push:** Genuine conflict on remote ref updates. Gated by `pending_background{Push}`.
- **Fetch + fetch:** Wasteful. Gated by `pending_background{Fetch}`.
- **Last `GraphLoaded` wins.** No ordering dependency. Each `GraphLoaded` reflects complete repo state at time of `load_graph()` call.

### Gate Behaviour

Dispatch checks the gate before emitting mutation effects. If gated, the keypress is swallowed — navigation remains responsive.

```rust
// Local mutation gating:
if state.pending_mutation.is_some() {
    return vec![]; // suppressed
}
state.pending_mutation = Some(MutationKind::Abandon);
vec![Effect::Abandon { change_id }]

// On success — clear the appropriate gate:
Action::RepoOpSuccess { op, message } => {
    match op {
        MutationKind::GitPush => { state.pending_background.remove(&BackgroundKind::Push); }
        MutationKind::GitFetch => { state.pending_background.remove(&BackgroundKind::Fetch); }
        _ => { state.pending_mutation = None; }
    }
    state.status_message = Some(message);
    vec![]
}

// On failure — also clear the gate, so the user isn't locked out:
Action::RepoOpFailed { op, error } => {
    match op {
        MutationKind::GitPush => { state.pending_background.remove(&BackgroundKind::Push); }
        MutationKind::GitFetch => { state.pending_background.remove(&BackgroundKind::Fetch); }
        _ => { state.pending_mutation = None; }
    }
    state.error = Some(error);
    vec![]
}
```

## Event Loop

Poll loop in `lajjzy-cli/src/main.rs`:

```rust
fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    state: &mut AppState,
    executor: &EffectExecutor,
    rx: &mpsc::Receiver<Action>,
) -> Result<()> {
    loop {
        terminal.draw(|frame| render(frame, state))?;

        if crossterm::event::poll(Duration::from_millis(50))? {
            if let Event::Key(key_event) = event::read()? {
                if key_event.kind != KeyEventKind::Press {
                    continue;
                }
                // Clear transient status on any keypress
                state.status_message = None;
                if let Some(action) = map_input(key_event, state) {
                    let effects = dispatch(state, action);
                    execute_effects(terminal, state, executor, &effects);
                }
            }
        }

        // Drain all pending results before next render
        while let Ok(action) = rx.try_recv() {
            let effects = dispatch(state, action);
            execute_effects(terminal, state, executor, &effects);
        }

        if state.should_quit {
            break;
        }
    }
    Ok(())
}
```

- **50ms poll timeout:** 20fps for spinner animation when idle. Immediate return when user is typing.
- **Drain loop:** All channel actions processed before render. Ensures batched results appear in the same frame.
- **Status message clearing:** `status_message` is cleared on any user keypress (before dispatch), not on channel actions. Background completions that set a status message persist until the next keypress.

**`SuspendForEditor` routing.** This effect is intercepted by `execute_effects` before reaching the executor, because it needs the terminal:

```rust
fn execute_effects(
    terminal: &mut ratatui::DefaultTerminal,
    state: &mut AppState,
    executor: &EffectExecutor,
    effects: &[Effect],
) {
    for effect in effects {
        match effect {
            Effect::SuspendForEditor { change_id, initial_text } => {
                ratatui::restore();
                let result = run_editor(initial_text);
                *terminal = ratatui::init();
                match result {
                    Ok(text) => {
                        let effects = dispatch(state, Action::EditorComplete {
                            change_id: change_id.clone(),
                            text,
                        });
                        execute_effects(terminal, state, executor, &effects);
                    }
                    Err(e) => {
                        state.error = Some(format!("Editor failed: {e}"));
                    }
                }
            }
            other => executor.execute(other.clone()),
        }
    }
}
```

The event loop and drain loop both call `execute_effects` instead of the executor directly. This ensures `SuspendForEditor` is always handled correctly regardless of whether it comes from a user keypress or a channel action.

## RepoBackend Trait Extension

```rust
pub trait RepoBackend: Send + Sync {
    // Existing
    fn load_graph(&self) -> Result<GraphData>;
    fn file_diff(&self, change_id: &str, path: &str) -> Result<Vec<DiffHunk>>;
    fn op_log(&self) -> Result<Vec<OpLogEntry>>;

    // M2 mutations — all return human-readable status message
    fn describe(&self, change_id: &str, text: &str) -> Result<String>;
    fn new_change(&self, after: &str) -> Result<String>;
    fn edit_change(&self, change_id: &str) -> Result<String>;
    fn abandon(&self, change_id: &str) -> Result<String>;
    fn squash(&self, change_id: &str) -> Result<String>;
    fn undo(&self) -> Result<String>;
    fn redo(&self) -> Result<String>;
    fn bookmark_set(&self, change_id: &str, name: &str) -> Result<String>;
    fn bookmark_delete(&self, name: &str) -> Result<String>;
    fn git_push(&self, bookmark: &str) -> Result<String>;
    fn git_fetch(&self) -> Result<String>;
}
```

Every mutation returns `Result<String>`. The string is jj's human-readable output for the status bar. The executor uses it for `RepoOpSuccess.message`.

Cursor positioning after mutations relies on `GraphLoaded` + `working_copy_index`. No structured return types needed.

**`GraphLoaded` cursor repositioning strategy** (single code path for all mutations):

1. Save `state.selected_change_id()` before replacing the graph.
2. Replace `state.graph` with the new graph.
3. Try to find the saved change ID in the new graph's node indices. If found, cursor stays on it.
4. If not found (change was abandoned, squashed away), fall back to `working_copy_index`.
5. If no working copy, fall back to first node index.
6. Call `state.reset_detail()`.

This is correct for all 10 mutations:
- **new:** Special case — after `jj new`, the new change becomes the working copy. Dispatch sets `cursor_follows_working_copy = true` before emitting the effect. When `GraphLoaded` arrives, if this flag is set, skip step 3 (find old change ID) and go directly to step 4 (working copy). This moves the cursor to the new change, matching the expected UX. The flag is cleared after use.
- **edit:** Old change still exists, cursor stays on it. The `@` marker moves to reflect the new working copy.
- **abandon:** Old change ID gone, falls back to working copy (which jj moves to the parent).
- **squash:** Old change ID gone (squashed into parent), falls back to working copy.
- **describe, bookmark set/delete, undo, redo, push, fetch:** Old change still exists, cursor stays on it.

## Mutations

### Interaction Patterns

| Pattern | Trigger | UI | Examples |
|---------|---------|-----|----------|
| **Instant** | Single keypress | Graph updates, status bar confirms | abandon, squash, undo, redo, edit, new, bookmark delete |
| **Mini-modal** | Keypress then input then confirm | Input field, enter confirms | bookmark set (name), describe (text editor) |
| **Background** | Keypress then spinner then callback | Status bar spinner, graph rebuilds on completion | push, fetch |

Every mutation in M3+ must declare which slot it fits. A new interaction pattern is a design decision, not an implementation detail.

### No Confirmation Dialogs

jj's operation log makes every mutation non-destructive. `abandon` doesn't delete data — it hides a change. `undo` restores it completely. The UX is: act immediately, show the result, let the user undo if wrong.

### Mutation Details

**Instant actions — fire on keypress:**

- `abandon` (`d`): Removes change from graph. Cursor moves to parent if on the abandoned change. Status: "Abandoned ksqxwpml." Fully reversible via undo.
- `squash` (`S`): Squashes selected change into parent. Change disappears, parent's files update. Status: "Squashed ksqxwpml into ytoqrzxn." Cursor moves to parent.
- `new` (`n`): Creates empty change after selected. Graph updates, cursor moves to new change (it becomes working copy). No description prompt — user hits `e` to describe. Status: "Created new change after ytoqrzxn."
- `edit` (`Ctrl-E`): Switches working copy to selected change. `@` marker moves. Status: "Now editing ksqxwpml."
- `undo` (`u`): Undoes last operation. Graph rebuilds. Status: "Undid: abandon ksqxwpml."
- `redo` (`Ctrl-R`): Redoes. Status shows what was redone.
- `bookmark delete` (`d` in bookmark picker): Deletes selected bookmark. Status: "Deleted bookmark main."

**Mini-modal actions:**

- `describe` (`e`): Opens `tui-textarea` modal overlaying the detail pane, pre-filled with current description. Keybindings:
  - `Ctrl-S` / `Ctrl-Enter`: Save and close. Emits `Effect::Describe`.
  - `Escape`: Discard and close.
  - `Shift-E`: Escalate to `$EDITOR`. TUI suspends, editor opens with current buffer, TUI resumes with editor output. User reviews in modal before saving.
  - All normal text editing handled by `tui-textarea`.
  - De-risk option: ship `$EDITOR`-only first, add inline modal as fast-follow within M2.

- `bookmark set` (`B`): Single-line input at bottom of screen (not centered popup). Pre-filled with existing bookmark name if present. Fuzzy completion against existing bookmark names. Enter emits `Effect::BookmarkSet`. Escape cancels.

**Background actions:**

- `git push` (`P`): Status bar shows "Pushing..." with spinner. UI remains interactive. On success: "Pushed main -> origin." On failure: error in status bar. Note: `undo` after push undoes local op but can't un-push from remote — status bar notes: "Undid push (remote unchanged)."
- `git fetch` (`f`): "Fetching..." with spinner. On completion, graph rebuilds. "Fetched N new changes from origin." Conflicts from remote changes surface in graph immediately.

### Key Bindings

All mutation keys are **Graph-context-only** — they are mapped in `map_event` under `PanelFocus::Graph`, not as global keys. This avoids collisions with existing keys in other contexts (e.g., `n` is DiffNextHunk in DiffView, `f` is unused in Detail but could be claimed later). Modal-specific keys are routed through `map_modal_event` as with existing modals.

| Key | Context | Action |
|-----|---------|--------|
| `d` | Graph | Abandon selected change |
| `n` | Graph | New change after selected |
| `e` | Graph | Open describe modal |
| `Ctrl-E` | Graph | Edit (switch working copy) |
| `S` | Graph | Squash into parent |
| `u` | Graph | Undo |
| `Ctrl-R` | Graph | Redo |
| `B` | Graph | Bookmark set (opens name input) |
| `P` | Graph | Git push |
| `f` | Graph | Git fetch |
| `d` | Bookmark picker | Delete selected bookmark |
| `Ctrl-S` | Describe modal | Save and close |
| `Ctrl-Enter` | Describe modal | Save and close (alt) |
| `Escape` | Describe modal | Discard and close |
| `Shift-E` | Describe modal | Escalate to `$EDITOR` |

## AppState Changes

```rust
pub struct AppState {
    // Existing (unchanged)
    pub graph: GraphData,
    cursor: usize,
    pub should_quit: bool,
    pub error: Option<String>,
    pub focus: PanelFocus,
    detail_cursor: usize,
    pub detail_mode: DetailMode,
    pub diff_scroll: usize,
    pub diff_data: Vec<DiffHunk>,
    pub modal: Option<Modal>,

    // M2 additions
    pub pending_mutation: Option<MutationKind>,
    pub pending_background: HashSet<BackgroundKind>,
    pub status_message: Option<String>,
}
```

- `error`: Red, sticky until cleared. For failures.
- `status_message`: Neutral/green, transient, cleared on next user keypress. For success feedback.

### New Modal Variants

```rust
pub enum Modal {
    // Existing
    OpLog { .. },
    BookmarkPicker { .. },
    FuzzyFind { .. },
    Help { .. },

    // M2
    Describe {
        change_id: String,
        editor: TextArea<'static>,
    },
    BookmarkInput {
        change_id: String,
        input: String,
        completions: Vec<String>,
        cursor: usize,
    },
}
```

## File Organization

`app.rs` decomposed before M2 implementation begins:

```
crates/lajjzy-tui/src/
  app.rs          AppState struct, constructors, helpers
  action.rs       Action enum, MutationKind, BackgroundKind
  effect.rs       Effect enum
  dispatch.rs     fn dispatch() and its tests
  modal.rs        Modal enum, HelpContext
  input.rs        map_event, map_modal_event (extended with new keys)
  render.rs       (existing)
  panels/         (existing)
  widgets/        (existing, plus describe and bookmark_input widgets)
```

## Testing Strategy

### Three Test Categories

| Category | Tests | Where | Mocks |
|----------|-------|-------|-------|
| Dispatch purity | `(state, action) -> (state, effects)` | `lajjzy-tui` | None |
| Backend methods | jj CLI subprocess + parsing | `lajjzy-core` | Real jj in tempdir |
| Integration | Full effect cycle | `lajjzy-cli` | Real jj in tempdir |

### Dispatch Tests (bulk of new tests)

Zero I/O. Verify:
- Correct effect emitted for each mutation action
- Gate prevents concurrent mutations
- Background gates independent of mutation gate
- Result actions update state correctly
- Navigation unaffected by pending mutations
- `GraphLoaded` replaces graph and repositions cursor
- Status messages set on success, errors set on failure

### Backend Tests

Extend existing tempdir pattern. Each new `RepoBackend` method gets a test:
- `abandon_on_real_repo`: init, create change, abandon, verify gone from `load_graph()`
- `describe_on_real_repo`: init, create change, describe, verify description in `load_graph()`
- etc.

### Migration

Existing 104 tests: `MockBackend`, `FailingBackend`, `DiffMockBackend` disappear. Dispatch tests become pure — assert on returned effects instead of backend side effects. Most test bodies shrink.

## CLAUDE.md Updates

### Architectural Constraints (revised)

- **Dispatch purity (enforced):** `dispatch()` takes `(&mut AppState, Action)` and returns `Vec<Effect>`. Never calls backend methods or performs I/O.
- **Effect executor boundary:** Effects executed in `lajjzy-cli` only. `lajjzy-tui` defines the `Effect` enum but never executes effects.
- **Facade boundary (updated):** `lajjzy-tui` never imports `RepoBackend`, `std::process::Command`, or jj-lib. `$EDITOR` launch handled by event loop in `lajjzy-cli`.
- **Mutation gate:** At most one local mutation in flight, enforced by `AppState.pending_mutation`. Background ops gated independently.
- **Three interaction patterns:** Every mutation declares its slot (Instant, Mini-modal, Background). New patterns require design justification.

### New Dependency

- `tui-textarea` in `lajjzy-tui/Cargo.toml` for the describe modal.
