---
source-spec: crates/lajjzy-tui/src/input.spec.md
target-language: rust
ephemeral: true
complexity: standard
---

# Concrete Spec: input.rs

## Strategy

Three pure functions, each a nested `match` tree. No state, no allocation beyond return values.

### `map_event`

```
fn map_event(event, focus, detail_mode) -> Option<Action>:
    # Phase 1: global keys (early return)
    match (event.code, event.modifiers):
        # Each global key has a guard: suppressed during HunkPicker and ConflictView
        # Exception: '?' (OpenHelp) is always available, no guard
        # Exception: Ctrl-C routes to HunkCancel/DetailBack/Quit based on detail_mode

    # Phase 2: focus dispatch
    match focus:
        Graph => match (code, mods) to graph-specific actions
        Detail => match detail_mode:
            FileList => file list keys
            DiffView => diff view keys
            ConflictView => conflict view keys
            HunkPicker => hunk picker keys
```

Pattern: global keys use `if detail_mode != X && detail_mode != Y` guards inline in the match arms. The `R` key additionally checks `!m.contains(KeyModifiers::CONTROL)` to avoid clashing with Ctrl-R.

Redo key: `Ctrl-Shift-R` matches as `(Char('r'), m) if m == CONTROL | SHIFT`. Fallback arm `(Char('R'), CONTROL)` handles terminals that report Ctrl-Shift-R as Ctrl+uppercase-R.

### `map_modal_event`

```
fn map_modal_event(event, modal) -> Option<Action>:
    # Describe modal: early return (Ctrl-S / Ctrl-Enter / Alt-Enter → Save, Esc → Dismiss, Shift-E → Escalate)
    # BookmarkInput modal: early return (Esc/Enter/Backspace/Char routing)
    # Common modal keys: Esc/Enter/Up/Down (early return on match)
    # Omnibar Tab completion: guard on !completions.is_empty()
    # Branch: is_omnibar → text input routing (Backspace, Ctrl-N/P, Char)
    #         else → vim-style navigation (q/j/k, context-specific dismiss keys)
```

Pattern: `let is_omnibar = matches!(modal, Modal::Omnibar { .. })` computed once, branching to two terminal match blocks.

The Omnibar Tab completion check uses `if let Modal::Omnibar { completions, .. } = modal && event.code == KeyCode::Tab && !completions.is_empty()` (let-chain syntax).

### `map_picking_event`

```
fn map_picking_event(event, picking) -> Option<Action>:
    match picking:
        Browsing => j/k/Down/Up/Enter/Esc/Char(c) with NONE|SHIFT
        Filtering => Ctrl-J/K for nav, Down/Up, Enter/Esc, Backspace, Char(c) with NONE|SHIFT
```

Browsing mode: plain chars go to `PickFilterChar(c)` (starts filtering).
Filtering mode: `Ctrl-J`/`Ctrl-K` for navigation (since plain j/k would append to filter).

## Pattern

Pure function mapper. No design patterns beyond exhaustive match dispatch.

Suppress `clippy::too_many_lines` on `map_event` via `#[expect(...)]`.

## Type Sketch

```
pub fn map_event(KeyEvent, PanelFocus, DetailMode) -> Option<Action>
pub fn map_modal_event(KeyEvent, &Modal) -> Option<Action>
pub fn map_picking_event(KeyEvent, &PickingMode) -> Option<Action>
```

All inputs are borrowed or Copy. No allocations in any path.

## Test Strategy

Test helpers: `key(code)` and `key_mod(code, mods)` constructors. Convenience wrappers `map_graph`, `map_file_list`, `map_diff_view`, `map_hunk_picker`, `map_conflict_view` fix the focus/mode arguments.

42 test functions covering:
- Global key routing across focus modes
- Per-mode key routing (graph mutations, detail navigation, diff navigation)
- Modal-specific routing (Describe, BookmarkInput, Omnibar, OpLog, Help, BookmarkPicker)
- Picking mode routing (browsing vs filtering)
- Suppression guards (quit/tab suppressed during HunkPicker/ConflictView)
- Context-dependent keys (same key → different action by focus)
