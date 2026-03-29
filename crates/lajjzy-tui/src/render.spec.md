---
managed-file: crates/lajjzy-tui/src/render.rs
intent: >
  Orchestrates the full-frame ratatui render pass for each display cycle. Divides the terminal area into a vertical stack of a main content region (1/3 graph panel left, 2/3 detail panel right) and a fixed 2-row status bar. Writes panel layout rectangles back into AppState so other subsystems can perform hit-testing. Constructs and renders a StatusBarWidget from live AppState fields (selected change, detail, error, status message, active revset, pending background ops, target pick, hunk picker, conflict view). When a modal is active, selects a modal-type-specific rendering strategy: Describe overlays the detail pane with no dim; BookmarkInput renders as a 4-row bottom bar anchored to the screen bottom with no dim; all other modals apply a DIM style modifier to the entire main content region behind a centered overlay. Clears the covered area before drawing any overlay. After rendering, caches the modal's computed Rect into AppState.layout.modal_area for downstream hit-testing. The render_modal helper dispatches to per-variant widget constructors (OpLogWidget, BookmarkPickerWidget, OmnibarWidget, HelpWidget, DescribeWidget, BookmarkInputWidget). The centered_rect utility computes a percentage-sized rectangle centred within a given area.
intent-approved: false
intent-hash: 51ac919cc603
distilled-from:
  - path: crates/lajjzy-tui/src/render.rs
    hash: f727d0e15003
non-goals:
  - Owning or mutating AppState fields other than layout rects and modal_area
  - Handling input events or dispatching actions
  - Executing backend effects or spawning subprocesses
depends-on:
  - crates/lajjzy-tui/src/app.spec.md
  - crates/lajjzy-tui/src/modal.spec.md
  - crates/lajjzy-tui/src/widgets/status_bar.spec.md
  - crates/lajjzy-tui/src/widgets/op_log.spec.md
  - crates/lajjzy-tui/src/widgets/bookmark_picker.spec.md
  - crates/lajjzy-tui/src/widgets/omnibar.spec.md
  - crates/lajjzy-tui/src/widgets/help.spec.md
  - crates/lajjzy-tui/src/widgets/describe.spec.md
  - crates/lajjzy-tui/src/widgets/bookmark_input.spec.md
---

## Purpose

`render(frame, state)` is the single entry point called by the event loop on every frame. Callers observe a fully composed terminal frame: graph and detail panels side by side, a status bar pinned to the bottom, and (when present) a modal overlay with appropriate background treatment. After the call, `state.layout` carries the current panel and modal `Rect` values ready for mouse hit-testing.

## Behavior

1. **Layout split** — The terminal area is split vertically into `[Min(1), Length(2)]`. The top region is split horizontally `[Ratio(1,3), Ratio(2,3)]` to yield graph (`main[0]`) and detail (`main[1]`) panes.
2. **Layout cache** — `state.layout` is updated via `LayoutRects::from_outer_rects(main[0], main[1])` before any panel renders.
3. **Panel render** — `panels::graph::render` is called with `main[0]`; `panels::detail::render` is called with `main[1]`.
4. **Status bar** — A `StatusBarWidget` is constructed from `state` fields and rendered into `outer[1]` (the 2-row strip).
5. **No-modal path** — When `state.modal` is `None`, rendering stops after the status bar. `state.layout.modal_area` is set to `None`.
6. **Modal: Describe** — Rendered directly over `main[1]` (detail pane). No DIM applied. The overlay area is cleared via `ratatui::widgets::Clear` before drawing.
7. **Modal: BookmarkInput** — Rendered as a 4-row bar at the bottom of the full frame (`frame.area()`). No DIM applied. Bar rect: `y = frame.height - 4`, `height = min(4, frame.height)`, full width. Area cleared before drawing.
8. **Modal: all others (OpLog, BookmarkPicker, Omnibar, Help, and any future variants)** — The DIM modifier is applied cell-by-cell to every cell in `outer[0]`. The modal is then rendered centered within `outer[0]`.
9. **Centering** — `centered_rect(percent_x, percent_y, area)` computes a rect occupying `percent_x`% of width and `percent_y`% of height, centered in `area` using equal-margin percentage splits.
10. **Modal size constants**:
    - `OpLog`: full `outer[0]`
    - `BookmarkPicker`, `Omnibar`: `centered_rect(60, 80, outer[0])`
    - `Help`: `centered_rect(50, 60, outer[0])`
    - All other variants: `centered_rect(60, 80, outer[0])`
11. **modal_area cache** — After rendering, `state.layout.modal_area` is set to the `Rect` that was used for the modal (matching the same sizing logic as render dispatch), or `None` if no modal.

## Constraints

- `render` is called with `&mut AppState` but only writes `state.layout`; it never mutates graph data, selection, or error fields.
- `STATUS_BAR_HEIGHT` is a compile-time constant (`2`). Layout arithmetic depends on it being non-zero.
- `render_modal` is a no-op if `state.modal` is `None`; it must not panic on that path.
- `centered_rect` must not produce a zero-sized or out-of-bounds rect when `percent_x` and `percent_y` are in `[0, 100]` and `area` is non-empty; the three-segment percentage layout is responsible for this constraint.
- No backend calls, filesystem I/O, or `std::process::Command` invocations anywhere in this module.

## Dependencies

- `ratatui::Frame`, `ratatui::layout::{Constraint, Layout, Rect}`, `ratatui::style::{Modifier, Style}`, `ratatui::widgets::Clear`
- `crate::app::{AppState, Modal, LayoutRects}`
- `crate::panels::graph::render`, `crate::panels::detail::render`
- `crate::widgets::status_bar::StatusBarWidget`
- `crate::widgets::op_log::OpLogWidget`
- `crate::widgets::bookmark_picker::BookmarkPickerWidget`
- `crate::widgets::omnibar::OmnibarWidget`
- `crate::widgets::help::HelpWidget`
- `crate::widgets::describe::DescribeWidget`
- `crate::widgets::bookmark_input::BookmarkInputWidget`
