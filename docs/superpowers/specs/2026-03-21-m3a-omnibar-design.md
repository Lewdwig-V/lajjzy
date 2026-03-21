# M3a: Omnibar — Revset-First Search/Filter Bar

**Date:** 2026-03-21
**Status:** Draft
**Depends on:** M2 (complete)

## Motivation

The current fuzzy-find (`/`) is client-side only — it matches against change descriptions and IDs already loaded in the graph. jj's revset language is one of its most powerful features (`mine() & ~empty()`, `ancestors(main) & ~immutable()`), but there's no way to use it from the TUI. The omnibar unifies search and revset filtering into a single input: type to fuzzy-find, press Enter to try it as a revset.

## Scope

### In scope (M3a)

- Replace `Modal::FuzzyFind` with `Modal::Omnibar`
- Live fuzzy matching while typing (same as current)
- Revset evaluation on Enter via `Effect::EvalRevset`
- Graceful fallback to fuzzy jump when revset parse fails
- `AppState.active_revset` with status bar breadcrumb
- Pre-fill omnibar with active revset when reopening
- Empty Enter clears active revset filter
- `RepoBackend::load_graph` gains `revset: Option<&str>` parameter
- Post-mutation refreshes respect the active revset

### Out of scope (M3b+)

- Live revset evaluation while typing (needs effect cancellation)
- Revset history / recent queries
- Revset syntax highlighting
- Revset autocomplete

## Core Behavior

### Two modes, one input

**While typing:** Instant client-side fuzzy matching against change descriptions, IDs, and authors. Results appear live, navigable with `j`/`k`/`Ctrl-N`/`Ctrl-P`. This is the "navigate to a change" flow — identical to current fuzzy-find.

**On Enter:** The input is tried as a jj revset via `Effect::EvalRevset { query }`. The executor calls `backend.load_graph(Some(&query))`, which runs `jj log -r <query>`. If jj returns successfully, the graph refilters to show only matching changes — the modal closes, `active_revset` is set, and a breadcrumb appears in the status bar. If jj returns an error (invalid revset), the behavior falls back to the current fuzzy-find Enter: jump to the selected fuzzy match, close the modal, no refilter.

### Active filter state

- `AppState.active_revset: Option<String>` — `None` means default revset, `Some(expr)` means filtered.
- Status bar shows the active revset when set: `revset: mine() & ~empty()`
- Opening `/` while a filter is active pre-fills the omnibar with the current expression. The user can edit it (refine), clear it (backspace all, Enter), or Escape to keep it.
- Empty Enter clears the filter: sets `active_revset = None` and emits `Effect::LoadGraph { revset: None }`.

### Escape behavior

Escape in the omnibar always means "dismiss without changes" — the current filter (if any) is preserved. Consistent with every other modal.

## Type Changes

### Replace `Modal::FuzzyFind` with `Modal::Omnibar`

```rust
pub enum Modal {
    // ... existing variants ...
    Omnibar {
        query: String,
        matches: Vec<usize>, // graph line indices, live fuzzy results
        cursor: usize,
    },
}
```

Same shape as `FuzzyFind` — same fields, different name. The fuzzy matching behavior during typing is unchanged. The revset behavior is entirely in what happens on Enter.

### AppState additions

```rust
pub struct AppState {
    // ... existing fields ...

    /// Active revset filter. None = default revset. Shown in status bar breadcrumb.
    pub active_revset: Option<String>,

    /// Fuzzy match fallback index. Set when omnibar Enter emits EvalRevset,
    /// consumed by RevsetLoaded on failure. Cleared on any other action.
    pub(crate) omnibar_fallback_idx: Option<usize>,
}
```

### New Action variants

```rust
enum Action {
    // Replace:
    //   OpenFuzzyFind → OpenOmnibar
    //   FuzzyInput(char) → OmnibarInput(char)
    //   FuzzyBackspace → OmnibarBackspace

    OpenOmnibar,
    OmnibarInput(char),
    OmnibarBackspace,

    /// Result of revset evaluation attempt. Carries the query string
    /// so dispatch can set active_revset on success.
    RevsetLoaded {
        query: String,
        generation: u64,
        result: Result<GraphData, String>,
    },
}
```

### New Effect variant

```rust
enum Effect {
    // ... existing variants ...

    /// Try evaluating a revset expression. Executor calls load_graph(Some(&query)).
    /// On success: sends RevsetLoaded with Ok(graph).
    /// On failure: sends RevsetLoaded with Err(error).
    EvalRevset { query: String },
}
```

Separate from `LoadGraph` because the dispatch handler is different: `RevsetLoaded` sets `active_revset` on success and falls back to fuzzy jump on failure. `GraphLoaded` does neither.

## Dispatch Logic

### `OpenOmnibar`

```rust
Action::OpenOmnibar => {
    let query = state.active_revset.clone().unwrap_or_default();
    let matches = if query.is_empty() {
        state.graph.node_indices().to_vec()
    } else {
        fuzzy_match(&query, &state.graph)
    };
    state.modal = Some(Modal::Omnibar { query, matches, cursor: 0 });
}
```

### `OmnibarInput` / `OmnibarBackspace`

Identical to current `FuzzyInput` / `FuzzyBackspace` — update `query`, recompute `matches`, reset cursor.

### `ModalEnter` for Omnibar

```rust
Some(Modal::Omnibar { query, matches, cursor }) => {
    if query.is_empty() {
        // Clear active revset, restore default view
        if state.active_revset.is_some() {
            state.active_revset = None;
            return vec![Effect::LoadGraph { revset: None }];
        }
        // No active revset and empty query: just close
    } else {
        // Non-empty: try as revset
        state.omnibar_fallback_idx = matches.get(cursor).copied();
        return vec![Effect::EvalRevset { query }];
    }
}
```

### `RevsetLoaded`

```rust
Action::RevsetLoaded { query, generation, result } => {
    match result {
        Ok(new_graph) => {
            state.active_revset = Some(query);
            // Install graph via recursive dispatch (reuses cursor positioning logic)
            let nested = dispatch(state, Action::GraphLoaded { generation, result: Ok(new_graph) });
            debug_assert!(nested.is_empty());
        }
        Err(_) => {
            // Not a valid revset — fall back to fuzzy jump
            if let Some(idx) = state.omnibar_fallback_idx.take() {
                state.cursor = idx;
                state.reset_detail();
            }
        }
    }
}
```

## Backend Changes

### `RepoBackend::load_graph` gains revset parameter

```rust
pub trait RepoBackend: Send + Sync {
    fn load_graph(&self, revset: Option<&str>) -> Result<GraphData>;
    // ... rest unchanged ...
}
```

`JjCliBackend` implementation — add `-r <revset>` to `jj log` args when `Some`:

```rust
fn load_graph(&self, revset: Option<&str>) -> Result<GraphData> {
    let op_id = /* ... same ... */;
    let mut args = vec!["log", "--summary", "--color=never", "-T", template];
    if let Some(rev) = revset {
        args.extend(["-r", rev]);
    }
    // ... rest unchanged ...
}
```

### Post-mutation refreshes respect active revset

The `EffectExecutor` stores `active_revset: Mutex<Option<String>>`. The event loop updates it after dispatching `RevsetLoaded(Ok(...))` or a revset-clear `GraphLoaded`. The executor reads it in `run_mutation` for the post-op `load_graph` call:

```rust
fn run_mutation(/* ... */) {
    match f() {
        Ok(message) => {
            let revset = executor_revset.lock().unwrap().clone();
            let graph = Some((generation, backend.load_graph(revset.as_deref()).map_err(|e| e.to_string())));
            let _ = tx.send(Action::RepoOpSuccess { op, message, graph });
        }
        // ...
    }
}
```

This ensures that after abandoning a change with `mine() & ~empty()` active, the refreshed graph still shows only the user's non-empty changes.

## Key Bindings

No new keys. `/` maps to `OpenOmnibar` (was `OpenFuzzyFind`). All modal keys (Esc, Enter, j/k, Ctrl-N/P, Backspace, Char) route identically to current fuzzy-find.

## Widget and Rendering

### `OmnibarWidget` replaces `FuzzyFindWidget`

Same visual layout — centered overlay, query input at top, scrollable results below. Context-sensitive title:

- No active revset, empty query: `/ Search or Revset`
- Non-empty query: `/ Search (Enter to filter as revset)`
- Pre-filled with active revset: `/ Revset (active)`

```
┌─ / Search or Revset ─────────────────────┐
│ > mine() & ~empty()                       │
│                                           │
│   ksqxwpml  fix: resolve parser bug       │
│   ytoqrzxn  feat: add retry logic         │
│   vlpmrokx  refactor: extract trait       │
└───────────────────────────────────────────┘
```

### Status bar breadcrumb

When `active_revset` is `Some`, the status bar shows it. Priority: error > status_message > active revset > pending ops > change info.

## Testing Strategy

### Dispatch tests

1. `open_omnibar_empty_when_no_filter` — query empty, all matches shown
2. `open_omnibar_prefills_active_revset` — active revset pre-fills query
3. `omnibar_input_and_backspace` — renamed from fuzzy tests
4. `omnibar_enter_empty_clears_revset` — emits `LoadGraph { revset: None }`, clears `active_revset`
5. `omnibar_enter_empty_no_revset_just_closes` — no effect, modal closed
6. `omnibar_enter_nonempty_emits_eval_revset` — emits `EvalRevset`, stores fallback
7. `revset_loaded_success_sets_active_revset` — graph replaced, `active_revset` set
8. `revset_loaded_failure_falls_back_to_fuzzy_jump` — fallback consumed, cursor jumps
9. `revset_loaded_failure_no_fallback_is_noop` — no crash, silent ignore

### Backend tests

10. `load_graph_with_revset_filters_results` — pass `@`, verify subset
11. `load_graph_with_invalid_revset_returns_error` — pass garbage, verify `Err`

### Input tests

12. `omnibar_key_routing` — renamed from fuzzy tests
13. `/` maps to `OpenOmnibar`

### Widget tests

14. `omnibar_renders_query_and_results` — renamed from fuzzy widget test
15. `omnibar_title_shows_active_hint` — pre-filled shows "(active)"

### Migration

~6 existing `FuzzyFind`/`FuzzyInput`/`FuzzyBackspace` tests renamed to `Omnibar*`. No tests lost.

## File Changes

| File | Changes |
|------|---------|
| `crates/lajjzy-tui/src/action.rs` | Replace `OpenFuzzyFind`/`FuzzyInput`/`FuzzyBackspace` with `OpenOmnibar`/`OmnibarInput`/`OmnibarBackspace`. Add `RevsetLoaded`. |
| `crates/lajjzy-tui/src/effect.rs` | Add `EvalRevset { query: String }` |
| `crates/lajjzy-tui/src/modal.rs` | Replace `FuzzyFind` with `Omnibar` |
| `crates/lajjzy-tui/src/app.rs` | Add `active_revset`, `omnibar_fallback_idx` |
| `crates/lajjzy-tui/src/dispatch.rs` | Rename fuzzy handlers → omnibar. Add `RevsetLoaded` handler. Update `ModalEnter` for omnibar. |
| `crates/lajjzy-tui/src/input.rs` | Rename `FuzzyFind` → `Omnibar` in routing. `/` → `OpenOmnibar`. |
| `crates/lajjzy-tui/src/render.rs` | Rename `FuzzyFind` → `Omnibar` in modal rendering. |
| `crates/lajjzy-tui/src/widgets/fuzzy_find.rs` | Rename to `omnibar.rs`. Update title logic. |
| `crates/lajjzy-tui/src/widgets/mod.rs` | Rename module. |
| `crates/lajjzy-tui/src/widgets/status_bar.rs` | Show `active_revset` breadcrumb. |
| `crates/lajjzy-core/src/backend.rs` | `load_graph` gains `revset: Option<&str>` |
| `crates/lajjzy-core/src/cli.rs` | Pass `-r <revset>` to `jj log` when `Some`. |
| `crates/lajjzy-cli/src/main.rs` | Add `active_revset: Mutex<Option<String>>` to executor. Handle `EvalRevset` effect. Update `run_mutation` to read active revset. |
