---
source-spec: crates/lajjzy-tui/src/mouse.spec.md
target-language: rust
ephemeral: true
complexity: standard
---

# Concrete Spec: mouse.rs

## Strategy

One public entry point dispatching to two private handlers. All hit-testing via a shared helper.

### `map_mouse_event`

```
fn map_mouse_event(event, state) -> Option<Action>:
    let col = event.column
    let row = event.row
    let layout = &state.layout
    match event.kind:
        Down(Left) => handle_click(col, row, layout, state)
        ScrollUp => handle_scroll(col, row, layout, state, up=true)
        ScrollDown => handle_scroll(col, row, layout, state, up=false)
        _ => None  # Drag, Moved, Right, Middle all ignored
```

### `handle_click`

Priority-ordered cascade with early returns:

```
fn handle_click(col, row, layout, state) -> Option<Action>:
    # 1. Modal guard
    if modal active:
        if Describe or BookmarkInput => None (not click-dismissable)
        if click outside modal_area => ModalDismiss
        else => None (click inside modal, swallow)
        # Always return here — no fall-through to panel logic

    # 2. Mode guards
    if target_pick active => None
    if detail_mode in {HunkPicker, ConflictView, DiffView} => None

    # 3. Graph inner => ClickGraphNode { line_index: scroll_offset + viewport_row }
    # 4. Detail inner => ClickDetailItem { index: row - detail_inner.y }
    # 5. Graph outer (border) => ClickFocusGraph
    # 6. Detail outer (border) => ClickFocusDetail
    # 7. None
```

Key: inner checks come before outer checks. A click at (1,1) hits graph_inner first, not graph_outer. The cascade naturally handles this because `hit_test(inner)` is checked before `hit_test(outer)`.

Line index computation: `absolute_line = layout.graph_scroll_offset + (row - layout.graph_inner.y) as usize`.

### `handle_scroll`

```
fn handle_scroll(col, row, layout, state, up) -> Option<Action>:
    if modal active:
        if modal_area exists and hit_test passes => ModalMoveUp/Down
        else => None
    # Panel detection via hit_test on outer rects
    panel = Graph if over graph_outer, Detail if over detail_outer, else None
    if panel => ScrollUp/Down { count: SCROLL_LINES, panel }
    else => None
```

Scroll targets the **hovered** pane (determined by hit_test), not the focused pane.

### `hit_test`

```
fn hit_test(rect, col, row) -> bool:
    col >= rect.x && col < rect.x + rect.width &&
    row >= rect.y && row < rect.y + rect.height
```

Half-open interval on both axes. `x + width` and `y + height` are exclusive.

## Pattern

Pure function mapper with priority cascade. No design patterns beyond sequential if-else with early returns.

## Type Sketch

```
pub fn map_mouse_event(MouseEvent, &AppState) -> Option<Action>
fn handle_click(u16, u16, &LayoutRects, &AppState) -> Option<Action>
fn handle_scroll(u16, u16, &LayoutRects, &AppState, bool) -> Option<Action>
fn hit_test(Rect, u16, u16) -> bool
const SCROLL_LINES: usize = 3
```

## Test Strategy

Test helpers: `sample_graph()` (3-node), `default_layout()` (graph at 0,0,40,20 + detail at 40,0,40,20), `make_event(kind, col, row)`, `make_state_with_layout(layout)`.

25 test functions covering:
- Click hit-testing (inner vs border vs outside for both panels)
- Scroll offset arithmetic
- Modal dismiss/swallow logic (Help, Describe, BookmarkInput)
- Mode guards (HunkPicker, DiffView, ConflictView, PickingMode)
- Scroll targeting (hovered pane, not focused)
- Event type filtering (Drag, Moved, Right, Middle → None)
- hit_test boundary conditions
