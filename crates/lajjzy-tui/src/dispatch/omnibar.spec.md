---
managed_file: crates/lajjzy-tui/src/dispatch/omnibar.rs
version: 1
test_policy: "Tests live in dispatch/mod.rs protected region — not in this file"
depends-on:
  - crates/lajjzy-tui/src/action.spec.md
  - crates/lajjzy-core/src/types.spec.md
---

# Omnibar completion engine

## Purpose

Provide revset-aware completion candidates for the omnibar. Matches against
revset function names, bookmark names, and change IDs. Pure functions, no I/O.

## Dependencies

- `lajjzy_core::types::GraphData`
- `crate::action::{Arity, CompletionItem}`

## Public API (crate-visible)

### `REVSET_FUNCTIONS: &[(&str, Arity)]`

Static table of jj revset function names with their arity. Sorted alphabetically.
25 entries covering: all, ancestors, author, bookmarks, committer, conflicts,
connected, descendants, description, diff_contains, empty, file, fork_point,
heads, immutable, mine, none, present, remote_bookmarks, root, roots, tags,
trunk, visible_heads.

### `is_revset_boundary(c: char) -> bool`

Returns true for characters that delimit revset tokens: `& | ~ ( ) : . , + -`
and ASCII whitespace.

### `extract_current_word(query: &str) -> (usize, &str)`

Find the word being typed at the end of the query string. Returns `(start_offset, word_slice)`.
Scans backward for the last boundary character. If none found, the entire query is the current word.

### `compute_completions(query: &str, graph: &GraphData) -> Vec<CompletionItem>`

Generate ranked completion candidates for the current word in the query.

**Algorithm:**
1. Extract current word via `extract_current_word`. If empty, return empty vec.
2. Case-insensitive prefix match against `REVSET_FUNCTIONS` — Nullary inserts `name()`, Optional/Required inserts `name(`
3. Collect bookmarks from graph (via `node_indices` traversal), deduplicated and sorted, prefix-matched
4. Collect change IDs prefix-matched (requires 2+ character prefix to reduce noise). Display format: `{cid} — {description}` (or "(no description)")
5. Ranking: functions first, bookmarks second, change IDs third
6. Truncate to 20 results

## Invariants

- Ordering is deterministic: functions (alphabetical from table), bookmarks (sorted+deduped), change IDs (node_indices order)
- Empty current word always returns empty results
- Single-character change ID prefixes are filtered out (noise reduction)
