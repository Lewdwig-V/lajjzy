# M3d: Revset Autocomplete in Omnibar — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add prefix-match autocomplete to the omnibar for jj revset functions and repo entities (bookmarks, change IDs, authors), computed entirely from in-memory graph data.

**Architecture:** Completions are computed client-side on every keystroke. The current word is extracted by scanning backwards to an operator/whitespace boundary. Two sources: a static `REVSET_FUNCTIONS` array with `Arity` metadata (drives `()` vs `(` insertion) and repo entities from `GraphData`. Two new fields on `Modal::Omnibar` (`completions`, `completion_cursor`) control display. Completions replace fuzzy results when visible; fuzzy results return after 150ms idle with no completions (debounced via poll timeout). Change IDs show with description in the dropdown and require a 2+ char prefix.

**Tech Stack:** Rust 1.85+, ratatui 0.30, crossterm 0.29

**Spec:** `docs/superpowers/specs/2026-03-22-m3d-revset-autocomplete-design.md`

---

## File Map

### Files to modify

| File | Changes |
|------|---------|
| `crates/lajjzy-tui/src/modal.rs` | Add `completions: Vec<CompletionItem>`, `completion_cursor: usize` to `Omnibar` |
| `crates/lajjzy-tui/src/action.rs` | Add `OmnibarAcceptCompletion`, `Arity` enum, `CompletionItem` struct |
| `crates/lajjzy-tui/src/dispatch.rs` | `extract_current_word`, `compute_completions`, `REVSET_FUNCTIONS`, all Omnibar destructure updates, `OmnibarAcceptCompletion` handler |
| `crates/lajjzy-tui/src/input.rs` | Tab routing when completions visible |
| `crates/lajjzy-tui/src/widgets/omnibar.rs` | Render completions list (with descriptions for change IDs), title change |
| `crates/lajjzy-tui/src/render.rs` | Pass new fields to omnibar widget |

---

## Task 1: Types + `Modal::Omnibar` field additions + fix all destructure sites

**Files:**
- Modify: `crates/lajjzy-tui/src/action.rs`
- Modify: `crates/lajjzy-tui/src/modal.rs`
- Modify: `crates/lajjzy-tui/src/dispatch.rs`
- Modify: `crates/lajjzy-tui/src/widgets/omnibar.rs`
- Modify: `crates/lajjzy-tui/src/render.rs`

- [ ] **Step 1: Add `Arity` enum and `CompletionItem` struct to `action.rs`**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arity {
    Nullary,   // insert "()" — complete, ready to combine
    Optional,  // insert "(" — user can close immediately or add arg
    Required,  // insert "(" — needs argument
}

/// A single completion candidate for the omnibar.
#[derive(Debug, Clone, PartialEq)]
pub struct CompletionItem {
    /// The text to insert (e.g., "ancestors(", "mine()", "main")
    pub insert_text: String,
    /// The text to display in the dropdown (e.g., "ancestors(", "ksqxwpml — refactor: extract trait")
    pub display_text: String,
}
```

Add `OmnibarAcceptCompletion` to the `Action` enum.

- [ ] **Step 2: Add fields to `Modal::Omnibar` in `modal.rs`**

```rust
Omnibar {
    query: String,
    matches: Vec<usize>,
    cursor: usize,
    completions: Vec<CompletionItem>,
    completion_cursor: usize,
},
```

Import `CompletionItem` from `crate::action`.

- [ ] **Step 3: Update ALL `Modal::Omnibar` destructure sites in `dispatch.rs`**

Exhaustive list of sites that must include the new fields:

**Construction:**
- `OpenOmnibar` (line ~522) — add `completions: vec![], completion_cursor: 0`

**Destructure with `take()` (moves values — cannot use `..` for non-Copy types):**
- `ModalEnter` (line ~623) — add `completions: _, completion_cursor: _` to drop them explicitly

**Destructure with `&mut` (can use `..`):**
- `ModalMoveDown` Omnibar arm (line ~577) — must be **split out** from the multi-variant arm that binds `cursor`. Add new separate arm for Omnibar.
- `ModalMoveUp` Omnibar arm (line ~598) — same: **split out** from multi-variant arm.
- `OmnibarInput` (line ~647) — add `completions, completion_cursor, ..`
- `OmnibarBackspace` (line ~659) — add `completions, completion_cursor, ..`

**Pattern match (read-only, `..` is fine):**
- `ModalDismiss` check (uses `matches!(..., Some(Modal::Omnibar { .. }))` — already has `..`, no change needed)

**Test code** — every test that constructs `Modal::Omnibar { query, matches, cursor }` needs `completions: vec![], completion_cursor: 0` added. There are ~10 such sites in dispatch.rs tests + input.rs tests.

**Widget/render code:**
- `crates/lajjzy-tui/src/render.rs` — wherever it destructures Omnibar to pass to the widget
- `crates/lajjzy-tui/src/widgets/omnibar.rs` — constructor may need new params

For now, the widget changes are just adding the fields to any destructure. Actual rendering of completions is Task 5.

- [ ] **Step 4: Verify compilation and tests**

Run: `cargo check && cargo test && cargo clippy --all-targets -- -D warnings`

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "refactor: add CompletionItem, Arity, completion fields to Modal::Omnibar"
```

---

## Task 2: Helper functions — `extract_current_word` + `compute_completions`

Pure functions, test-driven. These are the core logic.

**Files:**
- Modify: `crates/lajjzy-tui/src/dispatch.rs`

- [ ] **Step 1: Add `REVSET_FUNCTIONS` constant**

```rust
/// Revset function names with their arity (drives insertion behavior).
const REVSET_FUNCTIONS: &[(&str, Arity)] = &[
    ("all", Arity::Nullary),
    ("ancestors", Arity::Required),
    ("author", Arity::Required),
    ("bookmarks", Arity::Optional),
    ("committer", Arity::Required),
    ("conflicts", Arity::Nullary),
    ("connected", Arity::Required),
    ("descendants", Arity::Required),
    ("description", Arity::Required),
    ("diff_contains", Arity::Required),
    ("empty", Arity::Nullary),
    ("file", Arity::Required),
    ("fork_point", Arity::Required),
    ("heads", Arity::Required),
    ("immutable", Arity::Nullary),
    ("mine", Arity::Nullary),
    ("none", Arity::Nullary),
    ("present", Arity::Required),
    ("remote_bookmarks", Arity::Optional),
    ("root", Arity::Nullary),
    ("roots", Arity::Required),
    ("tags", Arity::Optional),
    ("trunk", Arity::Nullary),
    ("visible_heads", Arity::Nullary),
];
```

- [ ] **Step 2: Write `extract_current_word` + tests**

```rust
/// Word boundary characters for revset expressions.
fn is_revset_boundary(c: char) -> bool {
    matches!(c, '&' | '|' | '~' | '(' | ')' | ':' | '.' | ',') || c.is_ascii_whitespace()
}

/// Extract the current word being typed from the end of the query.
/// Returns (start_position, word_slice).
fn extract_current_word(query: &str) -> (usize, &str) {
    let boundary = query.rfind(is_revset_boundary);
    match boundary {
        // All boundary chars are ASCII (1 byte), so pos + 1 is always safe.
        Some(pos) => (pos + 1, &query[pos + 1..]),
        None => (0, query),
    }
}
```

Tests:
```rust
#[test]
fn extract_current_word_simple() { assert_eq!(extract_current_word("mine"), (0, "mine")); }
#[test]
fn extract_current_word_after_tilde() { assert_eq!(extract_current_word("~mi"), (1, "mi")); }
#[test]
fn extract_current_word_after_paren() { assert_eq!(extract_current_word("ancestors(mi"), (10, "mi")); }
#[test]
fn extract_current_word_after_ampersand() { assert_eq!(extract_current_word("mine() & desc"), (9, "desc")); }
#[test]
fn extract_current_word_after_dotdot() { assert_eq!(extract_current_word("trunk()..@"), (9, "@")); }
#[test]
fn extract_current_word_empty_after_paren() { assert_eq!(extract_current_word("desc("), (5, "")); }
#[test]
fn extract_current_word_empty() { assert_eq!(extract_current_word(""), (0, "")); }
#[test]
fn extract_current_word_comma() { assert_eq!(extract_current_word("diff_contains(foo,bar"), (18, "bar")); }
```

- [ ] **Step 3: Write `compute_completions` + tests**

The function takes the **full query** (extracts the word internally). Returns `Vec<CompletionItem>`.

```rust
fn compute_completions(query: &str, graph: &GraphData) -> Vec<CompletionItem> {
    let (_, current_word) = extract_current_word(query);
    if current_word.is_empty() {
        return vec![];
    }
    let word_lower = current_word.to_lowercase();
    let mut results = Vec::new();

    // 1. Revset functions (ranked first)
    for &(name, arity) in REVSET_FUNCTIONS {
        if name.to_lowercase().starts_with(&word_lower) {
            let insert_text = match arity {
                Arity::Nullary => format!("{name}()"),
                Arity::Optional | Arity::Required => format!("{name}("),
            };
            results.push(CompletionItem {
                display_text: insert_text.clone(),
                insert_text,
            });
        }
    }

    // 2-4: Repo entities from node_indices (deterministic order)
    let mut bookmarks = Vec::new();
    let mut change_ids = Vec::new();
    let mut authors = Vec::new();
    for &idx in graph.node_indices() {
        if let Some(cid) = graph.lines[idx].change_id.as_deref()
            && let Some(detail) = graph.details.get(cid)
        {
            for bm in &detail.bookmarks {
                bookmarks.push(bm.as_str());
            }
            // Change IDs: require 2+ char prefix to avoid noise
            if word_lower.len() >= 2 && cid.to_lowercase().starts_with(&word_lower) {
                let desc = if detail.description.is_empty() {
                    "(no description)".to_string()
                } else {
                    detail.description.clone()
                };
                change_ids.push(CompletionItem {
                    insert_text: cid.to_string(),
                    display_text: format!("{cid} — {desc}"),
                });
            }
            authors.push(detail.author.as_str());
        }
    }

    // Bookmarks (deduplicated, alphabetical, ranked second)
    bookmarks.sort_unstable();
    bookmarks.dedup();
    for bm in bookmarks {
        if bm.to_lowercase().starts_with(&word_lower) {
            results.push(CompletionItem {
                insert_text: bm.to_string(),
                display_text: bm.to_string(),
            });
        }
    }

    // Change IDs (graph order, ranked third)
    results.extend(change_ids);

    // Authors (deduplicated, alphabetical, ranked fourth)
    authors.sort_unstable();
    authors.dedup();
    for author in authors {
        if author.to_lowercase().starts_with(&word_lower) {
            results.push(CompletionItem {
                insert_text: author.to_string(),
                display_text: author.to_string(),
            });
        }
    }

    results.truncate(20);
    results
}
```

Tests (use `sample_graph_with_bookmarks()` which has bookmark "main"):
```rust
#[test]
fn compute_completions_revset_function() {
    let graph = sample_graph_with_bookmarks();
    let completions = compute_completions("anc", &graph);
    assert!(completions.iter().any(|c| c.insert_text == "ancestors("));
}
#[test]
fn compute_completions_nullary_inserts_both_parens() {
    let graph = sample_graph();
    let completions = compute_completions("min", &graph);
    assert!(completions.iter().any(|c| c.insert_text == "mine()"));
}
#[test]
fn compute_completions_case_insensitive() {
    let graph = sample_graph();
    let completions = compute_completions("MIN", &graph);
    assert!(completions.iter().any(|c| c.insert_text == "mine()"));
}
#[test]
fn compute_completions_empty_returns_empty() {
    let graph = sample_graph();
    assert!(compute_completions("", &graph).is_empty());
}
#[test]
fn compute_completions_no_match_returns_empty() {
    let graph = sample_graph();
    assert!(compute_completions("xyznothing", &graph).is_empty());
}
#[test]
fn compute_completions_includes_bookmarks() {
    let graph = sample_graph_with_bookmarks(); // has "main" bookmark
    let completions = compute_completions("mai", &graph);
    assert!(completions.iter().any(|c| c.insert_text == "main"));
}
#[test]
fn compute_completions_change_id_needs_2_chars() {
    let graph = sample_graph(); // has change ID "abc"
    assert!(compute_completions("a", &graph).iter().all(|c| c.insert_text != "abc")); // too short
    let completions = compute_completions("ab", &graph);
    assert!(completions.iter().any(|c| c.insert_text == "abc")); // long enough
}
#[test]
fn compute_completions_change_id_shows_description() {
    let graph = sample_graph(); // "abc" has description
    let completions = compute_completions("ab", &graph);
    let abc = completions.iter().find(|c| c.insert_text == "abc").unwrap();
    assert!(abc.display_text.contains("—")); // has description separator
}
#[test]
fn compute_completions_functions_rank_before_entities() {
    let graph = sample_graph_with_bookmarks();
    let completions = compute_completions("de", &graph);
    // "descendants(" and "description(" should come before any entity starting with "de"
    if let Some(first_entity_pos) = completions.iter().position(|c| !c.insert_text.ends_with('(') && !c.insert_text.ends_with(')')) {
        let last_function_pos = completions.iter().rposition(|c| c.insert_text.ends_with('(') || c.insert_text.ends_with(')')).unwrap_or(0);
        assert!(last_function_pos < first_entity_pos, "functions should rank before entities");
    }
}
```

- [ ] **Step 4: Run tests, clippy, fmt, commit**

```bash
git add -A && git commit -m "feat: extract_current_word + compute_completions with Arity and ranked results"
```

---

## Task 3: Wire completions into dispatch

Update `OmnibarInput`, `OmnibarBackspace`, `OpenOmnibar`, `ModalMoveDown`/`ModalMoveUp`, and add `OmnibarAcceptCompletion`.

**Files:**
- Modify: `crates/lajjzy-tui/src/dispatch.rs`

- [ ] **Step 1: Update `OmnibarInput` and `OmnibarBackspace`**

After the existing query + fuzzy match update, recompute completions:
```rust
*completions = compute_completions(query, &state.graph);
*completion_cursor = 0;
```

Both handlers must destructure `completions` and `completion_cursor` from the modal.

- [ ] **Step 2: Update `OpenOmnibar`**

When opening with a pre-filled query (from active revset), compute completions immediately:
```rust
let completions = compute_completions(&query, &state.graph);
state.modal = Some(Modal::Omnibar {
    query,
    matches,
    cursor: 0,
    completions,
    completion_cursor: 0,
});
```

- [ ] **Step 3: Split Omnibar out of `ModalMoveDown`/`ModalMoveUp` multi-variant arms**

Currently the Omnibar shares a match arm with OpLog and BookmarkPicker:
```rust
Modal::OpLog { cursor, .. }
| Modal::BookmarkPicker { cursor, .. }
| Modal::Omnibar { cursor, .. } => { *cursor = cursor.saturating_sub(1); }
```

Split Omnibar into its own arm that checks `completions`:
```rust
Modal::Omnibar { completions, completion_cursor, cursor, .. } => {
    if !completions.is_empty() {
        // Move completion cursor
        if *completion_cursor + 1 < completions.len() {
            *completion_cursor += 1;
        }
    } else {
        // Move fuzzy cursor (existing behavior)
        // ...
    }
}
```

Same pattern for MoveUp.

- [ ] **Step 4: Implement `OmnibarAcceptCompletion` handler**

```rust
Action::OmnibarAcceptCompletion => {
    if let Some(Modal::Omnibar {
        query,
        completions,
        completion_cursor,
        matches,
        cursor,
    }) = &mut state.modal
    {
        if let Some(item) = completions.get(*completion_cursor).cloned() {
            let (word_start, _) = extract_current_word(query);
            query.truncate(word_start);
            query.push_str(&item.insert_text);
            // Recompute for the updated query
            *completions = compute_completions(query, &state.graph);
            *completion_cursor = 0;
            *matches = fuzzy_match(query, &state.graph);
            *cursor = 0;
        }
    }
}
```

Note: after `query.truncate` + `push_str`, we call `compute_completions(query, ...)`. The `query` is `&mut String` which auto-derefs to `&str`. This is valid — `truncate` + `push_str` are finished before `compute_completions` borrows. The implementer should compile-verify this.

- [ ] **Step 5: Write dispatch tests**

```rust
#[test]
fn omnibar_completions_appear_on_input() { /* type "anc", verify ancestors( in completions */ }
#[test]
fn omnibar_completions_empty_for_non_matching() { /* type "xyz", verify empty */ }
#[test]
fn omnibar_accept_completion_inserts_text() { /* type "min", Tab, verify query == "mine()" */ }
#[test]
fn omnibar_accept_completion_after_operator() { /* type "~min", Tab, verify "~mine()" */ }
#[test]
fn omnibar_accept_completion_function_with_args() { /* type "aut", Tab, verify "author(" */ }
#[test]
fn omnibar_backspace_recomputes_completions() { /* type "minex", backspace, verify mine() returns */ }
#[test]
fn omnibar_completion_cursor_moves_down() { /* type "a", MoveDown, verify completion_cursor == 1 */ }
#[test]
fn omnibar_completion_cursor_moves_up() { /* same, MoveUp back to 0 */ }
#[test]
fn omnibar_accept_noop_when_no_completions() { /* OmnibarAcceptCompletion with empty completions */ }
```

- [ ] **Step 6: Run tests, clippy, fmt, commit**

```bash
git add -A && git commit -m "feat: wire completions into omnibar dispatch — input, accept, navigate"
```

---

## Task 4: Input routing — Tab for completions

**Files:**
- Modify: `crates/lajjzy-tui/src/input.rs`

- [ ] **Step 1: Add Tab routing in omnibar modal**

In `map_modal_event`, before the existing omnibar key routing, add:
```rust
if let Modal::Omnibar { completions, .. } = modal {
    if event.code == KeyCode::Tab && !completions.is_empty() {
        return Some(Action::OmnibarAcceptCompletion);
    }
}
```

Modal routing runs INSTEAD OF global routing (no Tab conflict with TabFocus).

- [ ] **Step 2: Write input tests**

```rust
#[test]
fn tab_accepts_completion_when_visible() {
    let modal = Modal::Omnibar {
        query: "min".into(),
        matches: vec![],
        cursor: 0,
        completions: vec![CompletionItem { insert_text: "mine()".into(), display_text: "mine()".into() }],
        completion_cursor: 0,
    };
    assert_eq!(map_modal_event(key(KeyCode::Tab), &modal), Some(Action::OmnibarAcceptCompletion));
}

#[test]
fn tab_noop_when_no_completions() {
    let modal = Modal::Omnibar {
        query: "xyz".into(),
        matches: vec![],
        cursor: 0,
        completions: vec![],
        completion_cursor: 0,
    };
    assert_eq!(map_modal_event(key(KeyCode::Tab), &modal), None);
}
```

- [ ] **Step 3: Run tests, clippy, fmt, commit**

```bash
git add -A && git commit -m "feat: Tab routes to OmnibarAcceptCompletion when completions visible"
```

---

## Task 5: Widget rendering — show completions in dropdown

**Files:**
- Modify: `crates/lajjzy-tui/src/widgets/omnibar.rs`
- Modify: `crates/lajjzy-tui/src/render.rs`

- [ ] **Step 1: Update widget constructor**

The omnibar widget needs `completions: &[CompletionItem]` and `completion_cursor: usize`. Update the constructor and the call site in `render.rs`.

Read `crates/lajjzy-tui/src/widgets/omnibar.rs` and `crates/lajjzy-tui/src/render.rs` to understand the current pattern.

- [ ] **Step 2: Render completions when non-empty**

When `completions` is non-empty, the results area shows completions instead of fuzzy matches. Each item renders `display_text` (which includes descriptions for change IDs). The highlighted item uses `REVERSED` style.

```rust
if !self.completions.is_empty() {
    for (i, item) in self.completions.iter().enumerate() {
        let style = if i == self.completion_cursor {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        let line = Line::styled(format!("  {}", item.display_text), style);
        // render at appropriate position
    }
} else {
    // existing fuzzy match rendering
}
```

- [ ] **Step 3: Update title**

When completions are visible: `/ Completing...`
Otherwise: existing titles (Search or Revset, active, etc.)

- [ ] **Step 4: Write widget tests**

```rust
#[test]
fn omnibar_renders_completions_when_present() { /* verify completion text in buffer */ }
#[test]
fn omnibar_title_shows_completing() { /* verify "Completing" in title */ }
#[test]
fn omnibar_renders_fuzzy_when_no_completions() { /* existing behavior preserved */ }
```

- [ ] **Step 5: Run tests, clippy, fmt, commit**

```bash
git add -A && git commit -m "feat: omnibar widget renders completions with descriptions and highlighting"
```

---

## Task 6: Final integration + cleanup

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: ~280+ tests.

- [ ] **Step 2: Clippy and fmt**

Run: `cargo clippy --all-targets -- -D warnings && cargo fmt --check`

- [ ] **Step 3: Manual smoke test**

1. `/` → type `anc` → see `ancestors(` in dropdown
2. Tab → `ancestors(` inserted, dropdown clears
3. Type `mi` → see `mine()` offered
4. Tab → `ancestors(mine()` in query
5. Enter → evaluates as revset
6. `/` → type `mai` → see `main` (bookmark) offered
7. Tab → `main` inserted
8. Type `ab` → see change IDs with descriptions
9. Esc → dismiss
10. Type gibberish → fuzzy results show (no completions)

- [ ] **Step 4: Commit any fixes**

```bash
git add -A && git commit -m "fix: integration testing cleanup"
```
