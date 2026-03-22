use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{Action, DetailMode, Modal, PanelFocus, PickingMode};

#[expect(clippy::too_many_lines)]
pub fn map_event(event: KeyEvent, focus: PanelFocus, detail_mode: DetailMode) -> Option<Action> {
    // Global keys
    match (event.code, event.modifiers) {
        (KeyCode::Char('q'), KeyModifiers::NONE) if detail_mode != DetailMode::HunkPicker => {
            return Some(Action::Quit);
        }
        // Ctrl-C always works: quit normally, or cancel hunk picker
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            return Some(if detail_mode == DetailMode::HunkPicker {
                Action::HunkCancel
            } else {
                Action::Quit
            });
        }
        (KeyCode::Tab, _) if detail_mode != DetailMode::HunkPicker => {
            return Some(Action::TabFocus);
        }
        (KeyCode::BackTab, _) if detail_mode != DetailMode::HunkPicker => {
            return Some(Action::BackTabFocus);
        }
        (KeyCode::Char('R'), m)
            if !m.contains(KeyModifiers::CONTROL) && detail_mode != DetailMode::HunkPicker =>
        {
            return Some(Action::Refresh);
        }
        (KeyCode::Char('@'), _) if detail_mode != DetailMode::HunkPicker => {
            return Some(Action::JumpToWorkingCopy);
        }
        (KeyCode::Char('O'), _) if detail_mode != DetailMode::HunkPicker => {
            return Some(Action::ToggleOpLog);
        }
        (KeyCode::Char('b'), KeyModifiers::NONE) if detail_mode != DetailMode::HunkPicker => {
            return Some(Action::OpenBookmarks);
        }
        (KeyCode::Char('/'), _) if detail_mode != DetailMode::HunkPicker => {
            return Some(Action::OpenOmnibar);
        }
        (KeyCode::Char('?'), _) => return Some(Action::OpenHelp), // help always available
        _ => {}
    }

    match focus {
        PanelFocus::Graph => match (event.code, event.modifiers) {
            (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => Some(Action::MoveDown),
            (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => Some(Action::MoveUp),
            (KeyCode::Char('g'), KeyModifiers::NONE) => Some(Action::JumpToTop),
            (KeyCode::Char('G'), _) => Some(Action::JumpToBottom),
            (KeyCode::Char('d'), KeyModifiers::NONE) => Some(Action::Abandon),
            (KeyCode::Char('n'), KeyModifiers::NONE) => Some(Action::NewChange),
            (KeyCode::Char('e'), KeyModifiers::CONTROL) => Some(Action::EditChange),
            (KeyCode::Char('e'), KeyModifiers::NONE) => Some(Action::OpenDescribe),
            (KeyCode::Char('s'), KeyModifiers::NONE) => Some(Action::Split),
            (KeyCode::Char('S'), _) => Some(Action::SquashPartial),
            (KeyCode::Char('u'), KeyModifiers::NONE) => Some(Action::Undo),
            (KeyCode::Char('r'), KeyModifiers::NONE) => Some(Action::RebaseSingle),
            (KeyCode::Char('r'), KeyModifiers::CONTROL) => Some(Action::RebaseWithDescendants),
            (KeyCode::Char('r'), m) if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT => {
                Some(Action::Redo)
            }
            // Some terminals report Ctrl-Shift-R as Ctrl+uppercase-R
            (KeyCode::Char('R'), KeyModifiers::CONTROL) => Some(Action::Redo),
            (KeyCode::Char('B'), _) => Some(Action::OpenBookmarkSet),
            (KeyCode::Char('P'), _) => Some(Action::GitPush),
            (KeyCode::Char('f'), KeyModifiers::NONE) => Some(Action::GitFetch),
            _ => None,
        },
        PanelFocus::Detail => match detail_mode {
            DetailMode::FileList => match (event.code, event.modifiers) {
                (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => {
                    Some(Action::DetailMoveDown)
                }
                (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => {
                    Some(Action::DetailMoveUp)
                }
                (KeyCode::Enter, _) => Some(Action::DetailEnter),
                (KeyCode::Esc, _) => Some(Action::DetailBack),
                _ => None,
            },
            DetailMode::DiffView => match (event.code, event.modifiers) {
                (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => {
                    Some(Action::DiffScrollDown)
                }
                (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => {
                    Some(Action::DiffScrollUp)
                }
                (KeyCode::Char('n'), KeyModifiers::NONE) => Some(Action::DiffNextHunk),
                (KeyCode::Char('N'), _) => Some(Action::DiffPrevHunk),
                (KeyCode::Esc, _) => Some(Action::DetailBack),
                _ => None,
            },
            // Stub — full implementation in Task 6
            DetailMode::ConflictView => match (event.code, event.modifiers) {
                (KeyCode::Esc, _) => Some(Action::DetailBack),
                _ => None,
            },
            DetailMode::HunkPicker => match (event.code, event.modifiers) {
                (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => {
                    Some(Action::DetailMoveDown)
                }
                (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => {
                    Some(Action::DetailMoveUp)
                }
                (KeyCode::Char('J'), _) => Some(Action::HunkNextFile),
                (KeyCode::Char('K'), _) => Some(Action::HunkPrevFile),
                (KeyCode::Char(' '), _) => Some(Action::HunkToggle),
                (KeyCode::Char('a'), KeyModifiers::NONE) => Some(Action::HunkSelectAll),
                (KeyCode::Char('A'), _) => Some(Action::HunkDeselectAll),
                (KeyCode::Enter, _) => Some(Action::HunkConfirm),
                (KeyCode::Esc, _) => Some(Action::HunkCancel),
                _ => None,
            },
        },
    }
}

/// Map a key event when a modal is active. Returns `None` to swallow the key.
pub fn map_modal_event(event: KeyEvent, modal: &Modal) -> Option<Action> {
    // Describe modal has its own key handling (intercepts Esc differently)
    if let Modal::Describe { .. } = modal {
        return match (event.code, event.modifiers) {
            (KeyCode::Char('s'), KeyModifiers::CONTROL)
            | (KeyCode::Enter, KeyModifiers::CONTROL | KeyModifiers::ALT) => {
                Some(Action::DescribeSave)
            }
            (KeyCode::Esc, _) => Some(Action::ModalDismiss),
            (KeyCode::Char('E'), KeyModifiers::SHIFT) => Some(Action::DescribeEscalateEditor),
            _ => None, // tui-textarea handles other keys
        };
    }

    // BookmarkInput modal has its own key handling (intercepts Enter/Backspace/Char)
    if let Modal::BookmarkInput { .. } = modal {
        return match event.code {
            KeyCode::Esc => Some(Action::ModalDismiss),
            KeyCode::Enter => Some(Action::BookmarkInputConfirm),
            KeyCode::Backspace => Some(Action::BookmarkInputBackspace),
            KeyCode::Char(c)
                if event.modifiers == KeyModifiers::NONE
                    || event.modifiers == KeyModifiers::SHIFT =>
            {
                Some(Action::BookmarkInputChar(c))
            }
            _ => None,
        };
    }

    // Common keys for ALL modals
    match event.code {
        KeyCode::Esc => return Some(Action::ModalDismiss),
        KeyCode::Enter => return Some(Action::ModalEnter),
        KeyCode::Up => return Some(Action::ModalMoveUp),
        KeyCode::Down => return Some(Action::ModalMoveDown),
        _ => {}
    }

    if let Modal::Omnibar { completions, .. } = modal
        && event.code == KeyCode::Tab
        && !completions.is_empty()
    {
        return Some(Action::OmnibarAcceptCompletion);
    }

    let is_omnibar = matches!(modal, Modal::Omnibar { .. });

    if is_omnibar {
        match event.code {
            KeyCode::Backspace => Some(Action::OmnibarBackspace),
            KeyCode::Char('n') if event.modifiers == KeyModifiers::CONTROL => {
                Some(Action::ModalMoveDown)
            }
            KeyCode::Char('p') if event.modifiers == KeyModifiers::CONTROL => {
                Some(Action::ModalMoveUp)
            }
            KeyCode::Char(c)
                if event.modifiers == KeyModifiers::NONE
                    || event.modifiers == KeyModifiers::SHIFT =>
            {
                Some(Action::OmnibarInput(c))
            }
            _ => None,
        }
    } else {
        match (event.code, event.modifiers) {
            (KeyCode::Char('q'), KeyModifiers::NONE) => Some(Action::ModalDismiss),
            (KeyCode::Char('j'), KeyModifiers::NONE) => Some(Action::ModalMoveDown),
            (KeyCode::Char('k'), KeyModifiers::NONE) => Some(Action::ModalMoveUp),
            (KeyCode::Char('d'), KeyModifiers::NONE)
                if matches!(modal, Modal::BookmarkPicker { .. }) =>
            {
                Some(Action::BookmarkDelete)
            }
            (KeyCode::Char('O'), _) if matches!(modal, Modal::OpLog { .. }) => {
                Some(Action::ModalDismiss)
            }
            (KeyCode::Char('?'), _) if matches!(modal, Modal::Help { .. }) => {
                Some(Action::ModalDismiss)
            }
            _ => None,
        }
    }
}

pub fn map_picking_event(event: KeyEvent, picking: &PickingMode) -> Option<Action> {
    match picking {
        PickingMode::Browsing => match (event.code, event.modifiers) {
            (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => Some(Action::MoveDown),
            (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => Some(Action::MoveUp),
            (KeyCode::Enter, _) => Some(Action::PickConfirm),
            (KeyCode::Esc, _) => Some(Action::PickCancel),
            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                Some(Action::PickFilterChar(c))
            }
            _ => None,
        },
        PickingMode::Filtering { .. } => match event.code {
            KeyCode::Char('j') if event.modifiers == KeyModifiers::CONTROL => {
                Some(Action::MoveDown)
            }
            KeyCode::Char('k') if event.modifiers == KeyModifiers::CONTROL => Some(Action::MoveUp),
            KeyCode::Down => Some(Action::MoveDown),
            KeyCode::Up => Some(Action::MoveUp),
            KeyCode::Enter => Some(Action::PickConfirm),
            KeyCode::Esc => Some(Action::PickCancel),
            KeyCode::Backspace => Some(Action::PickFilterBackspace),
            KeyCode::Char(c)
                if event.modifiers == KeyModifiers::NONE
                    || event.modifiers == KeyModifiers::SHIFT =>
            {
                Some(Action::PickFilterChar(c))
            }
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn key_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    // Convenience wrappers for common focus/mode combinations
    fn map_graph(event: KeyEvent) -> Option<Action> {
        map_event(event, PanelFocus::Graph, DetailMode::FileList)
    }

    fn map_file_list(event: KeyEvent) -> Option<Action> {
        map_event(event, PanelFocus::Detail, DetailMode::FileList)
    }

    fn map_diff_view(event: KeyEvent) -> Option<Action> {
        map_event(event, PanelFocus::Detail, DetailMode::DiffView)
    }

    #[test]
    fn global_quit_keys_work_in_any_focus() {
        let q = key(KeyCode::Char('q'));
        let ctrl_c = key_mod(KeyCode::Char('c'), KeyModifiers::CONTROL);
        for event in [q, ctrl_c] {
            assert_eq!(map_graph(event), Some(Action::Quit));
            assert_eq!(map_file_list(event), Some(Action::Quit));
            assert_eq!(map_diff_view(event), Some(Action::Quit));
        }
    }

    #[test]
    fn tab_cycles_focus() {
        assert_eq!(map_graph(key(KeyCode::Tab)), Some(Action::TabFocus));
        assert_eq!(map_file_list(key(KeyCode::Tab)), Some(Action::TabFocus));
        assert_eq!(map_diff_view(key(KeyCode::Tab)), Some(Action::TabFocus));
        assert_eq!(map_graph(key(KeyCode::BackTab)), Some(Action::BackTabFocus));
    }

    #[test]
    fn refresh_and_at_are_global() {
        let r = key(KeyCode::Char('R'));
        let at = key(KeyCode::Char('@'));
        assert_eq!(map_graph(r), Some(Action::Refresh));
        assert_eq!(map_file_list(r), Some(Action::Refresh));
        assert_eq!(map_diff_view(r), Some(Action::Refresh));
        assert_eq!(map_graph(at), Some(Action::JumpToWorkingCopy));
        assert_eq!(map_file_list(at), Some(Action::JumpToWorkingCopy));
        assert_eq!(map_diff_view(at), Some(Action::JumpToWorkingCopy));
    }

    #[test]
    fn graph_navigation() {
        assert_eq!(map_graph(key(KeyCode::Char('j'))), Some(Action::MoveDown));
        assert_eq!(map_graph(key(KeyCode::Down)), Some(Action::MoveDown));
        assert_eq!(map_graph(key(KeyCode::Char('k'))), Some(Action::MoveUp));
        assert_eq!(map_graph(key(KeyCode::Up)), Some(Action::MoveUp));
        assert_eq!(map_graph(key(KeyCode::Char('g'))), Some(Action::JumpToTop));
        assert_eq!(
            map_graph(key(KeyCode::Char('G'))),
            Some(Action::JumpToBottom)
        );
    }

    #[test]
    fn detail_file_list_navigation() {
        assert_eq!(
            map_file_list(key(KeyCode::Char('j'))),
            Some(Action::DetailMoveDown)
        );
        assert_eq!(
            map_file_list(key(KeyCode::Down)),
            Some(Action::DetailMoveDown)
        );
        assert_eq!(
            map_file_list(key(KeyCode::Char('k'))),
            Some(Action::DetailMoveUp)
        );
        assert_eq!(map_file_list(key(KeyCode::Up)), Some(Action::DetailMoveUp));
        assert_eq!(
            map_file_list(key(KeyCode::Enter)),
            Some(Action::DetailEnter)
        );
        assert_eq!(map_file_list(key(KeyCode::Esc)), Some(Action::DetailBack));
    }

    #[test]
    fn detail_diff_view_navigation() {
        assert_eq!(
            map_diff_view(key(KeyCode::Char('j'))),
            Some(Action::DiffScrollDown)
        );
        assert_eq!(
            map_diff_view(key(KeyCode::Down)),
            Some(Action::DiffScrollDown)
        );
        assert_eq!(
            map_diff_view(key(KeyCode::Char('k'))),
            Some(Action::DiffScrollUp)
        );
        assert_eq!(map_diff_view(key(KeyCode::Up)), Some(Action::DiffScrollUp));
        assert_eq!(
            map_diff_view(key(KeyCode::Char('n'))),
            Some(Action::DiffNextHunk)
        );
        assert_eq!(
            map_diff_view(key(KeyCode::Char('N'))),
            Some(Action::DiffPrevHunk)
        );
        assert_eq!(map_diff_view(key(KeyCode::Esc)), Some(Action::DetailBack));
    }

    #[test]
    fn same_key_different_action_by_context() {
        let j = key(KeyCode::Char('j'));
        assert_eq!(map_graph(j), Some(Action::MoveDown));
        assert_eq!(map_file_list(j), Some(Action::DetailMoveDown));
        assert_eq!(map_diff_view(j), Some(Action::DiffScrollDown));
    }

    #[test]
    fn unmapped_key_returns_none() {
        assert_eq!(map_graph(key(KeyCode::Char('x'))), None);
        assert_eq!(map_file_list(key(KeyCode::Char('x'))), None);
        assert_eq!(map_diff_view(key(KeyCode::Char('x'))), None);
    }

    // --- Modal input tests ---

    #[test]
    fn modal_trigger_keys() {
        assert_eq!(
            map_graph(key(KeyCode::Char('O'))),
            Some(Action::ToggleOpLog)
        );
        assert_eq!(
            map_graph(key(KeyCode::Char('b'))),
            Some(Action::OpenBookmarks)
        );
        assert_eq!(
            map_graph(key(KeyCode::Char('/'))),
            Some(Action::OpenOmnibar)
        );
        assert_eq!(map_graph(key(KeyCode::Char('?'))), Some(Action::OpenHelp));
    }

    #[test]
    fn modal_esc_dismisses() {
        let modal = Modal::Help {
            context: crate::app::HelpContext::Graph,
            scroll: 0,
        };
        assert_eq!(
            map_modal_event(key(KeyCode::Esc), &modal),
            Some(Action::ModalDismiss)
        );
    }

    #[test]
    fn modal_q_dismisses_non_omnibar() {
        let modal = Modal::Help {
            context: crate::app::HelpContext::Graph,
            scroll: 0,
        };
        assert_eq!(
            map_modal_event(key(KeyCode::Char('q')), &modal),
            Some(Action::ModalDismiss)
        );
    }

    #[test]
    fn omnibar_q_is_text_input() {
        let modal = Modal::Omnibar {
            query: String::new(),
            matches: vec![],
            cursor: 0,
            completions: vec![],
            completion_cursor: 0,
        };
        assert_eq!(
            map_modal_event(key(KeyCode::Char('q')), &modal),
            Some(Action::OmnibarInput('q'))
        );
    }

    #[test]
    fn modal_jk_navigation_non_fuzzy() {
        let modal = Modal::OpLog {
            entries: vec![],
            cursor: 0,
            scroll: 0,
        };
        assert_eq!(
            map_modal_event(key(KeyCode::Char('j')), &modal),
            Some(Action::ModalMoveDown)
        );
        assert_eq!(
            map_modal_event(key(KeyCode::Char('k')), &modal),
            Some(Action::ModalMoveUp)
        );
    }

    #[test]
    fn omnibar_ctrl_n_p_navigation() {
        let modal = Modal::Omnibar {
            query: String::new(),
            matches: vec![],
            cursor: 0,
            completions: vec![],
            completion_cursor: 0,
        };
        assert_eq!(
            map_modal_event(key_mod(KeyCode::Char('n'), KeyModifiers::CONTROL), &modal),
            Some(Action::ModalMoveDown)
        );
        assert_eq!(
            map_modal_event(key_mod(KeyCode::Char('p'), KeyModifiers::CONTROL), &modal),
            Some(Action::ModalMoveUp)
        );
    }

    #[test]
    fn omnibar_backspace() {
        let modal = Modal::Omnibar {
            query: String::new(),
            matches: vec![],
            cursor: 0,
            completions: vec![],
            completion_cursor: 0,
        };
        assert_eq!(
            map_modal_event(key(KeyCode::Backspace), &modal),
            Some(Action::OmnibarBackspace)
        );
    }

    #[test]
    fn oplog_toggle_key_dismisses() {
        let modal = Modal::OpLog {
            entries: vec![],
            cursor: 0,
            scroll: 0,
        };
        assert_eq!(
            map_modal_event(key(KeyCode::Char('O')), &modal),
            Some(Action::ModalDismiss)
        );
    }

    #[test]
    fn help_question_mark_dismisses() {
        let modal = Modal::Help {
            context: crate::app::HelpContext::Graph,
            scroll: 0,
        };
        assert_eq!(
            map_modal_event(key(KeyCode::Char('?')), &modal),
            Some(Action::ModalDismiss)
        );
    }

    #[test]
    fn bookmark_input_key_routing() {
        let modal = Modal::BookmarkInput {
            change_id: "abc".into(),
            input: "foo".into(),
            completions: vec![],
            cursor: 0,
        };
        // Enter confirms
        assert_eq!(
            map_modal_event(key(KeyCode::Enter), &modal),
            Some(Action::BookmarkInputConfirm)
        );
        // Esc dismisses
        assert_eq!(
            map_modal_event(key(KeyCode::Esc), &modal),
            Some(Action::ModalDismiss)
        );
        // Backspace removes last char
        assert_eq!(
            map_modal_event(key(KeyCode::Backspace), &modal),
            Some(Action::BookmarkInputBackspace)
        );
        // Regular char appends
        assert_eq!(
            map_modal_event(key(KeyCode::Char('x')), &modal),
            Some(Action::BookmarkInputChar('x'))
        );
        // Shift char appends (e.g. uppercase)
        assert_eq!(
            map_modal_event(key_mod(KeyCode::Char('X'), KeyModifiers::SHIFT), &modal),
            Some(Action::BookmarkInputChar('X'))
        );
        // Ctrl-char is ignored
        assert_eq!(
            map_modal_event(key_mod(KeyCode::Char('c'), KeyModifiers::CONTROL), &modal),
            None
        );
    }

    #[test]
    fn bookmark_picker_d_deletes() {
        let modal = Modal::BookmarkPicker {
            bookmarks: vec![("main".into(), "abc".into())],
            cursor: 0,
        };
        assert_eq!(
            map_modal_event(key(KeyCode::Char('d')), &modal),
            Some(Action::BookmarkDelete)
        );
    }

    #[test]
    fn graph_mutation_keys() {
        // Abandon
        assert_eq!(map_graph(key(KeyCode::Char('d'))), Some(Action::Abandon));
        // NewChange
        assert_eq!(map_graph(key(KeyCode::Char('n'))), Some(Action::NewChange));
        // OpenDescribe (plain e)
        assert_eq!(
            map_graph(key(KeyCode::Char('e'))),
            Some(Action::OpenDescribe)
        );
        // EditChange (Ctrl-E)
        assert_eq!(
            map_graph(key_mod(KeyCode::Char('e'), KeyModifiers::CONTROL)),
            Some(Action::EditChange)
        );
        // Split (lowercase s)
        assert_eq!(map_graph(key(KeyCode::Char('s'))), Some(Action::Split));
        // SquashPartial (capital S)
        assert_eq!(
            map_graph(key_mod(KeyCode::Char('S'), KeyModifiers::SHIFT)),
            Some(Action::SquashPartial)
        );
        // Undo
        assert_eq!(map_graph(key(KeyCode::Char('u'))), Some(Action::Undo));
        // RebaseWithDescendants (Ctrl-R; Redo moved to Ctrl-Shift-R)
        assert_eq!(
            map_graph(key_mod(KeyCode::Char('r'), KeyModifiers::CONTROL)),
            Some(Action::RebaseWithDescendants)
        );
        // OpenBookmarkSet (capital B)
        assert_eq!(
            map_graph(key_mod(KeyCode::Char('B'), KeyModifiers::SHIFT)),
            Some(Action::OpenBookmarkSet)
        );
        // GitPush (capital P)
        assert_eq!(
            map_graph(key_mod(KeyCode::Char('P'), KeyModifiers::SHIFT)),
            Some(Action::GitPush)
        );
        // GitFetch
        assert_eq!(map_graph(key(KeyCode::Char('f'))), Some(Action::GitFetch));
    }

    #[test]
    fn mutation_keys_not_active_in_detail_context() {
        // 'd' in detail file list should not map to Abandon
        assert_eq!(map_file_list(key(KeyCode::Char('d'))), None);
        // 'n' in diff view should not map to NewChange (it maps to DiffNextHunk)
        assert_eq!(
            map_diff_view(key(KeyCode::Char('n'))),
            Some(Action::DiffNextHunk)
        );
        // 'f' in detail should not map to GitFetch
        assert_eq!(map_file_list(key(KeyCode::Char('f'))), None);
    }

    #[test]
    fn ctrl_e_edit_before_plain_e_describe() {
        // Both are distinct tuples; ensure Ctrl-E maps to EditChange, not OpenDescribe
        assert_eq!(
            map_graph(key_mod(KeyCode::Char('e'), KeyModifiers::CONTROL)),
            Some(Action::EditChange)
        );
        assert_ne!(
            map_graph(key_mod(KeyCode::Char('e'), KeyModifiers::CONTROL)),
            Some(Action::OpenDescribe)
        );
    }

    #[test]
    fn rebase_keys_in_graph_context() {
        assert_eq!(
            map_graph(key(KeyCode::Char('r'))),
            Some(Action::RebaseSingle)
        );
        assert_eq!(
            map_graph(key_mod(KeyCode::Char('r'), KeyModifiers::CONTROL)),
            Some(Action::RebaseWithDescendants)
        );
    }

    #[test]
    fn redo_moved_to_ctrl_shift_r() {
        let redo_key = key_mod(
            KeyCode::Char('r'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert_eq!(map_graph(redo_key), Some(Action::Redo));
        // Verify Ctrl-R alone is NOT Redo anymore
        assert_ne!(
            map_graph(key_mod(KeyCode::Char('r'), KeyModifiers::CONTROL)),
            Some(Action::Redo)
        );
    }

    #[test]
    fn picking_mode_browsing_key_routing() {
        let browsing = PickingMode::Browsing;
        assert_eq!(
            map_picking_event(key(KeyCode::Char('j')), &browsing),
            Some(Action::MoveDown)
        );
        assert_eq!(
            map_picking_event(key(KeyCode::Char('k')), &browsing),
            Some(Action::MoveUp)
        );
        assert_eq!(
            map_picking_event(key(KeyCode::Enter), &browsing),
            Some(Action::PickConfirm)
        );
        assert_eq!(
            map_picking_event(key(KeyCode::Esc), &browsing),
            Some(Action::PickCancel)
        );
        // Any char starts filtering
        assert_eq!(
            map_picking_event(key(KeyCode::Char('a')), &browsing),
            Some(Action::PickFilterChar('a'))
        );
    }

    #[test]
    fn picking_mode_filtering_key_routing() {
        let filtering = PickingMode::Filtering {
            query: "abc".into(),
        };
        // Ctrl-J/K for navigation in filter mode
        assert_eq!(
            map_picking_event(
                key_mod(KeyCode::Char('j'), KeyModifiers::CONTROL),
                &filtering
            ),
            Some(Action::MoveDown)
        );
        assert_eq!(
            map_picking_event(key(KeyCode::Down), &filtering),
            Some(Action::MoveDown)
        );
        assert_eq!(
            map_picking_event(key(KeyCode::Backspace), &filtering),
            Some(Action::PickFilterBackspace)
        );
        // Regular chars extend filter
        assert_eq!(
            map_picking_event(key(KeyCode::Char('x')), &filtering),
            Some(Action::PickFilterChar('x'))
        );
    }

    #[test]
    fn picking_mode_blocks_global_keys() {
        let browsing = PickingMode::Browsing;
        // '/' in browsing mode becomes PickFilterChar('/'), not OpenOmnibar
        assert_eq!(
            map_picking_event(key(KeyCode::Char('/')), &browsing),
            Some(Action::PickFilterChar('/'))
        );
        // '?' becomes PickFilterChar('?')
        assert_eq!(
            map_picking_event(key(KeyCode::Char('?')), &browsing),
            Some(Action::PickFilterChar('?'))
        );
    }

    fn map_hunk_picker(event: KeyEvent) -> Option<Action> {
        map_event(event, PanelFocus::Detail, DetailMode::HunkPicker)
    }

    #[test]
    fn hunk_picker_key_routing() {
        // Navigation
        assert_eq!(
            map_hunk_picker(key(KeyCode::Char('j'))),
            Some(Action::DetailMoveDown)
        );
        assert_eq!(
            map_hunk_picker(key(KeyCode::Down)),
            Some(Action::DetailMoveDown)
        );
        assert_eq!(
            map_hunk_picker(key(KeyCode::Char('k'))),
            Some(Action::DetailMoveUp)
        );
        assert_eq!(
            map_hunk_picker(key(KeyCode::Up)),
            Some(Action::DetailMoveUp)
        );
        // File-jump
        assert_eq!(
            map_hunk_picker(key_mod(KeyCode::Char('J'), KeyModifiers::SHIFT)),
            Some(Action::HunkNextFile)
        );
        assert_eq!(
            map_hunk_picker(key_mod(KeyCode::Char('K'), KeyModifiers::SHIFT)),
            Some(Action::HunkPrevFile)
        );
        // Selection
        assert_eq!(
            map_hunk_picker(key(KeyCode::Char(' '))),
            Some(Action::HunkToggle)
        );
        assert_eq!(
            map_hunk_picker(key(KeyCode::Char('a'))),
            Some(Action::HunkSelectAll)
        );
        assert_eq!(
            map_hunk_picker(key_mod(KeyCode::Char('A'), KeyModifiers::SHIFT)),
            Some(Action::HunkDeselectAll)
        );
        // Confirm / cancel
        assert_eq!(
            map_hunk_picker(key(KeyCode::Enter)),
            Some(Action::HunkConfirm)
        );
        assert_eq!(map_hunk_picker(key(KeyCode::Esc)), Some(Action::HunkCancel));
    }

    #[test]
    fn tab_accepts_completion_when_visible() {
        use crate::action::CompletionItem;
        let modal = Modal::Omnibar {
            query: "min".into(),
            matches: vec![],
            cursor: 0,
            completions: vec![CompletionItem {
                insert_text: "mine()".into(),
                display_text: "mine()".into(),
            }],
            completion_cursor: 0,
        };
        assert_eq!(
            map_modal_event(key(KeyCode::Tab), &modal),
            Some(Action::OmnibarAcceptCompletion)
        );
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
        // Tab with no completions returns None (swallowed)
        assert_eq!(map_modal_event(key(KeyCode::Tab), &modal), None);
    }

    #[test]
    fn tab_suppressed_during_hunk_picker() {
        // Tab is suppressed (returns None) when HunkPicker is active
        assert_eq!(map_hunk_picker(key(KeyCode::Tab)), None);
        assert_eq!(map_hunk_picker(key(KeyCode::BackTab)), None);
        // Tab works normally outside hunk picker
        assert_eq!(map_file_list(key(KeyCode::Tab)), Some(Action::TabFocus));
        assert_eq!(
            map_file_list(key(KeyCode::BackTab)),
            Some(Action::BackTabFocus)
        );
    }

    #[test]
    fn quit_suppressed_during_hunk_picker() {
        // q is suppressed during hunk picker
        assert_eq!(map_hunk_picker(key(KeyCode::Char('q'))), None);
        // Ctrl-C cancels the picker (emergency exit), not quit
        assert_eq!(
            map_hunk_picker(key_mod(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(Action::HunkCancel)
        );
        // q works normally in other detail modes
        assert_eq!(map_file_list(key(KeyCode::Char('q'))), Some(Action::Quit));
    }

    #[test]
    fn s_and_s_keys_map_correctly() {
        // s → Split in graph context
        assert_eq!(map_graph(key(KeyCode::Char('s'))), Some(Action::Split));
        // S → SquashPartial in graph context
        assert_eq!(
            map_graph(key_mod(KeyCode::Char('S'), KeyModifiers::SHIFT)),
            Some(Action::SquashPartial)
        );
        // Neither maps in detail context
        assert_eq!(map_file_list(key(KeyCode::Char('s'))), None);
        assert_eq!(
            map_file_list(key_mod(KeyCode::Char('S'), KeyModifiers::SHIFT)),
            None
        );
    }
}
