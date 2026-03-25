---
managed_file: crates/lajjzy-tui/src/mouse.rs
version: 1
test_policy: "Write or extend tests for all mouse mapping paths"
---

# Mouse event mapper

## Purpose

Pure stateless mapper from crossterm `MouseEvent` to `Action` using cached
layout rects from the last render pass. No I/O, no state mutation.

## Dependencies

- `crossterm::event::{MouseButton, MouseEvent, MouseEventKind}`
- `ratatui::layout::Rect`
- `crate::action::Action`
- `crate::app::{AppState, DetailMode, LayoutRects, PanelFocus}`
- `crate::modal::Modal`

## Constants

- `SCROLL_LINES: usize = 3` — lines per scroll wheel tick

## Functions

### `pub fn map_mouse_event(event: MouseEvent, state: &AppState) -> Option<Action>`

Dispatches on `event.kind`:
- `Down(MouseButton::Left)` → `handle_click`
- `ScrollUp` → `handle_scroll(up=true)`
- `ScrollDown` → `handle_scroll(up=false)`
- All other events (Drag, Moved, Right, Middle) → `None`

### `fn handle_click(col, row, layout, state) -> Option<Action>`

Priority order:

1. **Modal guard**: If a modal is active:
   - If `Describe` or `BookmarkInput` → `None` (not click-dismissable)
   - If click outside `modal_area` → `ModalDismiss`
   - If click inside modal → `None`
   - Early return in all modal cases

2. **Mode guards**: If `target_pick.is_some()` → `None`. If `detail_mode` is
   HunkPicker, ConflictView, or DiffView → `None`.

3. **Graph inner**: hit_test against `layout.graph_inner` → `ClickGraphNode { line_index }`
   where `line_index = graph_scroll_offset + (row - graph_inner.y)`

4. **Detail inner**: hit_test against `layout.detail_inner` → `ClickDetailItem { index }`
   where `index = row - detail_inner.y`

5. **Graph border** (outer but not inner): → `ClickFocusGraph`

6. **Detail border** (outer but not inner): → `ClickFocusDetail`

7. Otherwise → `None`

### `fn handle_scroll(col, row, layout, state, up) -> Option<Action>`

1. **Modal scroll**: If modal is active and click is inside `modal_area` →
   `ModalMoveUp` or `ModalMoveDown`. If outside modal → `None`.

2. **Panel scroll**: Determine hovered pane via hit_test on `graph_outer` / `detail_outer`.
   Scroll targets the *hovered* pane, not the focused pane. →
   `ScrollUp { count: SCROLL_LINES, panel }` or `ScrollDown { count: SCROLL_LINES, panel }`

3. Otherwise → `None`

### `fn hit_test(rect: Rect, col: u16, row: u16) -> bool`

Half-open interval: `col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height`

## Tests

Test helpers:
- `sample_graph()` — 3-node GraphData
- `default_layout()` — graph at (0,0,40,20), detail at (40,0,40,20), inner rects shrunk by 1
- `make_event(kind, col, row)` — MouseEvent with NONE modifiers
- `make_state_with_layout(layout)` — AppState with given layout

Test coverage (22 tests):
1. `click_graph_inner_emits_click_node` — (5,1) → ClickGraphNode { line_index: 0 }
2. `click_graph_with_scroll_offset` — offset=10, (5,1) → line_index: 10
3. `click_detail_when_graph_focused_selects_item` — (45,5) → ClickDetailItem { index: 4 }
4. `click_graph_when_detail_focused_selects_node` — (5,5) → ClickGraphNode { line_index: 4 }
5. `click_graph_border_only_focuses` — (0,0) → ClickFocusGraph
6. `click_detail_border_only_focuses` — (40,0) → ClickFocusDetail
7. `click_detail_inner_when_focused_selects_item` — (45,3) → ClickDetailItem { index: 2 }
8. `scroll_up_in_graph` — ScrollUp { count: 3, panel: Graph }
9. `scroll_down_in_detail` — ScrollDown { count: 3, panel: Detail }
10. `scroll_in_modal_emits_modal_move_down` — inside modal → ModalMoveDown
11. `scroll_in_modal_emits_modal_move_up` — inside modal → ModalMoveUp
12. `scroll_outside_modal_ignored` — outside modal → None
13. `click_outside_modal_dismisses` — Help modal, click outside → ModalDismiss
14. `click_inside_modal_returns_none` — Help modal, click inside → None
15. `click_outside_describe_modal_ignored` — Describe modal, click outside → None
16. `click_outside_bookmark_input_ignored` — BookmarkInput, click outside → None
17. `mouse_drag_ignored` — Drag → None
18. `mouse_move_ignored` — Moved → None
19. `click_on_graph_border_focuses_graph` — (0,0) with Detail focus → ClickFocusGraph
20. `click_during_hunk_picker_ignored` — HunkPicker mode → None
21. `click_during_diff_view_ignored` — DiffView mode → None
22. `click_during_picking_mode_ignored` — target_pick active → None
23. `hit_test_basic` — boundary checks on a 20x10 rect
24. `right_click_ignored` — Right button → None
25. `middle_click_ignored` — Middle button → None
