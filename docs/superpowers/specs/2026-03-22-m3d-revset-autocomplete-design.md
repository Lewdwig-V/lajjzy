# M3d: Revset Autocomplete in the Omnibar

**Date:** 2026-03-22
**Status:** Draft
**Depends on:** M3a (omnibar, complete)

## Motivation

The omnibar accepts jj revset expressions, but the user has to know the syntax by heart. Autocomplete makes revset functions and repo entities discoverable: type `anc` and see `ancestors(` offered, type `mai` and see your `main` bookmark. No docs lookup, no typos, faster query building.

## Scope

### In scope (M3d)

- Prefix-match autocomplete against revset functions and repo entities
- Two sources: static function list + repo-aware (bookmarks, change IDs, authors)
- Current word extracted by scanning backwards to operator/whitespace boundary
- Completions replace fuzzy results when visible; fuzzy results return when no matches
- Tab accepts, arrows cycle, Enter submits (unchanged)
- Functions with args insert open paren (`author(`), nullary insert both (`mine()`)
- Case-insensitive, capped at 20 candidates
- Computed entirely from in-memory `GraphData` — no backend calls

### Out of scope

- Cursor positioning within query (cursor always at end)
- Operator completion (`&`, `|`, `~`, `::`)
- Syntax error highlighting
- Incremental revset parsing
- Completion for revset aliases

## Completion Sources

### Revset functions (static)

Hardcoded list of jj revset functions. Changes only with jj version upgrades.

Nullary (insert with both parens):
`all()`, `empty()`, `heads()`, `immutable()`, `mine()`, `none()`, `root()`, `roots()`, `tags()`, `trunk()`, `visible_heads()`

With arguments (insert with open paren only):
`ancestors(`, `author(`, `bookmarks(`, `committer(`, `connected(`, `description(`, `descendants(`, `diff_contains(`, `file(`, `fork_point(`, `present(`, `remote_bookmarks(`

### Repo-aware entities (from GraphData)

Extracted from `state.graph` at completion time:
- **Bookmark names** — from `ChangeDetail.bookmarks` across all changes, deduplicated
- **Change IDs** — from `GraphLine.change_id` for all node lines
- **Author names** — from `ChangeDetail.author`, deduplicated

No backend call needed — all data is already loaded.

### Ordering

When multiple candidates match:
1. Revset functions (alphabetical)
2. Bookmarks (alphabetical)
3. Change IDs (graph order)
4. Authors (alphabetical)

Capped at 20 total items.

**Completions are context-free:** There is no awareness of being inside a function's argument list. Typing `author(al` will show both the function `all()` and any author named "alice". This is a known simplification — context-aware completion (only showing valid arguments inside a function call) is deferred.

**Author names with spaces:** An author like "Alice Smith" won't be discoverable by typing "Alice" inside a function argument (space is a word boundary). The user would need to start a new word. This is a known limitation consistent with revset syntax requiring quoting for multi-word values.

## Word Extraction

Completion operates on the **current word**, not the full query. Operators and parens are word boundaries.

**Word boundary characters:** `&`, `|`, `~`, `(`, `)`, `:`, whitespace

**Algorithm:** Scan backwards from end of query to find the last word boundary. Everything after it is the current word.

Examples:
- `mine` → current word = `mine`, start = 0
- `~mi` → current word = `mi`, start = 1
- `ancestors(mi` → current word = `mi`, start = 10
- `author(alice) & desc` → current word = `desc`, start = 16
- `` (empty) → current word = `""`, start = 0

```rust
fn extract_current_word(query: &str) -> (usize, &str) {
    let boundary = query.rfind(|c: char| {
        // ASCII-only boundaries — non-ASCII whitespace is not a word boundary.
        matches!(c, '&' | '|' | '~' | '(' | ')' | ':') || c.is_ascii_whitespace()
    });
    match boundary {
        Some(pos) => {
            // All boundary chars are ASCII (1 byte), so pos + 1 is always safe.
            (pos + 1, &query[pos + 1..])
        }
        None => (0, query),
    }
}
```

## Matching

Case-insensitive prefix match of the current word against all candidates.

When the current word has ≥1 character and matches at least one candidate, completions are shown. When the word is empty or matches nothing, fuzzy results show instead.

## Completion State

Added to the existing `Modal::Omnibar` variant:

```rust
pub enum Modal {
    Omnibar {
        query: String,
        matches: Vec<usize>,       // fuzzy match results (existing)
        cursor: usize,             // cursor in results list (existing)
        completions: Vec<String>,  // completion candidates for current word
        completion_cursor: usize,  // highlighted completion index
    },
}
```

When `completions` is non-empty, the widget renders completions. When empty, renders fuzzy matches. No new `AppState` field — completions live on the modal.

**All existing `Modal::Omnibar` destructure sites must be updated** to include the two new fields. Exhaustive list:
- `OpenOmnibar` — construction (add `completions: compute_completions(&query, &state.graph)`, `completion_cursor: 0` so completions appear immediately when reopening with pre-filled active revset)
- `ModalEnter` — destructure match arm
- `ModalMoveDown` / `ModalMoveUp` — need to move `completion_cursor` when completions visible
- `OmnibarInput` / `OmnibarBackspace` — recompute completions after query change
- Omnibar widget rendering — read completions for display

## Insertion

**Tab accepts the highlighted completion:**

1. Extract current word position: `(word_start, current_word)`
2. Truncate query to `word_start`
3. Append the completion text
4. Clear completions, reset completion_cursor
5. Recompute: the new query may trigger new completions or fuzzy results

**Function insertion format:**
- Nullary: `mine()` — both parens, user continues typing after `)`
- With arguments: `author(` — open paren only, user types the argument naturally

## Key Bindings

### When completions are visible

| Key | Action |
|-----|--------|
| `Tab` | Accept highlighted completion (insert into query) |
| `↓` / `Ctrl-N` | Move completion cursor down |
| `↑` / `Ctrl-P` | Move completion cursor up |
| `Esc` | Dismiss omnibar |
| `Enter` | Submit full query as revset (ignores completions) |
| Any char | Append to query, recompute completions |
| `Backspace` | Remove char, recompute completions |

### When no completions visible

Unchanged from M3a. Arrows navigate fuzzy matches. Tab is no-op.

### New Action

```rust
Action::OmnibarAcceptCompletion,  // Tab when completions visible
```

## Dispatch Logic

### Completion computation

Called after every `OmnibarInput` and `OmnibarBackspace`:

```rust
fn compute_completions(query: &str, graph: &GraphData) -> Vec<String> {
    let (_, current_word) = extract_current_word(query);
    if current_word.is_empty() {
        return vec![];
    }
    let word_lower = current_word.to_lowercase();
    let mut results = Vec::new();

    // 1. Revset functions
    for func in REVSET_FUNCTIONS {
        if func.to_lowercase().starts_with(&word_lower) {
            results.push(func.to_string());
        }
    }

    // 2-4: Repo entities — iterate node_indices() for deterministic order
    // (avoids HashMap iteration non-determinism from details.values())
    let mut bookmarks = Vec::new();
    let mut authors = Vec::new();
    for &idx in graph.node_indices() {
        if let Some(cid) = graph.lines[idx].change_id.as_deref()
            && let Some(detail) = graph.details.get(cid)
        {
            for bm in &detail.bookmarks {
                bookmarks.push(bm.as_str());
            }
            authors.push(detail.author.as_str());
            // Change IDs
            if cid.to_lowercase().starts_with(&word_lower) {
                results.push(cid.to_string());
            }
        }
    }

    // Bookmarks (deduplicated, alphabetical)
    bookmarks.sort_unstable();
    bookmarks.dedup();
    for bm in bookmarks {
        if bm.to_lowercase().starts_with(&word_lower) {
            results.push(bm.to_string());
        }
    }

    // Authors (deduplicated, alphabetical)
    authors.sort_unstable();
    authors.dedup();
    for author in authors {
        if author.to_lowercase().starts_with(&word_lower) {
            results.push(author.to_string());
        }
    }

    results.truncate(20);
    results
}
```

### `OmnibarInput` / `OmnibarBackspace` update

After updating the query and fuzzy matches (existing logic), also recompute completions:

```rust
*completions = compute_completions(query, &state.graph);
*completion_cursor = 0;
```

### `OmnibarAcceptCompletion`

```rust
Action::OmnibarAcceptCompletion => {
    if let Some(Modal::Omnibar { query, completions, completion_cursor, matches, cursor, .. }) = &mut state.modal {
        if let Some(completion) = completions.get(*completion_cursor).cloned() {
            let (word_start, _) = extract_current_word(query);
            query.truncate(word_start);
            query.push_str(&completion);
            // Recompute completions and fuzzy matches for updated query
            *completions = compute_completions(query, &state.graph);
            *completion_cursor = 0;
            *matches = fuzzy_match(query, &state.graph);
            *cursor = 0;
        }
    }
}
```

### Navigation with completions visible

`ModalMoveDown` / `ModalMoveUp` already handle the omnibar cursor. When completions are visible, they should move `completion_cursor` instead of `cursor`. This requires checking `completions.is_empty()` in the handler.

## Input Routing

In `map_modal_event`, the omnibar already routes `Ctrl-N`/`Ctrl-P`/arrows to `ModalMoveDown`/`ModalMoveUp`. Tab needs routing:

```rust
// In the omnibar branch of map_modal_event:
if let Modal::Omnibar { completions, .. } = modal {
    if event.code == KeyCode::Tab && !completions.is_empty() {
        return Some(Action::OmnibarAcceptCompletion);
    }
}
```

Tab is only routed to `OmnibarAcceptCompletion` when completions are visible. Otherwise it's swallowed (no-op).

**No Tab conflict with global `TabFocus`:** When a modal is active, the event loop calls `map_modal_event` instead of `map_event`. The global Tab → TabFocus mapping in `map_event` never runs while the omnibar is open. Modal routing takes complete precedence.

## Widget Rendering

The omnibar widget checks `completions`:

- **Non-empty:** Render completions list. Each item shows the completion text. Highlighted item has `REVERSED` style. Title: `/ Completing...`
- **Empty:** Render fuzzy matches as before.

The rendering code path is the same area — just switching between two data sources based on `completions.is_empty()`.

## Testing Strategy

### Dispatch tests

1. `omnibar_completions_appear_for_revset_prefix` — type `anc`, verify `ancestors(` in completions
2. `omnibar_completions_empty_for_non_matching` — type `xyz`, completions empty
3. `omnibar_completions_include_bookmarks` — graph has bookmark, type prefix, verify match
4. `omnibar_completions_include_authors` — type author prefix, verify match
5. `omnibar_completions_case_insensitive` — type `MIN`, verify `mine()` matches
6. `omnibar_accept_completion_inserts_text` — Tab replaces current word
7. `omnibar_accept_completion_clears_completions` — after Tab, completions empty
8. `omnibar_completions_after_operator` — `~mi` → `mi` matched
9. `omnibar_completions_after_paren` — `ancestors(mi` → `mi` matched
10. `omnibar_backspace_recomputes_completions` — narrow → widen
11. `omnibar_functions_with_args_insert_open_paren` — `author(` not `author()`
12. `omnibar_nullary_functions_insert_with_parens` — `mine()` not `mine(`

### Input tests

13. `tab_accepts_completion_when_visible` — Tab → `OmnibarAcceptCompletion`
14. `tab_noop_when_no_completions` — Tab → None

### Helper tests

15. `extract_current_word_simple` — `"mine"` → `(0, "mine")`
16. `extract_current_word_after_operator` — `"~mine"` → `(1, "mine")`
17. `extract_current_word_after_paren` — `"ancestors(mi"` → `(10, "mi")`
18. `extract_current_word_empty` — `""` → `(0, "")`

## File Changes

| File | Changes |
|------|---------|
| `crates/lajjzy-tui/src/modal.rs` | Add `completions: Vec<String>`, `completion_cursor: usize` to `Omnibar` variant |
| `crates/lajjzy-tui/src/action.rs` | Add `OmnibarAcceptCompletion` |
| `crates/lajjzy-tui/src/dispatch.rs` | `extract_current_word`, `compute_completions`, `REVSET_FUNCTIONS` constant, `OmnibarAcceptCompletion` handler, completion recomputation in `OmnibarInput`/`OmnibarBackspace`, `ModalMoveDown`/`ModalMoveUp` check completions |
| `crates/lajjzy-tui/src/input.rs` | Tab routing when completions visible |
| `crates/lajjzy-tui/src/widgets/omnibar.rs` | Render completions list when non-empty, title change |
| `crates/lajjzy-tui/src/render.rs` | Pass completions state if needed (may not need changes if widget reads from modal directly) |
