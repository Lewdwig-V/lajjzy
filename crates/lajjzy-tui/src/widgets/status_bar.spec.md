---
managed_file: crates/lajjzy-tui/src/widgets/status_bar.rs
intent: >
  Render a priority-ordered status bar showing the most important contextual
  information for the current TUI state: hunk picker status, rebase picking
  prompt, errors, status messages, conflict view state, active revset filter,
  background operation indicators, or change detail.
intent-approved: false
intent-hash: 38d822b1f2ba
distilled-from:
  - path: crates/lajjzy-tui/src/widgets/status_bar.rs
    hash: 0bf50f74a39b
non-goals:
  - Does not handle user input — display only
  - Does not manage its own state — all data passed via constructor
  - Does not wrap or truncate text — relies on ratatui buffer width clipping
depends-on:
  - crates/lajjzy-tui/src/action.spec.md
  - crates/lajjzy-tui/src/app.spec.md
  - crates/lajjzy-core/src/types.spec.md
spec-changelog:
  - intent-hash: 38d822b1f2ba
    timestamp: 2026-03-28T00:00:00Z
    operation: elicit-distill-review
    prior-intent-hash: 38d822b1f2ba
  - intent-hash: 38d822b1f2ba
    timestamp: 2026-03-28T00:00:00Z
    operation: distill
    prior-intent-hash: null
---

## Purpose

`StatusBarWidget` is a ratatui `Widget` that renders a 1-2 line status bar at
the bottom of the TUI. It displays the single most important piece of contextual
information based on a strict priority cascade — higher-priority states completely
replace lower-priority ones.

## Behavior

The widget renders the FIRST matching state from this priority list:

1. **Hunk picker** (magenta) — "Split: N/M hunks selected → new change after {source}" or "Squash: N/M hunks from {source} → into {destination}" with keybinding hints
2. **Picking mode** (yellow) — "Rebase {source} onto →" with descendant count for WithDescendants mode; shows filter query when in Filtering mode
3. **Error** (red) — error message text
4. **Status message** (green) — transient success/info message
5. **Conflict view** (yellow) — "Hunk N/M: {state} | 1: left | 2: right | n: next | m: merge tool"
6. **Active revset** (cyan) — "revset: {query}"
7. **Background operations** (cyan) — "Pushing..." / "Fetching..." joined; falls through to render change detail on line 2 if height > 1
8. **Change detail** (default) — line 1: "{change_id} {commit_id}  {author} <{email}>"; line 2 (if height > 1): "{description}  [{bookmarks}]"
9. **Nothing** — empty buffer when no state to display

Each level early-returns after rendering, except background operations which
may render a second line.

## Constraints

- Zero-height area: immediate return, render nothing
- All rendering uses `buf.set_line` — single-line writes to the buffer
- Colors are hardcoded per priority level (not configurable)
- The widget borrows all data — no owned state, no allocations beyond format strings
- `#[expect(clippy::too_many_arguments)]` on the constructor
- `#[expect(clippy::too_many_lines)]` on the render method

## Dependencies

- `std::collections::HashSet`
- `ratatui::{Buffer, Rect, Color, Style, Line, Widget}`
- `lajjzy_core::types::ChangeDetail`
- `crate::action::{BackgroundKind, HunkPickerOp, RebaseMode}`
- `crate::app::{ConflictView, HunkPicker, HunkResolution, PickingMode, TargetPick}`

## Changelog

- **2026-03-28 elicit-distill-review** — Non-goals ratified, intent confirmed, no uncertainties
- **2026-03-28 distill** — Initial spec distilled from existing implementation (0bf50f74a39b)
