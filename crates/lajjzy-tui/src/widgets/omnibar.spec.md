---
managed-file: crates/lajjzy-tui/src/widgets/omnibar.rs
intent: >
  A ratatui widget that renders the omnibar overlay: a bordered, titled input panel showing the current query string and, below it, either a scrollable list of inline completion items (when completions are present) or a scrollable list of fuzzy-matched graph changes (change ID, author, description). The title adapts to four states: idle, typing-search, active-revset, and completing. The selected row is highlighted with a REVERSED style. When no matches exist, a context-sensitive empty-state message is shown. Completions whose insert_text ends with '(' are styled in cyan to indicate revset functions.
intent-approved: false
intent-hash: 4a4096c7ce71
distilled-from:
  - path: crates/lajjzy-tui/src/widgets/omnibar.rs
    hash: f1fa3070df82
non-goals:
  - Handling keyboard input or mutating query/cursor state — the widget is purely a renderer
  - Filtering or ranking matches — it receives pre-computed match indices and displays them verbatim
  - Persisting or emitting any actions — callers own dispatch; the widget only writes to the ratatui Buffer
depends-on:
  - crates/lajjzy-tui/src/action.spec.md
  - crates/lajjzy-core/src/types.spec.md
---

## Purpose

Callers obtain a fully-rendered omnibar panel in a ratatui `Buffer`. The widget is constructed with all display data pre-computed and called once via `Widget::render`. It exposes no mutable state and produces no side effects.

## Behavior

1. **Border and title**: Renders a full `Borders::ALL` block with a blue border. The title string is chosen from four states based on inputs:
   - Completions non-empty → `" / Tab: accept | Enter: submit as revset "`
   - Completions empty, query empty, no active revset → `" / Search or Revset "`
   - Completions empty, `has_active_revset` true → `" / Revset (active) "`
   - Completions empty, query non-empty, no active revset → `" / Search (Enter to filter as revset) "`

2. **Input line**: The first inner row always renders `"/ "` (blue) followed by the raw query string, followed by a `"|"` cursor indicator (dark gray). This row is always present if inner height is non-zero.

3. **Results area**: All inner rows below the input line are the results area. If inner height is 1 or less, no results are rendered.

4. **Completion mode** (completions slice non-empty): Renders each `CompletionItem` by its `display_text`. Items whose `insert_text` ends with `'('` are rendered in cyan; all others use the default style. The selected item (at `completion_cursor`) is highlighted with `Modifier::REVERSED` across the full inner width.

5. **Fuzzy-match mode** (completions slice empty): Renders each matched graph line as `[change_id (yellow)]  [author (blue)]  [description]`. The selected match (at `cursor`) is highlighted with `Modifier::REVERSED` across the full inner width.

6. **Empty-state messages**: When in fuzzy-match mode and `matches` is empty:
   - If `query` is empty → renders `"(no changes)"` in dark gray on the first results row.
   - If `query` is non-empty → renders `"(no matches)"` in dark gray on the first results row.
   - No message is rendered if results height is zero.

7. **Scrolling**: Both modes scroll the list so the selected item is always visible. The scroll offset is `max(0, cursor - results_height + 1)`, clipping the list to the visible window.

8. **Zero-height guard**: If the inner area height is zero after the border, the widget returns immediately without writing any content.

## Constraints

- `matches` contains indices into `graph.lines`; accessing out-of-range indices is silently skipped (loop breaks when `idx >= matches.len()`).
- `completion_cursor` and `cursor` are caller-supplied `usize` values; the widget does not validate that they are in bounds — out-of-range values result in no row being highlighted.
- The widget borrows all data for its lifetime `'a`; it owns nothing and performs no allocation beyond ratatui span/line construction.
- Highlight spans the full `inner.width` columns, not just the text width.
- The `u16` cast for row Y positions uses `#[expect(clippy::cast_possible_truncation)]`, meaning callers are responsible for keeping result counts within `u16` range.

## Dependencies

- `ratatui` — `Buffer`, `Rect`, `Widget`, `Block`, `Borders`, `Style`, `Color`, `Modifier`, `Line`, `Span`
- `lajjzy_core::types::GraphData` — source of graph lines and change details
- `crate::action::CompletionItem` — `{ insert_text: String, display_text: String }`
