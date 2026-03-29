---
managed_file: crates/lajjzy-tui/src/panels/graph.rs
intent: >
  Renders the commit-graph panel: wraps GraphWidget in a titled border whose colour reflects focus state, computes and stores the scroll offset into AppState.layout, and delegates all graph content rendering to the widget.
intent-approved: false
intent-hash: ed9538a15fbb
distilled-from:
  - path: crates/lajjzy-tui/src/panels/graph.rs
    hash: 7f9593989915
non-goals:
  - Does not handle keyboard input or cursor movement — those live in dispatch
  - Does not own or mutate graph data, PR status, or target-pick state beyond writing scroll_offset
  - Does not perform scrolling logic itself — scroll offset computation is delegated to GraphWidget.scroll_offset()
depends-on:
  - crates/lajjzy-tui/src/widgets/graph.spec.md
  - crates/lajjzy-tui/src/app.spec.md
---

## Purpose

`render(frame, state, area)` is the single public entry point. Callers pass a ratatui `Frame`, a mutable `AppState` reference, and a `Rect` describing available screen real estate. After the call the frame contains a fully drawn commit-graph panel and `state.layout.graph_scroll_offset` is up-to-date.

## Behavior

1. **Focus detection** — reads `state.focus == PanelFocus::Graph`; if true the border is drawn in `Color::Blue`, otherwise `Color::DarkGray`.
2. **Border rendering** — a `Block` with `Borders::ALL` and the title `"Changes"` is rendered over the full `area`.
3. **Inner area derivation** — `block.inner(area)` produces the content rect that excludes the border.
4. **Widget construction** — a `GraphWidget` is built from `&state.graph`, `state.cursor()`, and `&state.pr_status`, then augmented with `state.target_pick.as_ref()` via `.with_target_pick()`.
5. **Scroll offset write-back** — `graph_widget.scroll_offset(inner.height as usize)` is called and its result is stored in `state.layout.graph_scroll_offset` before the widget is rendered; the widget uses this height to clamp/derive the offset internally.
6. **Widget rendering** — the `GraphWidget` is rendered into `inner` via `frame.render_widget`.

## Constraints

- The panel writes exactly one field of `AppState`: `layout.graph_scroll_offset`. No other state mutations occur.
- The title is the fixed string `"Changes"`.
- Focus colour is binary: `Color::Blue` (focused) or `Color::DarkGray` (unfocused); no intermediate states.
- `block.inner()` is called before the block is rendered; ratatui requires this ordering because `render_widget` consumes the block.

## Dependencies

- `ratatui` — `Frame`, `Rect`, `Block`, `Borders`, `Style`, `Color`
- `crate::app::{AppState, PanelFocus}` — focus state and layout write-back
- `crate::widgets::graph::GraphWidget` — all commit-graph content rendering and scroll computation
