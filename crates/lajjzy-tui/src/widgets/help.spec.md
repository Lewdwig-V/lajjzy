---
managed_file: crates/lajjzy-tui/src/widgets/help.rs
intent: >
  Renders a context-sensitive, scrollable keyboard-shortcut reference overlay for the active UI panel (Graph, File List, Diff View, or Conflict View). Displays a bordered panel titled "Help — <Context>" with key bindings rendered as right-aligned yellow-bold key labels paired with plain-text descriptions, clipped to the visible area starting at the given scroll offset.
intent-approved: false
intent-hash: 2faa1c4e6956
distilled-from:
  - path: crates/lajjzy-tui/src/widgets/help.rs
    hash: 075ce2b8fa81
non-goals:
  - Handling input events or updating scroll state in response to keypresses
  - Dynamically reflecting runtime keybinding overrides or user-configured remaps
  - Rendering multiple help contexts simultaneously or providing tabbed navigation between contexts
depends-on:
  - crates/lajjzy-tui/src/widgets/conflict_view.spec.md
---

## Purpose

`HelpWidget` is a passive ratatui `Widget` that renders a read-only help overlay. Callers instantiate it with a `HelpContext` variant and a `scroll` offset, then pass it to ratatui's rendering pipeline. The rendered output appears as a bordered overlay titled "Help — <Context>" listing all keyboard shortcuts relevant to the named context.

## Behavior

1. **Context dispatch:** `HelpWidget::new(context, scroll)` stores the context and scroll offset. On `render`, the widget selects a static list of `(key, description)` pairs by matching `context` against four variants: `Graph`, `DetailFileList`, `DetailDiffView`, `ConflictView`.

2. **Graph context bindings:** Includes navigation (`j/k`, `g/G`, `@`), panel switching (`Tab`), mutation ops (`d`, `n`, `e`, `Ctrl-E`, `s`, `S`, `r`, `Ctrl-R`, `u`, `Ctrl-Shift-R`), bookmark and git ops (`B`, `b`, `P`, `f`), utility ops (`O`, `/`, `R`, `?`, `q`), advanced ops (`a`, `D`, `x`), GitHub ops (`F`, `W`), and a mouse section (separator rows with empty keys, followed by `Click` and `Scroll` entries).

3. **DetailFileList context bindings:** Navigation (`j/k`), open diff (`Enter`), return to graph (`Esc`), panel switch (`Tab`).

4. **DetailDiffView context bindings:** Scroll diff (`j/k`), hunk navigation (`n/N`), return to file list (`Esc`).

5. **ConflictView context bindings:** Accept left/right (`1`, `2`), conflict hunk navigation (`n/N`), scroll (`j/k`), launch external merge tool (`m`), confirm all (`Enter`), cancel (`Esc`).

6. **Border and title:** A full-border block (`Borders::ALL`) with a blue border is rendered around the widget area. The title string is `"Help — Graph"`, `"Help — File List"`, `"Help — Diff View"`, or `"Help — Conflict View"` depending on context.

7. **Key label styling:** Each key string is right-aligned within a 10-character field and styled `Color::Yellow` + `Modifier::BOLD`. Description text is unstyled. Key and description are separated by two plain spaces.

8. **Scrolling and clipping:** Rendering iterates rows from `scroll` to `scroll + inner_height`, stopping early if the binding list is exhausted. Rows outside the visible window are never written to the buffer. No bounds check is performed on `scroll` against list length; iteration simply exits on exhaustion.

9. **Mouse section separator:** In the `Graph` context, two entries with empty `""` key and description `""` and `"Mouse:"` act as visual separators. Two further entries with empty key strings display indented `Click` and `Scroll` descriptions without a key label.

## Constraints

- `HelpWidget` holds only `HelpContext` (Copy) and `usize`; it carries no heap allocation.
- All binding text is `&'static str`; no runtime string construction occurs for binding content.
- The key column is fixed at 10 characters (right-padded via `format!("{key:>10}")`); descriptions that exceed `inner.width - 12` are truncated by `buf.set_line`.
- `scroll + inner_height` must not overflow `usize`; callers are responsible for bounding scroll before construction.
- The widget is consumed on `render` (implements `Widget`, not `StatefulWidget`); scroll state is not mutated during rendering.

## Dependencies

- `ratatui` — `Buffer`, `Rect`, `Color`, `Modifier`, `Style`, `Line`, `Span`, `Block`, `Borders`, `Widget`
- `crate::app::HelpContext` (re-exported from `crate::modal`) — the four-variant enum driving context dispatch
