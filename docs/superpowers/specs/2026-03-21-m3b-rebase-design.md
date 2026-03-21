# M3b: Rebase — Target Picker with In-Graph Selection

**Date:** 2026-03-21
**Status:** Draft
**Depends on:** M3a (complete)

## Motivation

Rebase is jj's primary tool for reorganizing change history. Without it, the TUI can create and destroy changes but can't rearrange them — stacks accumulate in the wrong order with no way to fix them. M3b adds rebase with an in-graph target picker that preserves spatial context.

## Scope

### In scope (M3b)

- `jj rebase -r` (single revision) via `r` key
- `jj rebase -s` (source + descendants) via `Ctrl-R` key
- In-graph target picking mode with inline filtering
- Excluded changes (source + descendants) dimmed and navigation-skipped
- Status bar blast radius preview for `-s` mode
- `ChangeDetail.parents` field for descendant computation
- Cursor restoration on cancel
- `--onto` destination mode only

### Out of scope

- `jj rebase -b` (branch mode) — hard to visualize, power users use CLI
- `--insert-after` / `--insert-before` — composition operations, different visual affordance
- Multi-parent rebase (onto multiple destinations)
- Stack reordering / drag-drop

## Interaction Flow

### Two keybindings, asymmetric safety

**`r` (Graph context)** — Single-revision rebase. Enters target-picking mode. Moves the selected change only; descendants reparent onto the change's parent. Status bar: `Rebase ksqxwpml onto → (j/k navigate, Enter confirm, Esc cancel)`

Safe, surgical, the default. A wrong `-r` rebase affects one change. Descendants stay connected. `undo` restores topology trivially.

**`Ctrl-R` (Graph context)** — Source+descendants rebase. Same picker, but status bar shows blast radius: `Rebase ksqxwpml + 3 descendants onto → (j/k navigate, Enter confirm, Esc cancel)`. The descendant count is static — computed once from the graph when picking mode opens, since descendants are defined by the source, not the destination.

**Key conflict resolution:** `R` (Shift-R) is currently a global key for Refresh. To avoid collision, rebase-with-descendants uses `Ctrl-R` instead. This displaces Redo (`Ctrl-R`), which moves to `Ctrl-Shift-R`. The rationale: Redo is rare (used after Undo, which is itself rare), while rebase-with-descendants is a primary workflow action that benefits from a single chord.

Deliberate escalation. A wrong `-s` rebase moves an entire subtree. Still reversible via `undo`, but the visual disruption is larger. The count preview makes the blast radius explicit.

### Target-picking mode

Two sub-modes:

**Browsing** — `j`/`k` navigate the graph (skipping excluded changes). `Enter` confirms target, emits rebase effect. `Escape` cancels picking mode, restores cursor to original position.

**Filtering** — Any non-navigation key while browsing starts typing a filter query. The fuzzy match runs against the same fields as the omnibar: change ID, description, and author (reuses the existing `fuzzy_match()` function). The graph dims non-matching changes. `j`/`k` cycle through matches only (using `Ctrl-J`/`Ctrl-K` or arrow keys since `j`/`k` are text input in filtering mode). `Enter` confirms. `Escape` clears the filter (back to Browsing). Second `Escape` cancels picking mode. Backspace on an empty query transitions back to Browsing mode.

### Excluded targets

The source change and (for `-s` mode) all its descendants are excluded from navigation. The graph renderer dims them. This prevents:
- "Rebase onto self" — a no-op
- "Rebase onto descendant" — creates a cycle, jj rejects with a confusing error

Filtering candidates by exclusion at the UI layer is more helpful than letting jj reject the operation after the fact.

### Graph refresh during picking

If a background fetch completes while picking is active, `GraphLoaded` refreshes the graph. If the source change disappears from the refreshed graph (e.g., remote force-push), picking mode is cancelled and the cursor is restored. The `GraphLoaded` handler checks `state.target_pick` and validates that the source is still present; if not, it clears `target_pick` and sets `status_message` to "Rebase cancelled: source change no longer exists."

### After confirmation

Standard instant mutation pattern. Mutation gate set, rebase effect emitted, graph refreshes on completion. Status bar: `Rebased ksqxwpml onto ytoqrzxn` or `Rebased ksqxwpml + 3 descendants onto ytoqrzxn`.

## Type Changes

### New types

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebaseMode {
    /// -r: move single revision, descendants reparent onto parent
    Single,
    /// -s: move revision + all descendants
    WithDescendants,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickingMode {
    /// j/k navigate, Enter confirms, Esc cancels
    Browsing,
    /// User typing a filter string. Graph dims non-matches.
    Filtering { query: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TargetPick {
    /// The change being rebased
    pub source: String,
    /// -r or -s
    pub mode: RebaseMode,
    /// Invalid targets (source + descendants for -s, just source for -r)
    pub excluded: HashSet<String>,
    /// Current picking sub-mode
    pub picking: PickingMode,
    /// Cursor position when picking started — restored on cancel
    pub original_cursor: usize,
    /// Descendant count for status bar (0 for Single mode)
    pub descendant_count: usize,
}
```

### AppState addition

```rust
pub struct AppState {
    // ... existing fields ...
    pub target_pick: Option<TargetPick>,
}
```

### ChangeDetail addition

```rust
pub struct ChangeDetail {
    // ... existing fields ...
    /// Parent change IDs, for computing descendants during target picking.
    pub parents: Vec<String>,
}
```

Captured from the jj log template: `parents.map(|p| p.change_id().short())`.

### New Action variants

```rust
// Trigger actions (Graph context)
RebaseSingle,            // r key
RebaseWithDescendants,   // R key

// Picking mode actions
PickConfirm,             // Enter — emit rebase effect
PickCancel,              // Escape — cancel or clear filter
PickFilterChar(char),    // Any char — start/extend filter
PickFilterBackspace,     // Backspace — shrink filter
```

Navigation during picking reuses `Action::MoveUp` / `Action::MoveDown` — dispatch already handles these, and the cursor-skip-excluded logic is added to the navigation handler when `target_pick` is `Some`.

### New Effect variants

```rust
Effect::RebaseSingle { source: String, destination: String },
Effect::RebaseWithDescendants { source: String, destination: String },
```

### New MutationKind variants

```rust
MutationKind::RebaseSingle,
MutationKind::RebaseWithDescendants,
```

Added to `clear_op_gate()` as local mutations.

## Dispatch Logic

### Entering picking mode

```rust
Action::RebaseSingle => {
    if state.pending_mutation.is_some() || state.target_pick.is_some() {
        state.status_message = Some("Operation in progress…".into());
        return vec![];
    }
    if let Some(cid) = state.selected_change_id().map(String::from) {
        state.target_pick = Some(TargetPick {
            source: cid.clone(),
            mode: RebaseMode::Single,
            excluded: HashSet::from([cid]),
            picking: PickingMode::Browsing,
            original_cursor: state.cursor(),
            descendant_count: 0,
        });
    }
}
```

For `RebaseWithDescendants`, compute descendants by finding all changes whose `parents` contain the source (direct children), then transitively collecting changes whose `parents` contain any already-collected descendant. This is a BFS from the source through the child→parent edges *in reverse* (parent→child direction). Populate `excluded` with source + all descendants, set `descendant_count`.

### Navigation during picking

`MoveUp` / `MoveDown` handlers check `state.target_pick`. If active, cursor skips changes in `excluded`. In `Filtering` mode, cursor also skips changes that don't match the filter query.

### PickConfirm

```rust
Action::PickConfirm => {
    if let Some(pick) = state.target_pick.take() {
        if let Some(dest) = state.selected_change_id().map(String::from) {
            if pick.excluded.contains(&dest) {
                state.target_pick = Some(pick); // restore — shouldn't happen
                return vec![];
            }
            state.pending_mutation = Some(match pick.mode {
                RebaseMode::Single => MutationKind::RebaseSingle,
                RebaseMode::WithDescendants => MutationKind::RebaseWithDescendants,
            });
            return vec![match pick.mode {
                RebaseMode::Single => Effect::RebaseSingle {
                    source: pick.source,
                    destination: dest,
                },
                RebaseMode::WithDescendants => Effect::RebaseWithDescendants {
                    source: pick.source,
                    destination: dest,
                },
            }];
        }
    }
}
```

### PickCancel

```rust
Action::PickCancel => {
    if let Some(ref mut pick) = state.target_pick {
        match pick.picking {
            PickingMode::Filtering { .. } => {
                // Clear filter, stay in picking mode
                pick.picking = PickingMode::Browsing;
            }
            PickingMode::Browsing => {
                // Cancel picking, restore cursor
                let original = pick.original_cursor;
                state.target_pick = None;
                state.cursor = original; // pub(crate) — valid in dispatch.rs (same crate)
            }
        }
    }
}
```

Note: `state.cursor` is `pub(crate)`. All picking dispatch handlers live in `dispatch.rs` within `lajjzy-tui`, so direct field access is valid. `TargetPick` and related types also live in `lajjzy-tui` (in `action.rs` or `app.rs`).

### PickFilterChar / PickFilterBackspace

```rust
Action::PickFilterChar(c) => {
    if let Some(ref mut pick) = state.target_pick {
        match &mut pick.picking {
            PickingMode::Browsing => {
                pick.picking = PickingMode::Filtering {
                    query: c.to_string(),
                };
            }
            PickingMode::Filtering { query } => {
                query.push(c);
            }
        }
        // Jump cursor to first matching non-excluded change
    }
}
```

## Input Routing

New `map_picking_event` function, called BEFORE modal and normal event routing:

```rust
pub fn map_picking_event(event: KeyEvent, picking: &PickingMode) -> Option<Action> {
    match picking {
        PickingMode::Browsing => match (event.code, event.modifiers) {
            (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => Some(Action::MoveDown),
            (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => Some(Action::MoveUp),
            (KeyCode::Enter, _) => Some(Action::PickConfirm),
            (KeyCode::Esc, _) => Some(Action::PickCancel),
            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                Some(Action::PickFilterChar(c))
            }
            _ => None, // swallow
        },
        PickingMode::Filtering { .. } => match event.code {
            KeyCode::Char('j') if event.modifiers == KeyModifiers::CONTROL => Some(Action::MoveDown),
            KeyCode::Char('k') if event.modifiers == KeyModifiers::CONTROL => Some(Action::MoveUp),
            KeyCode::Down => Some(Action::MoveDown),
            KeyCode::Up => Some(Action::MoveUp),
            KeyCode::Enter => Some(Action::PickConfirm),
            KeyCode::Esc => Some(Action::PickCancel),
            KeyCode::Backspace => Some(Action::PickFilterBackspace),
            KeyCode::Char(c)
                if event.modifiers == KeyModifiers::NONE
                    || event.modifiers == KeyModifiers::SHIFT =>
            {
                Some(Action::PickFilterChar(c))
            }
            _ => None,
        },
    }
}
```

Picking mode swallows all keys not in its map — no modals, no global shortcuts. Note: `map_picking_event` is defined in `input.rs` (lajjzy-tui) and called from `run_loop` in `main.rs` (lajjzy-cli).

**Revset interaction:** If an active revset filter is set when picking mode opens, the filtered graph is what the user navigates. This is correct — the user wants to pick from the changes they're currently viewing. The filter is neither cleared nor modified by picking mode.

**Event loop integration:**

```rust
// In run_loop, before modal/normal routing:
if let Some(ref pick) = state.target_pick {
    if let Some(action) = map_picking_event(key_event, &pick.picking) {
        let effects = dispatch(state, action);
        // ... execute effects
    }
    continue; // swallow unhandled keys
}
```

## Graph Rendering in Picking Mode

When `target_pick` is `Some`:

- **Excluded changes:** dimmed (dark gray). Cursor skips them.
- **Valid targets (Browsing):** normal rendering, cursor highlights as usual.
- **Valid targets (Filtering):** only changes matching the fuzzy query at full brightness. Non-matching valid targets dimmed. Cursor skips both excluded and non-matching.
- **Status bar:** context-dependent (see Interaction Flow section).
- **Detail pane:** works normally — shows file info for the currently highlighted target candidate.

No new widgets. The graph widget's style computation gains a few branches checking `target_pick` state.

## Backend Changes

### New `RepoBackend` methods

```rust
fn rebase_single(&self, source: &str, destination: &str) -> Result<String>;
fn rebase_with_descendants(&self, source: &str, destination: &str) -> Result<String>;
```

### JjCliBackend implementation

```rust
fn rebase_single(&self, source: &str, destination: &str) -> Result<String> {
    self.run_jj(&["rebase", "-r", source, "--onto", destination])?;
    Ok(format!("Rebased {source} onto {destination}"))
}

fn rebase_with_descendants(&self, source: &str, destination: &str) -> Result<String> {
    self.run_jj(&["rebase", "-s", source, "--onto", destination])?;
    Ok(format!("Rebased {source} + descendants onto {destination}"))
}
```

### Parent data in jj log template

Add `parents.map(|p| p.change_id().short())` to the template, joined by a space. Parsed into `ChangeDetail.parents: Vec<String>`.

## Testing Strategy

### Dispatch tests

1. `rebase_single_enters_picking_mode` — verify `TargetPick` fields
2. `rebase_with_descendants_enters_picking_mode` — verify excluded includes descendants
3. `rebase_suppressed_while_pending` — mutation gate blocks
4. `rebase_suppressed_while_already_picking` — can't nest
5. `pick_confirm_emits_rebase_single_effect` — correct effect + gate
6. `pick_confirm_emits_rebase_with_descendants_effect` — same for `-s`
7. `pick_cancel_browsing_exits_and_restores_cursor` — cursor back to original
8. `pick_cancel_filtering_returns_to_browsing` — query cleared, still picking
9. `pick_filter_char_transitions_to_filtering` — state transition
10. `pick_filter_backspace_shrinks_query` — filter editing
11. `pick_confirm_on_excluded_is_noop` — safety check
12. `cursor_skips_excluded_in_picking_mode` — navigation skips

### Backend tests

13. `rebase_single_on_real_repo` — create stack, rebase, verify topology
14. `rebase_with_descendants_on_real_repo` — rebase subtree, verify
15. `load_graph_includes_parent_ids` — verify `ChangeDetail.parents`

### Input tests

16. `picking_mode_browsing_key_routing` — j/k/Enter/Esc/char
17. `picking_mode_filtering_key_routing` — Ctrl-J/K, arrows, Backspace
18. `picking_mode_blocks_global_keys` — `/`, `?`, `b` swallowed

## File Changes

| File | Changes |
|------|---------|
| `crates/lajjzy-tui/src/action.rs` | Add `RebaseSingle`, `RebaseWithDescendants`, `PickConfirm`, `PickCancel`, `PickFilterChar`, `PickFilterBackspace`. Add `RebaseMode` (action-level enum). |
| `crates/lajjzy-tui/src/app.rs` | Add `PickingMode`, `TargetPick` (state types, alongside `AppState`). Add `target_pick: Option<TargetPick>` field. |
| `crates/lajjzy-tui/src/effect.rs` | Add `RebaseSingle`, `RebaseWithDescendants` |
| `crates/lajjzy-tui/src/dispatch.rs` | Picking mode dispatch, navigation skip-excluded, `PickConfirm`/`PickCancel`/`PickFilter*` handlers. Update `MoveUp`/`MoveDown` to skip excluded when picking. |
| `crates/lajjzy-tui/src/input.rs` | Add `map_picking_event`. Add `r` (RebaseSingle) and `Ctrl-R` (RebaseWithDescendants) in Graph context. Move Redo from `Ctrl-R` to `Ctrl-Shift-R`. |
| `crates/lajjzy-tui/src/render.rs` | Graph dimming for excluded/filtered changes in picking mode. Status bar picking-mode text. |
| `crates/lajjzy-tui/src/widgets/graph.rs` | Style computation checks `target_pick` for dimming. |
| `crates/lajjzy-tui/src/widgets/status_bar.rs` | Picking mode status text with blast radius count. |
| `crates/lajjzy-core/src/backend.rs` | Add `rebase_single`, `rebase_with_descendants` |
| `crates/lajjzy-core/src/cli.rs` | Implement rebase methods. Add `parents` to jj log template + parsing. |
| `crates/lajjzy-core/src/types.rs` | Add `parents: Vec<String>` to `ChangeDetail` |
| `crates/lajjzy-cli/src/main.rs` | Handle new effects in executor. Add `RebaseSingle`/`RebaseWithDescendants` to `MutationKind` match + `next_graph_generation`. `map_picking_event` in event loop before modal routing. |
