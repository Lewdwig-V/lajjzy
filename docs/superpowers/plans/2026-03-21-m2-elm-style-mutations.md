# M2: Elm-Style State Transitions + Core Mutations — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make dispatch pure (returns `Vec<Effect>` instead of calling backend), add effect executor with thread+channel architecture, implement 10 jj mutations (abandon, squash, new, edit, describe, undo, redo, bookmark set/delete, push, fetch).

**Architecture:** Elm-style state machine where `dispatch(state, action) -> (state, effects)` is pure. Effects are executed by an `EffectExecutor` in `lajjzy-cli` using `std::thread::spawn` + `mpsc::channel`. Three independent concurrency lanes (local mutations, push, fetch) gated by state-machine fields, not locks.

**Tech Stack:** Rust 1.85+, ratatui 0.30, crossterm 0.29, tui-textarea (new), jj CLI 0.39.0

**Spec:** `docs/superpowers/specs/2026-03-21-m2-elm-style-mutations-design.md`

---

## File Map

### Files to create

| File | Responsibility |
|------|---------------|
| `crates/lajjzy-tui/src/action.rs` | `Action` enum (UI + result actions), `MutationKind`, `BackgroundKind` |
| `crates/lajjzy-tui/src/effect.rs` | `Effect` enum |
| `crates/lajjzy-tui/src/dispatch.rs` | Pure `fn dispatch()` + all dispatch tests |
| `crates/lajjzy-tui/src/modal.rs` | `Modal` enum, `HelpContext`, `Describe`/`BookmarkInput` variants |
| `crates/lajjzy-tui/src/widgets/describe.rs` | Describe modal widget (renders `tui-textarea::TextArea`) |
| `crates/lajjzy-tui/src/widgets/bookmark_input.rs` | Bookmark name input widget |

### Files to modify

| File | Changes |
|------|---------|
| `crates/lajjzy-tui/src/app.rs` | Remove `Action`, `Modal`, `HelpContext`, `dispatch()`, all tests. Keep `AppState`, add `pending_mutation`, `pending_background`, `status_message`, `cursor_follows_working_copy`. |
| `crates/lajjzy-tui/src/input.rs` | Add mutation key bindings (Graph-context-only), `BookmarkInput` modal routing, `Describe` modal routing |
| `crates/lajjzy-tui/src/render.rs` | Render `status_message`, pending-operation spinner, new modal variants |
| `crates/lajjzy-tui/src/lib.rs` | Add `pub mod action; pub mod effect; pub mod dispatch; pub mod modal;` |
| `crates/lajjzy-tui/src/widgets/mod.rs` | Add `pub mod describe; pub mod bookmark_input;` |
| `crates/lajjzy-tui/src/widgets/status_bar.rs` | Show `status_message` (green) and pending-op spinner |
| `crates/lajjzy-core/src/backend.rs` | Add 11 mutation methods to `RepoBackend` trait |
| `crates/lajjzy-core/src/cli.rs` | Implement mutation methods on `JjCliBackend` |
| `crates/lajjzy-cli/src/main.rs` | New event loop with poll+channel, `EffectExecutor`, `execute_effects`, `$EDITOR` suspend |
| `crates/lajjzy-tui/Cargo.toml` | Add `tui-textarea` dependency |
| `CLAUDE.md` | Update architectural constraints |

---

## Task 1: Decompose app.rs — extract types to new modules

This is a pure refactor. No behavior change, all 104 tests must pass after.

**Files:**
- Create: `crates/lajjzy-tui/src/action.rs`
- Create: `crates/lajjzy-tui/src/effect.rs`
- Create: `crates/lajjzy-tui/src/modal.rs`
- Create: `crates/lajjzy-tui/src/dispatch.rs`
- Modify: `crates/lajjzy-tui/src/app.rs`
- Modify: `crates/lajjzy-tui/src/lib.rs`
- Modify: `crates/lajjzy-tui/src/input.rs` (imports only)
- Modify: `crates/lajjzy-tui/src/render.rs` (imports only)
- Modify: `crates/lajjzy-tui/src/panels/graph.rs` (imports only)
- Modify: `crates/lajjzy-tui/src/panels/detail.rs` (imports only)
- Modify: `crates/lajjzy-tui/src/widgets/*.rs` (imports only)

- [ ] **Step 1: Create `action.rs`**

Move from `app.rs`: `Action` enum, `PanelFocus` enum, `DetailMode` enum. These are the input vocabulary shared by `input.rs` and `dispatch.rs`. Add `#[derive(Debug, Clone, Copy, PartialEq, Eq)]` on all. Re-export from `app.rs` via `pub use crate::action::*;` temporarily for backwards compatibility.

- [ ] **Step 2: Create `effect.rs`**

Create an empty `Effect` enum with a single placeholder variant for now:

```rust
/// Effects emitted by dispatch. Defined in lajjzy-tui, executed in lajjzy-cli.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {}
```

This compiles and will be populated in later tasks.

- [ ] **Step 3: Create `modal.rs`**

Move from `app.rs`: `Modal` enum, `HelpContext` enum and its `impl`. Re-export from `app.rs` via `pub use crate::modal::*;`.

- [ ] **Step 4: Create `dispatch.rs`**

Move from `app.rs`: `fn dispatch()`, `fn fuzzy_match()`, and the entire `#[cfg(test)] mod tests` block (including `MockBackend`, `FailingBackend`, `DiffMockBackend`, all test functions). The function signature stays `fn dispatch(state: &mut AppState, action: Action, backend: &dyn RepoBackend)` for now — we change it in Task 3.

- [ ] **Step 5: Update `lib.rs`**

```rust
pub mod action;
pub mod app;
pub mod dispatch;
pub mod effect;
pub mod input;
pub mod modal;
pub mod panels;
pub mod render;
pub mod widgets;
```

- [ ] **Step 6: Update imports across codebase**

Update all files that import from `crate::app` to import from the new module paths. Key changes:
- `input.rs`: `use crate::action::{Action, DetailMode, PanelFocus};` and `use crate::modal::Modal;`
- `render.rs`: update imports for `Modal`, `AppState`
- `panels/*.rs`, `widgets/*.rs`: update as needed
- `dispatch.rs`: `use crate::app::AppState;`, `use crate::action::*;`, `use crate::modal::*;`

Keep `pub use` re-exports in `app.rs` if needed for `lajjzy-cli/src/main.rs` (which imports `lajjzy_tui::app::{AppState, dispatch}`).

- [ ] **Step 7: Run tests, verify all 104 pass**

Run: `cargo test`
Expected: all 104 tests pass, no warnings about unused imports.

- [ ] **Step 8: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt --check`
Expected: clean

- [ ] **Step 9: Commit**

```bash
git add -A && git commit -m "refactor: decompose app.rs into action, effect, modal, dispatch modules"
```

---

## Task 2: Add new Action variants, Effect enum, and AppState fields

Extend the type system with everything M2 needs. No behavior change yet — just types.

**Files:**
- Modify: `crates/lajjzy-tui/src/action.rs`
- Modify: `crates/lajjzy-tui/src/effect.rs`
- Modify: `crates/lajjzy-tui/src/app.rs`
- Modify: `crates/lajjzy-tui/src/modal.rs`
- Modify: `crates/lajjzy-tui/Cargo.toml`

- [ ] **Step 1: Add `tui-textarea` dependency**

Check the latest version first:

Run: `cargo search tui-textarea --limit 1`

Add to `crates/lajjzy-tui/Cargo.toml`:
```toml
tui-textarea = "<version>"
```

- [ ] **Step 2: Add M2 Action variants**

In `action.rs`, add to the `Action` enum:

```rust
// Effect result actions
GraphLoaded(Result<GraphData, String>),
OpLogLoaded(Result<Vec<OpLogEntry>, String>),
FileDiffLoaded(Result<Vec<DiffHunk>, String>),
RepoOpSuccess { op: MutationKind, message: String },
RepoOpFailed { op: MutationKind, error: String },
EditorComplete { change_id: String, text: String },

// Mutation trigger actions
Abandon,
Squash,
NewChange,
EditChange,
OpenDescribe,
Undo,
Redo,
OpenBookmarkSet,
BookmarkInputChar(char),
BookmarkInputBackspace,
BookmarkInputConfirm,
BookmarkDelete,
GitPush,
GitFetch,
DescribeSave,
DescribeEscalateEditor,
```

Note: `Result<GraphData>` uses `anyhow::Result` in the backend but we pass `Result<GraphData, String>` through the channel (serialize the error to string). This avoids `anyhow::Error: Send` issues and keeps the Action enum simple.

Add `MutationKind` and `BackgroundKind`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MutationKind {
    Describe, New, Edit, Abandon, Squash,
    Undo, Redo, BookmarkSet, BookmarkDelete,
    GitPush, GitFetch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackgroundKind { Push, Fetch }
```

The `Action` enum can no longer derive `Copy` (it holds `String`/`Vec` in result variants). Update to `#[derive(Debug, Clone, PartialEq)]`.

- [ ] **Step 3: Populate `Effect` enum**

```rust
use lajjzy_core::types::{DiffHunk, GraphData, OpLogEntry};

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    // Read-only
    LoadGraph { revset: Option<String> },
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

- [ ] **Step 4: Add AppState fields**

In `app.rs`, add to `AppState`:

```rust
use std::collections::HashSet;
use crate::action::{MutationKind, BackgroundKind};

pub struct AppState {
    // ... existing fields ...
    pub pending_mutation: Option<MutationKind>,
    pub pending_background: HashSet<BackgroundKind>,
    pub status_message: Option<String>,
    pub cursor_follows_working_copy: bool,
}
```

Update `AppState::new()` to initialize:
```rust
pending_mutation: None,
pending_background: HashSet::new(),
status_message: None,
cursor_follows_working_copy: false,
```

- [ ] **Step 5: Add modal variants**

In `modal.rs`, add:

```rust
use tui_textarea::TextArea;

pub enum Modal {
    // ... existing variants ...
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

Note: `TextArea` doesn't implement `Debug` or `Clone`, so `Modal` loses those derives. Replace `#[derive(Debug, Clone)]` with a manual `Debug` impl that prints "Describe { .. }" for the `Describe` variant, or use `#[derive(Clone)]` only on the other variants and handle `Describe` separately. Check `tui-textarea` docs for what traits it implements.

- [ ] **Step 6: Fix all compilation errors**

The `Action` enum change from `Copy` to non-`Copy` will break any code that copies an `Action`. Find and fix all affected sites — mostly in `input.rs` tests where `Action` values are compared.

Run: `cargo check 2>&1 | head -50`

Fix each error. This is mechanical — the fixes are adding `.clone()` or changing match patterns.

- [ ] **Step 7: Run tests, verify all pass**

Run: `cargo test`
Expected: all existing tests pass. New types are unused but that's fine (they'll get used in Task 3+).

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat(types): add M2 Action variants, Effect enum, AppState fields, modal variants"
```

---

## Task 3: Make dispatch pure — return Vec<Effect>

Change the `dispatch()` signature and convert existing backend calls to effects. This is the core architectural change.

**Files:**
- Modify: `crates/lajjzy-tui/src/dispatch.rs`
- Modify: `crates/lajjzy-tui/src/app.rs` (remove `RepoBackend` import if present)

- [ ] **Step 1: Write a failing test for the new dispatch signature**

In `dispatch.rs`, add a test that calls `dispatch` without a backend parameter and checks it returns a `Vec<Effect>`:

```rust
#[test]
fn dispatch_returns_effects() {
    let graph = test_graph(); // reuse existing test helper
    let mut state = AppState::new(graph);
    let effects = dispatch(&mut state, Action::Quit);
    assert!(effects.is_empty()); // Quit produces no effects
    assert!(state.should_quit);
}
```

Run: `cargo test -p lajjzy-tui dispatch_returns_effects`
Expected: FAIL — `dispatch` still takes 3 args.

- [ ] **Step 2: Change dispatch signature**

Change:
```rust
pub fn dispatch(state: &mut AppState, action: Action, backend: &dyn RepoBackend)
```
To:
```rust
pub fn dispatch(state: &mut AppState, action: Action) -> Vec<Effect>
```

- [ ] **Step 3: Convert `Action::Refresh` to emit `Effect::LoadGraph`**

Replace the inline `backend.load_graph()` call with:

```rust
Action::Refresh => {
    state.error = None;
    return vec![Effect::LoadGraph { revset: None }];
}
```

- [ ] **Step 4: Add `Action::GraphLoaded` handler**

Move the graph-replacement logic from the old `Refresh` handler into a new `GraphLoaded` handler:

```rust
Action::GraphLoaded(result) => {
    match result {
        Ok(new_graph) => {
            let prev_id = state.selected_change_id().map(String::from);
            state.graph = new_graph;
            state.reset_detail();

            if state.cursor_follows_working_copy {
                state.cursor_follows_working_copy = false;
                state.cursor = state.graph.working_copy_index
                    .or_else(|| state.graph.node_indices().first().copied())
                    .unwrap_or(0);
            } else {
                let nodes = state.graph.node_indices();
                state.cursor = prev_id.as_deref()
                    .and_then(|id| nodes.iter()
                        .find(|&&i| state.graph.lines[i].change_id.as_deref() == Some(id))
                        .copied())
                    .or(state.graph.working_copy_index)
                    .or_else(|| nodes.first().copied())
                    .unwrap_or(0);
            }
        }
        Err(e) => {
            state.error = Some(format!("Failed to load graph: {e}"));
        }
    }
}
```

- [ ] **Step 5: Convert `Action::DetailEnter` to emit `Effect::LoadFileDiff`**

Replace the inline `backend.file_diff()` call. The rename-path extraction stays in dispatch (it's pure logic):

```rust
Action::DetailEnter => {
    let file_info = state.selected_detail()
        .and_then(|d| d.files.get(state.detail_cursor))
        .map(|f| (f.path.clone(), f.status));
    let change_id = state.selected_change_id().map(String::from);

    if let (Some(cid), Some((raw_path, status))) = (change_id, file_info) {
        let diff_path = if status == lajjzy_core::types::FileStatus::Renamed {
            raw_path.split("=> ").nth(1)
                .and_then(|s| s.strip_suffix('}'))
                .unwrap_or(&raw_path)
                .to_string()
        } else {
            raw_path
        };
        return vec![Effect::LoadFileDiff { change_id: cid, path: diff_path }];
    }
}
```

- [ ] **Step 6: Add `Action::FileDiffLoaded` handler**

```rust
Action::FileDiffLoaded(result) => {
    match result {
        Ok(hunks) => {
            state.diff_data = hunks;
            state.diff_scroll = 0;
            state.detail_mode = DetailMode::DiffView;
        }
        Err(e) => {
            state.diff_data = vec![];
            state.error = Some(format!("Failed to load diff: {e}"));
        }
    }
}
```

- [ ] **Step 7: Convert `Action::ToggleOpLog` to emit `Effect::LoadOpLog`**

```rust
Action::ToggleOpLog => {
    if matches!(state.modal, Some(Modal::OpLog { .. })) {
        state.modal = None;
    } else {
        return vec![Effect::LoadOpLog];
    }
}
```

- [ ] **Step 8: Add `Action::OpLogLoaded` handler**

```rust
Action::OpLogLoaded(result) => {
    match result {
        Ok(entries) => {
            state.modal = Some(Modal::OpLog { entries, cursor: 0, scroll: 0 });
        }
        Err(e) => {
            state.error = Some(format!("Failed to load op log: {e}"));
        }
    }
}
```

- [ ] **Step 9: Add `return vec![]` to all other match arms**

Every non-effect-producing arm returns `vec![]`. The compiler will guide you — any arm without a return is an error since the function now returns `Vec<Effect>`.

- [ ] **Step 10: Migrate existing dispatch tests**

Remove `MockBackend`, `FailingBackend`, `DiffMockBackend`. Update every test:

Old pattern:
```rust
dispatch(&mut state, Action::Refresh, &mock);
assert!(state.error.is_none());
```

New pattern:
```rust
let effects = dispatch(&mut state, Action::Refresh);
assert_eq!(effects, vec![Effect::LoadGraph { revset: None }]);
```

For tests that previously checked state after a backend call succeeded, split into two tests:
1. Test that the action emits the correct effect
2. Test that the result action updates state correctly

Example:
```rust
#[test]
fn refresh_emits_load_graph() {
    let mut state = test_state();
    let effects = dispatch(&mut state, Action::Refresh);
    assert_eq!(effects, vec![Effect::LoadGraph { revset: None }]);
    assert!(state.error.is_none()); // error cleared
}

#[test]
fn graph_loaded_updates_graph() {
    let mut state = test_state();
    let new_graph = test_graph_with_changes(&["xxx"]);
    let effects = dispatch(&mut state, Action::GraphLoaded(Ok(new_graph)));
    assert!(effects.is_empty());
    assert_eq!(state.graph.node_indices().len(), 1);
}
```

- [ ] **Step 11: Run tests**

Run: `cargo test -p lajjzy-tui`
Expected: all dispatch tests pass with new signature.

- [ ] **Step 12: Fix `lajjzy-cli/src/main.rs` compilation**

> **WARNING:** From this commit through Task 5, the binary is intentionally non-functional. Refresh, DetailEnter, and ToggleOpLog emit effects that are silently dropped. Do NOT attempt to smoke-test the binary until Task 5 is complete. All validation happens through unit tests only.

The event loop in `main.rs` still calls `dispatch(state, action, backend)`. For now, make it compile by updating the call and ignoring the returned effects (they'll be wired in Task 5):

```rust
let effects = dispatch(state, action);
// TODO(Task 5): wire effects to executor — binary is non-functional until then
let _ = effects;
```

Run: `cargo check`
Expected: compiles.

- [ ] **Step 13: Run full test suite**

Run: `cargo test`
Expected: all tests pass.

- [ ] **Step 14: Commit**

```bash
git add -A && git commit -m "feat: make dispatch pure — returns Vec<Effect>, no backend parameter"
```

---

## Task 4: RepoBackend mutation methods + JjCliBackend implementations

Add the 11 new methods to the backend trait and implement them.

**Files:**
- Modify: `crates/lajjzy-core/src/backend.rs`
- Modify: `crates/lajjzy-core/src/cli.rs`

- [ ] **Step 1: Write failing test for `abandon`**

In `cli.rs` tests:

```rust
#[test]
fn abandon_on_real_repo() {
    if !jj_available() { eprintln!("Skipping"); return; }
    let tmp = tempfile::tempdir().unwrap();
    Command::new("jj").args(["git", "init"]).current_dir(tmp.path()).status().unwrap();
    Command::new("jj").args(["describe", "-m", "doomed"]).current_dir(tmp.path()).status().unwrap();

    let backend = JjCliBackend::new(tmp.path()).unwrap();
    let graph = backend.load_graph().unwrap();
    let wc_id = graph.lines[graph.working_copy_index.unwrap()].change_id.as_ref().unwrap().clone();

    let msg = backend.abandon(&wc_id).unwrap();
    assert!(!msg.is_empty());

    // Verify change is gone from graph
    let graph2 = backend.load_graph().unwrap();
    let has_old = graph2.node_indices().iter().any(|&i|
        graph2.lines[i].change_id.as_deref() == Some(&wc_id)
    );
    assert!(!has_old);
}
```

Run: `cargo test -p lajjzy-core abandon_on_real_repo`
Expected: FAIL — method doesn't exist.

- [ ] **Step 2: Add mutation methods to `RepoBackend` trait**

In `backend.rs`:

```rust
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
```

- [ ] **Step 3: Implement all methods on `JjCliBackend`**

Each method follows the same pattern — run `jj <command>`, check exit status, return stdout/stderr as status message. Implement a helper to reduce boilerplate:

```rust
fn run_jj(&self, args: &[&str]) -> Result<String> {
    let output = Command::new("jj")
        .args(args)
        .current_dir(&self.workspace_root)
        .output()
        .with_context(|| format!("Failed to run `jj {}`", args.join(" ")))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() {
        bail!("{}", if stderr.is_empty() { &stdout } else { &stderr });
    }

    // Prefer stderr for status messages (jj prints most feedback there)
    Ok(if stderr.is_empty() { stdout } else { stderr })
}
```

Then each method is a one-liner:

```rust
fn abandon(&self, change_id: &str) -> Result<String> {
    self.run_jj(&["abandon", change_id])
}
fn describe(&self, change_id: &str, text: &str) -> Result<String> {
    self.run_jj(&["describe", change_id, "-m", text])
}
fn new_change(&self, after: &str) -> Result<String> {
    self.run_jj(&["new", after])
}
fn edit_change(&self, change_id: &str) -> Result<String> {
    self.run_jj(&["edit", change_id])
}
fn squash(&self, change_id: &str) -> Result<String> {
    self.run_jj(&["squash", "-r", change_id])
}
fn undo(&self) -> Result<String> {
    self.run_jj(&["op", "undo", "--no-edit"])
}
fn redo(&self) -> Result<String> {
    self.run_jj(&["op", "redo", "--no-edit"])
}
fn bookmark_set(&self, change_id: &str, name: &str) -> Result<String> {
    self.run_jj(&["bookmark", "set", name, "-r", change_id])
}
fn bookmark_delete(&self, name: &str) -> Result<String> {
    self.run_jj(&["bookmark", "delete", name])
}
fn git_push(&self, bookmark: &str) -> Result<String> {
    self.run_jj(&["git", "push", "--bookmark", bookmark])
}
fn git_fetch(&self) -> Result<String> {
    self.run_jj(&["git", "fetch"])
}
```

**MANDATORY:** Before implementing, run `jj op undo --help` and `jj op redo --help` to verify available flags. Remove `--no-edit` if not supported. Do NOT assume CLI flags exist without verification.

- [ ] **Step 4: Run the abandon test**

Run: `cargo test -p lajjzy-core abandon_on_real_repo`
Expected: PASS

- [ ] **Step 5: Write tests for remaining mutations**

Add one test per mutation, following the tempdir pattern. Key tests:

```rust
#[test]
fn describe_on_real_repo() { /* init, describe, verify in load_graph */ }

#[test]
fn new_change_on_real_repo() { /* init, new, verify graph has one more node */ }

#[test]
fn edit_on_real_repo() { /* init, create 2 changes, edit older one, verify working_copy_index changed */ }

#[test]
fn squash_on_real_repo() { /* init, create change with file, new, squash, verify parent has file */ }

#[test]
fn undo_on_real_repo() { /* init, describe, undo, verify description reverted */ }

#[test]
fn redo_on_real_repo() { /* init, describe, undo, redo, verify description restored */ }

#[test]
fn bookmark_set_on_real_repo() { /* init, bookmark set, verify in load_graph */ }

#[test]
fn bookmark_delete_on_real_repo() { /* init, bookmark set, bookmark delete, verify gone */ }
```

Skip `git_push` and `git_fetch` tests — they need a remote, which is complex to set up in CI. Add a `#[test] #[ignore]` test with a comment explaining why.

- [ ] **Step 6: Run all core tests**

Run: `cargo test -p lajjzy-core`
Expected: all pass (new + existing).

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "feat(core): add mutation methods to RepoBackend + JjCliBackend implementations"
```

---

## Task 5: Effect executor and poll-based event loop

Wire effects to the backend through the executor. This replaces the synchronous event loop.

**Files:**
- Modify: `crates/lajjzy-cli/src/main.rs`
- Modify: `crates/lajjzy-cli/Cargo.toml` (if needed for Arc)

- [ ] **Step 1: Add `EffectExecutor` struct**

In `main.rs`:

```rust
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use lajjzy_tui::action::{Action, MutationKind, BackgroundKind};
use lajjzy_tui::effect::Effect;

struct EffectExecutor {
    backend: Arc<JjCliBackend>,
    tx: mpsc::Sender<Action>,
}
```

- [ ] **Step 2: Implement `EffectExecutor::execute`**

Every effect spawns a thread. Mutation effects send `RepoOpSuccess`/`RepoOpFailed` + `GraphLoaded`:

```rust
impl EffectExecutor {
    fn execute(&self, effect: Effect) {
        let backend = self.backend.clone();
        let tx = self.tx.clone();

        thread::spawn(move || {
            match effect {
                Effect::LoadGraph { revset: _ } => {
                    // revset parameter reserved for M3 omnibar; ignored for now
                    let result = backend.load_graph()
                        .map_err(|e| e.to_string());
                    let _ = tx.send(Action::GraphLoaded(result));
                }
                Effect::LoadOpLog => {
                    let result = backend.op_log()
                        .map_err(|e| e.to_string());
                    let _ = tx.send(Action::OpLogLoaded(result));
                }
                Effect::LoadFileDiff { change_id, path } => {
                    let result = backend.file_diff(&change_id, &path)
                        .map_err(|e| e.to_string());
                    let _ = tx.send(Action::FileDiffLoaded(result));
                }
                Effect::Abandon { change_id } => {
                    Self::run_mutation(&backend, &tx, MutationKind::Abandon, || {
                        backend.abandon(&change_id)
                    });
                }
                // ... same pattern for all mutation effects
                Effect::GitPush { bookmark } => {
                    Self::run_mutation(&backend, &tx, MutationKind::GitPush, || {
                        backend.git_push(&bookmark)
                    });
                }
                Effect::GitFetch => {
                    Self::run_mutation(&backend, &tx, MutationKind::GitFetch, || {
                        backend.git_fetch()
                    });
                }
                Effect::SuspendForEditor { .. } => {
                    // Handled by execute_effects, never reaches here
                    unreachable!("SuspendForEditor must be intercepted by execute_effects");
                }
                // ... remaining variants
            }
        });
    }

    fn run_mutation(
        backend: &JjCliBackend,
        tx: &mpsc::Sender<Action>,
        op: MutationKind,
        f: impl FnOnce() -> anyhow::Result<String>,
    ) {
        match f() {
            Ok(message) => {
                let _ = tx.send(Action::RepoOpSuccess { op, message });
                // Always refresh graph after successful mutation
                let graph = backend.load_graph().map_err(|e| e.to_string());
                let _ = tx.send(Action::GraphLoaded(graph));
            }
            Err(e) => {
                let _ = tx.send(Action::RepoOpFailed { op, error: e.to_string() });
            }
        }
    }
}
```

- [ ] **Step 3: Implement `execute_effects` with `SuspendForEditor` interception**

```rust
fn execute_effects(
    terminal: &mut ratatui::DefaultTerminal,
    state: &mut AppState,
    executor: &EffectExecutor,
    effects: Vec<Effect>,
) {
    for effect in effects {
        match effect {
            Effect::SuspendForEditor { change_id, initial_text } => {
                ratatui::restore();
                let result = run_editor(&initial_text);
                *terminal = ratatui::init();
                match result {
                    Ok(text) => {
                        let effects = dispatch(state, Action::EditorComplete { change_id, text });
                        execute_effects(terminal, state, executor, effects);
                    }
                    Err(e) => {
                        state.error = Some(format!("Editor failed: {e}"));
                    }
                }
            }
            other => executor.execute(other),
        }
    }
}

fn run_editor(initial_text: &str) -> anyhow::Result<String> {
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string());

    let tmp = tempfile::NamedTempFile::new()?;
    std::fs::write(tmp.path(), initial_text)?;

    let status = std::process::Command::new(&editor)
        .arg(tmp.path())
        .status()
        .with_context(|| format!("Failed to launch editor: {editor}"))?;

    if !status.success() {
        bail!("Editor exited with status {status}");
    }

    Ok(std::fs::read_to_string(tmp.path())?)
}
```

Add `tempfile` to `lajjzy-cli/Cargo.toml` dev-dependencies (or dependencies if `run_editor` is in main code).

- [ ] **Step 4: Rewrite `run_loop` with poll + channel**

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
                state.status_message = None;
                if let Some(action) = if let Some(ref modal) = state.modal {
                    map_modal_event(key_event, modal)
                } else {
                    map_event(key_event, state.focus, state.detail_mode)
                } {
                    let effects = dispatch(state, action);
                    execute_effects(terminal, state, executor, effects);
                }
            }
        }

        while let Ok(action) = rx.try_recv() {
            let effects = dispatch(state, action);
            execute_effects(terminal, state, executor, effects);
        }

        if state.should_quit {
            break;
        }
    }
    Ok(())
}
```

- [ ] **Step 5: Update `main()` to create executor and channel**

```rust
fn main() -> Result<()> {
    let cwd = env::current_dir().context("Failed to get current directory")?;
    let backend = Arc::new(JjCliBackend::new(&cwd).context("Failed to open jj workspace")?);

    let graph = backend.load_graph().context("Failed to load graph")?;
    let mut state = AppState::new(graph);

    let (tx, rx) = mpsc::channel();
    let executor = EffectExecutor { backend, tx };

    // ... panic hook (unchanged) ...

    let mut terminal = ratatui::init();
    let result = run_loop(&mut terminal, &mut state, &executor, &rx);
    ratatui::restore();
    result
}
```

- [ ] **Step 6: Verify it compiles**

Run: `cargo check`
Expected: compiles (mutation dispatch arms not yet wired but existing actions work).

- [ ] **Step 7: Run full tests**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 8: Manual smoke test**

Run: `cargo run -p lajjzy` (in a jj repo)
Expected: navigation works, `R` refreshes (async now), op log opens, diff view works.

- [ ] **Step 9: Commit**

```bash
git add -A && git commit -m "feat: effect executor with thread+channel, poll-based event loop"
```

---

## Task 6: Instant mutation dispatch + key bindings

Wire the instant mutations: abandon, squash, new, edit, undo, redo.

**Files:**
- Modify: `crates/lajjzy-tui/src/dispatch.rs`
- Modify: `crates/lajjzy-tui/src/input.rs`

- [ ] **Step 1: Write failing tests for instant mutations**

In `dispatch.rs`:

```rust
#[test]
fn abandon_emits_effect_and_sets_gate() {
    let mut state = test_state_with_changes(&["aaa", "bbb"]);
    // cursor on "aaa"
    let effects = dispatch(&mut state, Action::Abandon);
    assert_eq!(effects.len(), 1);
    assert!(matches!(&effects[0], Effect::Abandon { change_id } if change_id == "aaa"));
    assert_eq!(state.pending_mutation, Some(MutationKind::Abandon));
}

#[test]
fn mutation_suppressed_while_pending() {
    let mut state = test_state_with_changes(&["aaa", "bbb"]);
    state.pending_mutation = Some(MutationKind::Abandon);
    let effects = dispatch(&mut state, Action::Squash);
    assert!(effects.is_empty());
}

#[test]
fn repo_op_success_clears_gate() {
    let mut state = test_state_with_changes(&["aaa"]);
    state.pending_mutation = Some(MutationKind::Abandon);
    let effects = dispatch(&mut state, Action::RepoOpSuccess {
        op: MutationKind::Abandon,
        message: "Abandoned aaa".into(),
    });
    assert!(state.pending_mutation.is_none());
    assert_eq!(state.status_message.as_deref(), Some("Abandoned aaa"));
    assert!(effects.is_empty());
}

#[test]
fn repo_op_failed_clears_gate() {
    let mut state = test_state_with_changes(&["aaa"]);
    state.pending_mutation = Some(MutationKind::Abandon);
    let effects = dispatch(&mut state, Action::RepoOpFailed {
        op: MutationKind::Abandon,
        error: "conflict".into(),
    });
    assert!(state.pending_mutation.is_none());
    assert_eq!(state.error.as_deref(), Some("conflict"));
}

#[test]
fn new_change_sets_cursor_follows_flag() {
    let mut state = test_state_with_changes(&["aaa"]);
    let effects = dispatch(&mut state, Action::NewChange);
    assert!(matches!(&effects[0], Effect::New { .. }));
    assert!(state.cursor_follows_working_copy);
}

#[test]
fn navigation_unaffected_by_pending_mutation() {
    let mut state = test_state_with_changes(&["aaa", "bbb"]);
    state.pending_mutation = Some(MutationKind::Abandon);
    let effects = dispatch(&mut state, Action::MoveDown);
    assert!(effects.is_empty()); // navigation produces no effects
    // But cursor moved:
    assert_ne!(state.cursor(), 0); // moved from first node
}
```

Run: `cargo test -p lajjzy-tui` — expect failures (handlers don't exist yet).

- [ ] **Step 2: Implement instant mutation dispatch arms**

In `dispatch.rs`, add handlers:

```rust
Action::Abandon => {
    if state.pending_mutation.is_some() { return vec![]; }
    if let Some(cid) = state.selected_change_id().map(String::from) {
        state.pending_mutation = Some(MutationKind::Abandon);
        return vec![Effect::Abandon { change_id: cid }];
    }
}
Action::Squash => {
    if state.pending_mutation.is_some() { return vec![]; }
    if let Some(cid) = state.selected_change_id().map(String::from) {
        state.pending_mutation = Some(MutationKind::Squash);
        return vec![Effect::Squash { change_id: cid }];
    }
}
Action::NewChange => {
    if state.pending_mutation.is_some() { return vec![]; }
    if let Some(cid) = state.selected_change_id().map(String::from) {
        state.pending_mutation = Some(MutationKind::New);
        state.cursor_follows_working_copy = true;
        return vec![Effect::New { after: cid }];
    }
}
Action::EditChange => {
    if state.pending_mutation.is_some() { return vec![]; }
    if let Some(cid) = state.selected_change_id().map(String::from) {
        state.pending_mutation = Some(MutationKind::Edit);
        return vec![Effect::Edit { change_id: cid }];
    }
}
Action::Undo => {
    if state.pending_mutation.is_some() { return vec![]; }
    state.pending_mutation = Some(MutationKind::Undo);
    return vec![Effect::Undo];
}
Action::Redo => {
    if state.pending_mutation.is_some() { return vec![]; }
    state.pending_mutation = Some(MutationKind::Redo);
    return vec![Effect::Redo];
}
```

And the result handlers:

```rust
Action::RepoOpSuccess { op, message } => {
    match op {
        MutationKind::GitPush => { state.pending_background.remove(&BackgroundKind::Push); }
        MutationKind::GitFetch => { state.pending_background.remove(&BackgroundKind::Fetch); }
        _ => { state.pending_mutation = None; }
    }
    state.status_message = Some(message);
}
Action::RepoOpFailed { op, error } => {
    match op {
        MutationKind::GitPush => { state.pending_background.remove(&BackgroundKind::Push); }
        MutationKind::GitFetch => { state.pending_background.remove(&BackgroundKind::Fetch); }
        _ => { state.pending_mutation = None; }
    }
    state.error = Some(error);
}
```

- [ ] **Step 3: Add key bindings in `input.rs`**

In the `PanelFocus::Graph` match arm:

```rust
(KeyCode::Char('d'), KeyModifiers::NONE) => Some(Action::Abandon),
(KeyCode::Char('n'), KeyModifiers::NONE) => Some(Action::NewChange),
(KeyCode::Char('e'), KeyModifiers::NONE) => Some(Action::OpenDescribe),
(KeyCode::Char('e'), KeyModifiers::CONTROL) => Some(Action::EditChange),
(KeyCode::Char('S'), _) => Some(Action::Squash),
(KeyCode::Char('u'), KeyModifiers::NONE) => Some(Action::Undo),
(KeyCode::Char('r'), KeyModifiers::CONTROL) => Some(Action::Redo),
(KeyCode::Char('B'), _) => Some(Action::OpenBookmarkSet),
(KeyCode::Char('P'), _) => Some(Action::GitPush),
(KeyCode::Char('f'), KeyModifiers::NONE) => Some(Action::GitFetch),
```

- [ ] **Step 4: Add key binding tests**

```rust
#[test]
fn graph_mutation_keys() {
    assert_eq!(map_graph(key(KeyCode::Char('d'))), Some(Action::Abandon));
    assert_eq!(map_graph(key(KeyCode::Char('n'))), Some(Action::NewChange));
    assert_eq!(map_graph(key(KeyCode::Char('S'))), Some(Action::Squash));
    assert_eq!(map_graph(key(KeyCode::Char('u'))), Some(Action::Undo));
    assert_eq!(map_graph(key_mod(KeyCode::Char('r'), KeyModifiers::CONTROL)), Some(Action::Redo));
    assert_eq!(map_graph(key_mod(KeyCode::Char('e'), KeyModifiers::CONTROL)), Some(Action::EditChange));
    assert_eq!(map_graph(key(KeyCode::Char('P'))), Some(Action::GitPush));
    assert_eq!(map_graph(key(KeyCode::Char('f'))), Some(Action::GitFetch));
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: instant mutation dispatch (abandon, squash, new, edit, undo, redo) + key bindings"
```

---

## Task 7: Background mutations — push and fetch

Wire push and fetch with independent background gating.

**Files:**
- Modify: `crates/lajjzy-tui/src/dispatch.rs`
- Modify: `crates/lajjzy-tui/src/widgets/status_bar.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn push_uses_background_gate() {
    let mut state = test_state_with_bookmarked_change("aaa", "main");
    let effects = dispatch(&mut state, Action::GitPush);
    assert!(matches!(&effects[0], Effect::GitPush { bookmark } if bookmark == "main"));
    assert!(state.pending_background.contains(&BackgroundKind::Push));
    // Local mutation gate untouched
    assert!(state.pending_mutation.is_none());
}

#[test]
fn push_suppressed_while_pushing() {
    let mut state = test_state_with_bookmarked_change("aaa", "main");
    state.pending_background.insert(BackgroundKind::Push);
    let effects = dispatch(&mut state, Action::GitPush);
    assert!(effects.is_empty());
}

#[test]
fn fetch_concurrent_with_mutation() {
    let mut state = test_state_with_changes(&["aaa"]);
    state.pending_mutation = Some(MutationKind::Abandon);
    let effects = dispatch(&mut state, Action::GitFetch);
    // Fetch is NOT suppressed by pending_mutation
    assert_eq!(effects.len(), 1);
    assert!(matches!(&effects[0], Effect::GitFetch));
}
```

- [ ] **Step 2: Implement push and fetch dispatch**

```rust
Action::GitPush => {
    if state.pending_background.contains(&BackgroundKind::Push) {
        return vec![];
    }
    // Find bookmark on selected change.
    // Known limitation: if multiple bookmarks exist, pushes the first one.
    // M3 could open a bookmark picker when multiple are present.
    if let Some(detail) = state.selected_detail() {
        if let Some(bookmark) = detail.bookmarks.first() {
            state.pending_background.insert(BackgroundKind::Push);
            return vec![Effect::GitPush { bookmark: bookmark.clone() }];
        }
        // No bookmark — set error
        state.error = Some("No bookmark on selected change".into());
    }
}
Action::GitFetch => {
    if state.pending_background.contains(&BackgroundKind::Fetch) {
        return vec![];
    }
    state.pending_background.insert(BackgroundKind::Fetch);
    return vec![Effect::GitFetch];
}
```

- [ ] **Step 3: Add test helpers**

The dispatch tests need these helpers (add to `dispatch.rs` `#[cfg(test)]` module). Extend or adapt existing `test_graph()` / `test_state()` helpers from the migration in Task 3:

```rust
/// Build a GraphData with N changes (no bookmarks, no files).
fn test_state_with_changes(ids: &[&str]) -> AppState {
    let mut lines = Vec::new();
    let mut details = HashMap::new();
    for (i, &id) in ids.iter().enumerate() {
        lines.push(GraphLine {
            raw: format!("◉  {id} test {i}m ago"),
            change_id: Some(id.to_string()),
            glyph_prefix: String::new(),
        });
        details.insert(id.to_string(), ChangeDetail {
            commit_id: format!("{id}_commit"),
            author: "test".into(),
            email: "test@test.com".into(),
            timestamp: format!("{i}m ago"),
            description: format!("change {id}"),
            bookmarks: vec![],
            is_empty: false,
            has_conflict: false,
            files: vec![],
        });
    }
    let graph = GraphData::new(lines, details, Some(0));
    AppState::new(graph)
}

/// Build state with one change that has a bookmark.
fn test_state_with_bookmarked_change(id: &str, bookmark: &str) -> AppState {
    let mut state = test_state_with_changes(&[id]);
    if let Some(detail) = state.graph.details.get_mut(id) {
        detail.bookmarks = vec![bookmark.to_string()];
    }
    state
}

/// Build state with one change that has a description.
fn test_state_with_described_change(id: &str, description: &str) -> AppState {
    let mut state = test_state_with_changes(&[id]);
    if let Some(detail) = state.graph.details.get_mut(id) {
        detail.description = description.to_string();
    }
    state
}
```

Note: `GraphData.details` is `pub` so direct mutation in tests is fine. If `details` has no `get_mut` because the GraphData constructor caches indices, adjust accordingly — you may need to build the `GraphData` with the correct details upfront rather than mutating after construction.

- [ ] **Step 4: Update status bar to show pending operations**

In `widgets/status_bar.rs`, show "Pushing..." or "Fetching..." when the corresponding background gate is set. Check `state.pending_background` and `state.pending_mutation`.

- [ ] **Step 5: Run tests**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: push/fetch with independent background gating + status bar spinner"
```

---

## Task 8: Describe modal — $EDITOR path

Ship the `$EDITOR` path first (de-risk strategy from spec). Inline `tui-textarea` modal is Task 9.

**Files:**
- Modify: `crates/lajjzy-tui/src/dispatch.rs`
- Modify: `crates/lajjzy-tui/src/input.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn open_describe_emits_suspend_for_editor() {
    let mut state = test_state_with_described_change("aaa", "old description");
    let effects = dispatch(&mut state, Action::OpenDescribe);
    assert_eq!(effects.len(), 1);
    assert!(matches!(&effects[0], Effect::SuspendForEditor { change_id, initial_text }
        if change_id == "aaa" && initial_text == "old description"));
}

#[test]
fn editor_complete_emits_describe_effect() {
    let mut state = test_state_with_changes(&["aaa"]);
    let effects = dispatch(&mut state, Action::EditorComplete {
        change_id: "aaa".into(),
        text: "new description".into(),
    });
    assert_eq!(effects.len(), 1);
    assert!(matches!(&effects[0], Effect::Describe { change_id, text }
        if change_id == "aaa" && text == "new description"));
    assert_eq!(state.pending_mutation, Some(MutationKind::Describe));
}
```

- [ ] **Step 2: Implement dispatch arms**

```rust
Action::OpenDescribe => {
    if state.pending_mutation.is_some() { return vec![]; }
    if let Some(cid) = state.selected_change_id().map(String::from) {
        let text = state.selected_detail()
            .map(|d| d.description.clone())
            .unwrap_or_default();
        return vec![Effect::SuspendForEditor {
            change_id: cid,
            initial_text: text,
        }];
    }
}
Action::EditorComplete { change_id, text } => {
    state.pending_mutation = Some(MutationKind::Describe);
    return vec![Effect::Describe { change_id, text }];
}
```

- [ ] **Step 3: Key binding already added in Task 6**

`e` → `Action::OpenDescribe` was added in Task 6 step 3.

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 5: Manual smoke test**

Run lajjzy in a jj repo, press `e` on a change. `$EDITOR` should open with the current description. Edit, save, quit. The graph should update with the new description.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: describe via \$EDITOR — suspend/resume TUI"
```

---

## Task 9: Describe modal — inline tui-textarea

Add the inline editor as an alternative to `$EDITOR`. This replaces `SuspendForEditor` as the default for `e`, with `Shift-E` escalating to `$EDITOR`.

**Files:**
- Create: `crates/lajjzy-tui/src/widgets/describe.rs`
- Modify: `crates/lajjzy-tui/src/dispatch.rs`
- Modify: `crates/lajjzy-tui/src/input.rs`
- Modify: `crates/lajjzy-tui/src/render.rs`
- Modify: `crates/lajjzy-tui/src/widgets/mod.rs`

- [ ] **Step 1: Create describe widget**

```rust
// crates/lajjzy-tui/src/widgets/describe.rs
use ratatui::prelude::*;
use tui_textarea::TextArea;

pub struct DescribeWidget<'a> {
    pub editor: &'a TextArea<'a>,
}

impl Widget for DescribeWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Render the TextArea within a bordered block
        let block = ratatui::widgets::Block::bordered()
            .title(" Describe (Ctrl-S save | Esc cancel | Shift-E editor) ");
        self.editor.widget().render(area, buf);
        // Note: TextArea has its own render method — check tui-textarea docs
    }
}
```

Check `tui-textarea` API for how to render. It may use `textarea.widget()` which returns an impl Widget, or it may render directly into a frame.

- [ ] **Step 2: Change `OpenDescribe` to open inline modal**

```rust
Action::OpenDescribe => {
    if state.pending_mutation.is_some() { return vec![]; }
    if let Some(cid) = state.selected_change_id().map(String::from) {
        let text = state.selected_detail()
            .map(|d| d.description.clone())
            .unwrap_or_default();
        let mut editor = TextArea::new(text.lines().map(String::from).collect());
        // Position cursor at end
        editor.move_cursor(tui_textarea::CursorMove::End);
        state.modal = Some(Modal::Describe { change_id: cid, editor });
    }
}
```

- [ ] **Step 3: Add describe modal key routing in `input.rs`**

In `map_modal_event`, add a branch before the common handlers:

```rust
if let Modal::Describe { .. } = modal {
    return match (event.code, event.modifiers) {
        (KeyCode::Char('s'), KeyModifiers::CONTROL) => Some(Action::DescribeSave),
        (KeyCode::Enter, KeyModifiers::CONTROL) => Some(Action::DescribeSave),
        (KeyCode::Esc, _) => Some(Action::ModalDismiss),
        (KeyCode::Char('E'), KeyModifiers::SHIFT) => Some(Action::DescribeEscalateEditor),
        _ => None, // Let tui-textarea handle it directly
    };
}
```

Note: `tui-textarea` handles its own key events via `textarea.input(event)`. When `map_modal_event` returns `None`, the event loop should pass the raw `KeyEvent` to the `TextArea`. This requires the event loop to check for unmapped keys and forward them. Add this to the event loop in `main.rs`, **after** the existing modal/event dispatch:

```rust
// In the key handling section of run_loop:
let action = if let Some(ref modal) = state.modal {
    map_modal_event(key_event, modal)
} else {
    map_event(key_event, state.focus, state.detail_mode)
};

if let Some(action) = action {
    let effects = dispatch(state, action);
    execute_effects(terminal, state, executor, effects);
} else if let Some(Modal::Describe { ref mut editor, .. }) = state.modal {
    // Unhandled key in describe modal — forward to tui-textarea
    editor.input(key_event);
}
```

The two borrows (`ref modal` for `map_modal_event`, then `ref mut editor` for `input`) are in separate branches, so no double-borrow occurs.

- [ ] **Step 4: Add `DescribeSave` and `DescribeEscalateEditor` handlers**

```rust
Action::DescribeSave => {
    if let Some(Modal::Describe { change_id, editor }) = state.modal.take() {
        let text = editor.lines().join("\n");
        state.pending_mutation = Some(MutationKind::Describe);
        return vec![Effect::Describe { change_id, text }];
    }
}
Action::DescribeEscalateEditor => {
    if let Some(Modal::Describe { change_id, editor }) = state.modal.take() {
        let text = editor.lines().join("\n");
        return vec![Effect::SuspendForEditor { change_id, initial_text: text }];
    }
}
```

Note: using `state.modal.take()` avoids the borrow conflict between `ref editor` (immutable borrow of `state.modal`) and setting `state.modal = None` (mutable borrow). The `take()` pattern moves the modal out, giving us ownership of `change_id` and `editor`.

- [ ] **Step 5: Render the describe modal**

In `render.rs`, when `state.modal` is `Describe`, render the `DescribeWidget` overlaying the detail pane area. Check how existing modals are rendered and follow the same pattern.

- [ ] **Step 6: Run tests**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 7: Manual smoke test**

Press `e` — inline editor appears. Type, `Ctrl-S` saves. Press `e` again, `Shift-E` opens `$EDITOR` with current text.

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat: inline describe modal with tui-textarea + Shift-E editor escalation"
```

---

## Task 10: Bookmark set modal + bookmark delete

**Files:**
- Create: `crates/lajjzy-tui/src/widgets/bookmark_input.rs`
- Modify: `crates/lajjzy-tui/src/dispatch.rs`
- Modify: `crates/lajjzy-tui/src/input.rs`
- Modify: `crates/lajjzy-tui/src/render.rs`
- Modify: `crates/lajjzy-tui/src/widgets/mod.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn open_bookmark_set_opens_modal() {
    let mut state = test_state_with_bookmarked_change("aaa", "main");
    let effects = dispatch(&mut state, Action::OpenBookmarkSet);
    assert!(effects.is_empty());
    assert!(matches!(state.modal, Some(Modal::BookmarkInput { .. })));
}

#[test]
fn bookmark_input_confirm_emits_effect() {
    let mut state = test_state_with_changes(&["aaa"]);
    state.modal = Some(Modal::BookmarkInput {
        change_id: "aaa".into(),
        input: "feature-x".into(),
        completions: vec![],
        cursor: 0,
    });
    let effects = dispatch(&mut state, Action::BookmarkInputConfirm);
    assert!(matches!(&effects[0], Effect::BookmarkSet { change_id, name }
        if change_id == "aaa" && name == "feature-x"));
    assert!(state.modal.is_none());
}

#[test]
fn bookmark_delete_in_picker_emits_effect() {
    // When 'd' is pressed in bookmark picker modal
    let mut state = test_state_with_bookmarked_change("aaa", "main");
    state.modal = Some(Modal::BookmarkPicker {
        bookmarks: vec![("main".into(), "aaa".into())],
        cursor: 0,
    });
    // Need a BookmarkDelete action variant
    let effects = dispatch(&mut state, Action::BookmarkDelete);
    assert!(matches!(&effects[0], Effect::BookmarkDelete { name } if name == "main"));
}
```

- [ ] **Step 2: Implement bookmark dispatch**

```rust
Action::OpenBookmarkSet => {
    if let Some(cid) = state.selected_change_id().map(String::from) {
        let existing = state.selected_detail()
            .and_then(|d| d.bookmarks.first().cloned())
            .unwrap_or_default();
        // Collect all bookmark names for completion
        let all_bookmarks: Vec<String> = state.graph.details.values()
            .flat_map(|d| d.bookmarks.iter().cloned())
            .collect();
        state.modal = Some(Modal::BookmarkInput {
            change_id: cid,
            input: existing,
            completions: all_bookmarks,
            cursor: 0,
        });
    }
}
Action::BookmarkInputChar(c) => {
    if let Some(Modal::BookmarkInput { input, .. }) = &mut state.modal {
        input.push(c);
    }
}
Action::BookmarkInputBackspace => {
    if let Some(Modal::BookmarkInput { input, .. }) = &mut state.modal {
        input.pop();
    }
}
Action::BookmarkInputConfirm => {
    if let Some(Modal::BookmarkInput { change_id, input, .. }) = state.modal.take() {
        if !input.is_empty() {
            state.pending_mutation = Some(MutationKind::BookmarkSet);
            return vec![Effect::BookmarkSet { change_id, name: input }];
        }
    }
}
```

- [ ] **Step 3: Add `BookmarkDelete` action and dispatch**

Add `BookmarkDelete` to the `Action` enum. In dispatch:

```rust
Action::BookmarkDelete => {
    if state.pending_mutation.is_some() { return vec![]; }
    if let Some(Modal::BookmarkPicker { bookmarks, cursor, .. }) = &state.modal {
        if let Some((name, _)) = bookmarks.get(*cursor) {
            let name = name.clone();
            state.modal = None;
            state.pending_mutation = Some(MutationKind::BookmarkDelete);
            return vec![Effect::BookmarkDelete { name }];
        }
    }
}
```

- [ ] **Step 4: Add key routing for bookmark modals**

In `input.rs` `map_modal_event`, add `BookmarkInput` routing (similar to `FuzzyFind`):

```rust
if let Modal::BookmarkInput { .. } = modal {
    return match event.code {
        KeyCode::Esc => Some(Action::ModalDismiss),
        KeyCode::Enter => Some(Action::BookmarkInputConfirm),
        KeyCode::Backspace => Some(Action::BookmarkInputBackspace),
        KeyCode::Char(c) if event.modifiers == KeyModifiers::NONE
            || event.modifiers == KeyModifiers::SHIFT => Some(Action::BookmarkInputChar(c)),
        _ => None,
    };
}
```

In `BookmarkPicker` modal, add `d` for delete:

```rust
// In the non-fuzzy modal branch:
(KeyCode::Char('d'), KeyModifiers::NONE) if matches!(modal, Modal::BookmarkPicker { .. }) => {
    Some(Action::BookmarkDelete)
}
```

- [ ] **Step 5: Create bookmark input widget**

```rust
// crates/lajjzy-tui/src/widgets/bookmark_input.rs
// Single-line input rendered at the bottom of the screen
```

- [ ] **Step 6: Render bookmark input modal**

In `render.rs`, render `BookmarkInput` at the bottom of the screen (not centered).

- [ ] **Step 7: Run tests**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat: bookmark set modal + bookmark delete from picker"
```

---

## Task 11: Update CLAUDE.md and help widget

**Files:**
- Modify: `CLAUDE.md`
- Modify: `crates/lajjzy-tui/src/widgets/help.rs`

- [ ] **Step 1: Update CLAUDE.md architectural constraints**

Replace the dispatch section with:

```markdown
## Architectural Constraints

- **Dispatch purity (enforced):** `dispatch()` takes `(&mut AppState, Action)` and returns `Vec<Effect>`. Never calls backend methods or performs I/O.
- **Effect executor boundary:** Effects executed in `lajjzy-cli` only. `lajjzy-tui` defines the `Effect` enum but never executes effects.
- **Facade boundary:** `lajjzy-tui` never imports `RepoBackend`, `std::process::Command`, or jj-lib. `$EDITOR` launch handled by event loop in `lajjzy-cli`.
- **Mutation gate:** At most one local mutation in flight, enforced by `AppState.pending_mutation`. Background ops (push/fetch) gated independently.
- **Three interaction patterns:** Every mutation declares its slot (Instant, Mini-modal, Background). New patterns require design justification.
- **Three concurrency lanes:** Local mutations, push, fetch — independent gates, no blocking between lanes. Last `GraphLoaded` wins.
```

Update the Key Patterns section too.

- [ ] **Step 2: Update help widget with new key bindings**

Add mutation keys to `HelpContext::Graph` content. Update `line_count()` accordingly.

- [ ] **Step 3: Run tests**

Run: `cargo test`
Expected: all pass (including help widget tests that check line counts).

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "docs: update CLAUDE.md constraints + help widget with M2 keys"
```

---

## Task 12: Final integration testing + cleanup

**Files:**
- All files (clippy/fmt pass)

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Fix any issues.

- [ ] **Step 3: Run fmt**

Run: `cargo fmt --check`
Fix any issues.

- [ ] **Step 4: Manual end-to-end test**

In a real jj repo with a git remote:
1. Navigate graph (`j`/`k`) — works
2. `n` — creates new change, cursor moves to it
3. `e` — inline describe opens, type description, `Ctrl-S` saves
4. `d` — abandons the change
5. `u` — undoes the abandon, change reappears
6. `Ctrl-R` — redoes the abandon
7. `u` — undo again to restore
8. `S` — squash into parent
9. `B` — bookmark set modal, type name, Enter
10. `b` — bookmark picker, `d` to delete
11. `f` — fetch (if remote configured)
12. `P` — push (if remote configured)
13. `Ctrl-E` — edit/switch working copy
14. Verify spinners show for push/fetch
15. Verify status messages appear and clear on next keypress

- [ ] **Step 5: Commit any fixes**

```bash
git add -A && git commit -m "fix: integration testing cleanup"
```

- [ ] **Step 6: Final commit count and test count**

Run: `cargo test 2>&1 | grep 'test result'` to verify total test count.
Expected: significantly more than 104 (original count).
