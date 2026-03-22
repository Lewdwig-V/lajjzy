use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::action::Action;
use crate::app::{AppState, DetailMode, LayoutRects, PanelFocus};
use crate::modal::Modal;

/// Map a crossterm mouse event to an Action using cached layout rects.
pub fn map_mouse_event(event: MouseEvent, state: &AppState) -> Option<Action> {
    let col = event.column;
    let row = event.row;
    let layout = &state.layout;

    match event.kind {
        MouseEventKind::Down(MouseButton::Left) => handle_click(col, row, layout, state),
        MouseEventKind::ScrollUp => handle_scroll(col, row, layout, state, true),
        MouseEventKind::ScrollDown => handle_scroll(col, row, layout, state, false),
        _ => None,
    }
}

const SCROLL_LINES: usize = 3;

fn handle_click(col: u16, row: u16, layout: &LayoutRects, state: &AppState) -> Option<Action> {
    // Modal: click-outside-to-dismiss, except Describe and BookmarkInput
    if let Some(modal) = &state.modal {
        if matches!(modal, Modal::Describe { .. } | Modal::BookmarkInput { .. }) {
            return None;
        }
        if let Some(modal_rect) = layout.modal_area
            && !hit_test(modal_rect, col, row)
        {
            return Some(Action::ModalDismiss);
        }
        return None;
    }

    // Picking mode and hunk picker/conflict view — mouse disabled
    if state.target_pick.is_some() {
        return None;
    }
    if matches!(
        state.detail_mode,
        DetailMode::HunkPicker | DetailMode::ConflictView | DetailMode::DiffView
    ) {
        return None;
    }

    // Click in graph inner area — focus + select in one action (lazygit style)
    if hit_test(layout.graph_inner, col, row) {
        let viewport_row = (row - layout.graph_inner.y) as usize;
        let absolute_line = layout.graph_scroll_offset + viewport_row;
        return Some(Action::ClickGraphNode {
            line_index: absolute_line,
        });
    }

    // Click in detail inner area — focus + select in one action
    if hit_test(layout.detail_inner, col, row) {
        let row_offset = (row - layout.detail_inner.y) as usize;
        return Some(Action::ClickDetailItem { index: row_offset });
    }

    // Click on graph border (not inner) — just focus
    if hit_test(layout.graph_outer, col, row) {
        return Some(Action::ClickFocusGraph);
    }
    // Click on detail border — just focus
    if hit_test(layout.detail_outer, col, row) {
        return Some(Action::ClickFocusDetail);
    }

    None
}

fn handle_scroll(
    col: u16,
    row: u16,
    layout: &LayoutRects,
    state: &AppState,
    up: bool,
) -> Option<Action> {
    if state.modal.is_some() {
        if let Some(modal_rect) = layout.modal_area
            && hit_test(modal_rect, col, row)
        {
            return Some(if up {
                Action::ModalMoveUp
            } else {
                Action::ModalMoveDown
            });
        }
        return None;
    }
    // Determine which pane the scroll is over — scroll targets the hovered pane,
    // not the focused pane, so scrolling an unfocused pane works correctly.
    let panel = if hit_test(layout.graph_outer, col, row) {
        Some(PanelFocus::Graph)
    } else if hit_test(layout.detail_outer, col, row) {
        Some(PanelFocus::Detail)
    } else {
        None
    };
    if let Some(panel) = panel {
        return Some(if up {
            Action::ScrollUp {
                count: SCROLL_LINES,
                panel,
            }
        } else {
            Action::ScrollDown {
                count: SCROLL_LINES,
                panel,
            }
        });
    }
    None
}

fn hit_test(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::layout::Rect;

    use super::*;
    use crate::action::{Action, DetailMode, PanelFocus};
    use crate::app::{AppState, LayoutRects};
    use crate::modal::{HelpContext, Modal};
    use lajjzy_core::types::{ChangeDetail, GraphData, GraphLine};

    // ── helpers ──────────────────────────────────────────────────────────────

    fn sample_graph() -> GraphData {
        GraphData::new(
            vec![
                GraphLine {
                    raw: "◉  abc".into(),
                    change_id: Some("abc".into()),
                    glyph_prefix: String::new(),
                },
                GraphLine {
                    raw: "◉  def".into(),
                    change_id: Some("def".into()),
                    glyph_prefix: String::new(),
                },
                GraphLine {
                    raw: "◉  ghi".into(),
                    change_id: Some("ghi".into()),
                    glyph_prefix: String::new(),
                },
            ],
            HashMap::from([
                (
                    "abc".into(),
                    ChangeDetail {
                        commit_id: "a1".into(),
                        author: "a".into(),
                        email: "a@b".into(),
                        timestamp: "1m".into(),
                        description: "desc1".into(),
                        bookmarks: vec![],
                        is_empty: false,
                        conflict_count: 0,
                        files: vec![],
                        parents: vec![],
                    },
                ),
                (
                    "def".into(),
                    ChangeDetail {
                        commit_id: "d1".into(),
                        author: "b".into(),
                        email: "b@c".into(),
                        timestamp: "2m".into(),
                        description: "desc2".into(),
                        bookmarks: vec![],
                        is_empty: false,
                        conflict_count: 0,
                        files: vec![],
                        parents: vec![],
                    },
                ),
                (
                    "ghi".into(),
                    ChangeDetail {
                        commit_id: "g1".into(),
                        author: "c".into(),
                        email: "c@d".into(),
                        timestamp: "3m".into(),
                        description: "desc3".into(),
                        bookmarks: vec![],
                        is_empty: false,
                        conflict_count: 0,
                        files: vec![],
                        parents: vec![],
                    },
                ),
            ]),
            Some(0),
            String::new(),
        )
    }

    fn default_layout() -> LayoutRects {
        LayoutRects {
            // graph: columns 0-39, rows 0-19 (outer includes border at 0)
            graph_outer: Rect::new(0, 0, 40, 20),
            graph_inner: Rect::new(1, 1, 38, 18),
            // detail: columns 40-79, rows 0-19
            detail_outer: Rect::new(40, 0, 40, 20),
            detail_inner: Rect::new(41, 1, 38, 18),
            modal_area: None,
            graph_scroll_offset: 0,
        }
    }

    fn make_event(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn make_state_with_layout(layout: LayoutRects) -> AppState {
        let mut state = AppState::new(sample_graph(), None);
        state.layout = layout;
        state
    }

    // ── click tests ──────────────────────────────────────────────────────────

    #[test]
    fn click_graph_inner_emits_click_node() {
        let state = make_state_with_layout(default_layout());
        // graph_inner starts at (1,1); click at (5,1) => viewport_row=0 => line_index=0
        let event = make_event(MouseEventKind::Down(MouseButton::Left), 5, 1);
        let action = map_mouse_event(event, &state);
        assert_eq!(action, Some(Action::ClickGraphNode { line_index: 0 }));
    }

    #[test]
    fn click_graph_with_scroll_offset() {
        let mut layout = default_layout();
        layout.graph_scroll_offset = 10;
        let state = make_state_with_layout(layout);
        // viewport_row = 1 - 1 = 0; absolute = 10 + 0 = 10
        let event = make_event(MouseEventKind::Down(MouseButton::Left), 5, 1);
        let action = map_mouse_event(event, &state);
        assert_eq!(action, Some(Action::ClickGraphNode { line_index: 10 }));
    }

    #[test]
    fn click_detail_when_graph_focused_selects_item() {
        let state = make_state_with_layout(default_layout());
        // focus defaults to Graph; click in detail_inner — focus + select in one click
        let event = make_event(MouseEventKind::Down(MouseButton::Left), 45, 5);
        let action = map_mouse_event(event, &state);
        // detail_inner.y = 1, click row 5 => offset 4
        assert_eq!(action, Some(Action::ClickDetailItem { index: 4 }));
    }

    #[test]
    fn click_graph_when_detail_focused_selects_node() {
        let mut state = make_state_with_layout(default_layout());
        state.focus = PanelFocus::Detail;
        // click inside graph_inner — focus + select in one click
        let event = make_event(MouseEventKind::Down(MouseButton::Left), 5, 5);
        let action = map_mouse_event(event, &state);
        // graph_inner.y = 1, click row 5 => viewport row 4, absolute = 0 + 4 = 4
        assert_eq!(action, Some(Action::ClickGraphNode { line_index: 4 }));
    }

    #[test]
    fn click_graph_border_only_focuses() {
        let mut state = make_state_with_layout(default_layout());
        state.focus = PanelFocus::Detail;
        // Click at (0, 0) — graph_outer but not graph_inner (border)
        let event = make_event(MouseEventKind::Down(MouseButton::Left), 0, 0);
        let action = map_mouse_event(event, &state);
        assert_eq!(action, Some(Action::ClickFocusGraph));
    }

    #[test]
    fn click_detail_border_only_focuses() {
        let state = make_state_with_layout(default_layout());
        // detail_outer starts at x=40. Click at (40, 0) — border, not inner
        let event = make_event(MouseEventKind::Down(MouseButton::Left), 40, 0);
        let action = map_mouse_event(event, &state);
        assert_eq!(action, Some(Action::ClickFocusDetail));
    }

    #[test]
    fn click_detail_inner_when_focused_selects_item() {
        let mut state = make_state_with_layout(default_layout());
        state.focus = PanelFocus::Detail;
        // detail_inner.y = 1; click at row=3 => row_offset = 3-1 = 2
        let event = make_event(MouseEventKind::Down(MouseButton::Left), 45, 3);
        let action = map_mouse_event(event, &state);
        assert_eq!(action, Some(Action::ClickDetailItem { index: 2 }));
    }

    // ── scroll tests ─────────────────────────────────────────────────────────

    #[test]
    fn scroll_up_in_graph() {
        let state = make_state_with_layout(default_layout());
        // col=5, row=5 is inside graph_outer
        let event = make_event(MouseEventKind::ScrollUp, 5, 5);
        let action = map_mouse_event(event, &state);
        assert_eq!(
            action,
            Some(Action::ScrollUp {
                count: 3,
                panel: PanelFocus::Graph
            })
        );
    }

    #[test]
    fn scroll_down_in_detail() {
        let state = make_state_with_layout(default_layout());
        // col=45, row=5 is inside detail_outer
        let event = make_event(MouseEventKind::ScrollDown, 45, 5);
        let action = map_mouse_event(event, &state);
        assert_eq!(
            action,
            Some(Action::ScrollDown {
                count: 3,
                panel: PanelFocus::Detail
            })
        );
    }

    #[test]
    fn scroll_in_modal_emits_modal_move_down() {
        let mut state = make_state_with_layout(default_layout());
        state.modal = Some(Modal::Help {
            context: HelpContext::Graph,
            scroll: 0,
        });
        state.layout.modal_area = Some(Rect::new(0, 0, 40, 20));
        // Scroll inside modal area
        let event = make_event(MouseEventKind::ScrollDown, 5, 5);
        let action = map_mouse_event(event, &state);
        assert_eq!(action, Some(Action::ModalMoveDown));
    }

    #[test]
    fn scroll_in_modal_emits_modal_move_up() {
        let mut state = make_state_with_layout(default_layout());
        state.modal = Some(Modal::Help {
            context: HelpContext::Graph,
            scroll: 0,
        });
        state.layout.modal_area = Some(Rect::new(0, 0, 40, 20));
        // Scroll inside modal area
        let event = make_event(MouseEventKind::ScrollUp, 5, 5);
        let action = map_mouse_event(event, &state);
        assert_eq!(action, Some(Action::ModalMoveUp));
    }

    #[test]
    fn scroll_outside_modal_ignored() {
        let mut state = make_state_with_layout(default_layout());
        state.modal = Some(Modal::Help {
            context: HelpContext::Graph,
            scroll: 0,
        });
        state.layout.modal_area = Some(Rect::new(10, 5, 20, 10));
        // Scroll outside modal area
        let event = make_event(MouseEventKind::ScrollDown, 5, 3);
        let action = map_mouse_event(event, &state);
        assert_eq!(action, None);
    }

    // ── modal click tests ────────────────────────────────────────────────────

    #[test]
    fn click_outside_modal_dismisses() {
        let mut layout = default_layout();
        // modal occupies columns 10-29, rows 5-14
        layout.modal_area = Some(Rect::new(10, 5, 20, 10));
        let mut state = make_state_with_layout(layout);
        state.modal = Some(Modal::Help {
            context: HelpContext::Graph,
            scroll: 0,
        });
        // click at (5,5) — outside the modal rect
        let event = make_event(MouseEventKind::Down(MouseButton::Left), 5, 5);
        let action = map_mouse_event(event, &state);
        assert_eq!(action, Some(Action::ModalDismiss));
    }

    #[test]
    fn click_inside_modal_returns_none() {
        let mut layout = default_layout();
        layout.modal_area = Some(Rect::new(10, 5, 20, 10));
        let mut state = make_state_with_layout(layout);
        state.modal = Some(Modal::Help {
            context: HelpContext::Graph,
            scroll: 0,
        });
        // click at (15,7) — inside the modal rect
        let event = make_event(MouseEventKind::Down(MouseButton::Left), 15, 7);
        let action = map_mouse_event(event, &state);
        assert_eq!(action, None);
    }

    #[test]
    fn click_outside_describe_modal_ignored() {
        let mut layout = default_layout();
        layout.modal_area = Some(Rect::new(10, 5, 20, 10));
        let mut state = make_state_with_layout(layout);
        // Describe modal is not click-dismissable
        use tui_textarea::TextArea;
        state.modal = Some(Modal::Describe {
            change_id: "abc".into(),
            editor: Box::new(TextArea::default()),
        });
        // click outside the modal rect — should NOT dismiss
        let event = make_event(MouseEventKind::Down(MouseButton::Left), 5, 5);
        let action = map_mouse_event(event, &state);
        assert_eq!(action, None);
    }

    #[test]
    fn click_outside_bookmark_input_ignored() {
        let mut layout = default_layout();
        layout.modal_area = Some(Rect::new(10, 5, 20, 10));
        let mut state = make_state_with_layout(layout);
        state.modal = Some(Modal::BookmarkInput {
            change_id: "abc".into(),
            input: String::new(),
            completions: vec![],
            cursor: 0,
        });
        // click outside — should NOT dismiss
        let event = make_event(MouseEventKind::Down(MouseButton::Left), 5, 5);
        let action = map_mouse_event(event, &state);
        assert_eq!(action, None);
    }

    // ── ignored event tests ──────────────────────────────────────────────────

    #[test]
    fn mouse_drag_ignored() {
        let state = make_state_with_layout(default_layout());
        let event = make_event(MouseEventKind::Drag(MouseButton::Left), 5, 5);
        let action = map_mouse_event(event, &state);
        assert_eq!(action, None);
    }

    #[test]
    fn mouse_move_ignored() {
        let state = make_state_with_layout(default_layout());
        let event = make_event(MouseEventKind::Moved, 5, 5);
        let action = map_mouse_event(event, &state);
        assert_eq!(action, None);
    }

    // ── border / boundary tests ──────────────────────────────────────────────

    #[test]
    fn click_on_graph_border_focuses_graph() {
        let mut state = make_state_with_layout(default_layout());
        state.focus = PanelFocus::Detail;
        // (0,0) is on the graph_outer border, not inside graph_inner
        let event = make_event(MouseEventKind::Down(MouseButton::Left), 0, 0);
        let action = map_mouse_event(event, &state);
        assert_eq!(action, Some(Action::ClickFocusGraph));
    }

    // ── mode guard tests ─────────────────────────────────────────────────────

    #[test]
    fn click_during_hunk_picker_ignored() {
        let mut state = make_state_with_layout(default_layout());
        state.detail_mode = DetailMode::HunkPicker;
        let event = make_event(MouseEventKind::Down(MouseButton::Left), 5, 5);
        let action = map_mouse_event(event, &state);
        assert_eq!(action, None);
    }

    #[test]
    fn click_during_diff_view_ignored() {
        let mut state = make_state_with_layout(default_layout());
        state.detail_mode = DetailMode::DiffView;
        let event = make_event(MouseEventKind::Down(MouseButton::Left), 5, 5);
        let action = map_mouse_event(event, &state);
        assert_eq!(action, None);
    }

    #[test]
    fn click_during_picking_mode_ignored() {
        use crate::action::RebaseMode;
        use crate::app::{PickingMode, TargetPick};
        use std::collections::HashSet;
        let mut state = make_state_with_layout(default_layout());
        state.target_pick = Some(TargetPick {
            source: "abc".into(),
            mode: RebaseMode::Single,
            excluded: HashSet::new(),
            picking: PickingMode::Browsing,
            original_change_id: "abc".into(),
            descendant_count: 0,
        });
        let event = make_event(MouseEventKind::Down(MouseButton::Left), 5, 5);
        let action = map_mouse_event(event, &state);
        assert_eq!(action, None);
    }

    // ── hit_test unit tests ──────────────────────────────────────────────────

    #[test]
    fn hit_test_basic() {
        let rect = Rect::new(10, 10, 20, 10); // x:10-29, y:10-19

        // inside
        assert!(hit_test(rect, 10, 10)); // top-left corner
        assert!(hit_test(rect, 29, 19)); // bottom-right corner (inclusive)
        assert!(hit_test(rect, 20, 15)); // center

        // outside — just past each edge
        assert!(!hit_test(rect, 9, 10)); // left of rect
        assert!(!hit_test(rect, 30, 10)); // right of rect (x + width)
        assert!(!hit_test(rect, 10, 9)); // above rect
        assert!(!hit_test(rect, 10, 20)); // below rect (y + height)
    }

    #[test]
    fn right_click_ignored() {
        let state = make_state_with_layout(default_layout());
        let event = make_event(MouseEventKind::Down(MouseButton::Right), 5, 5);
        assert_eq!(map_mouse_event(event, &state), None);
    }

    #[test]
    fn middle_click_ignored() {
        let state = make_state_with_layout(default_layout());
        let event = make_event(MouseEventKind::Down(MouseButton::Middle), 5, 5);
        assert_eq!(map_mouse_event(event, &state), None);
    }
}
