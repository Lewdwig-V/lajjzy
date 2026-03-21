use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{Action, DetailMode, Modal, PanelFocus};

pub fn map_event(event: KeyEvent, focus: PanelFocus, detail_mode: DetailMode) -> Option<Action> {
    // Global keys
    match (event.code, event.modifiers) {
        (KeyCode::Char('q'), KeyModifiers::NONE) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            return Some(Action::Quit);
        }
        (KeyCode::Tab, _) => return Some(Action::TabFocus),
        (KeyCode::BackTab, _) => return Some(Action::BackTabFocus),
        (KeyCode::Char('R'), _) => return Some(Action::Refresh),
        (KeyCode::Char('@'), _) => return Some(Action::JumpToWorkingCopy),
        (KeyCode::Char('O'), _) => return Some(Action::ToggleOpLog),
        (KeyCode::Char('b'), KeyModifiers::NONE) => return Some(Action::OpenBookmarks),
        (KeyCode::Char('/'), _) => return Some(Action::OpenFuzzyFind),
        (KeyCode::Char('?'), _) => return Some(Action::OpenHelp),
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
            (KeyCode::Char('S'), _) => Some(Action::Squash),
            (KeyCode::Char('u'), KeyModifiers::NONE) => Some(Action::Undo),
            (KeyCode::Char('r'), KeyModifiers::CONTROL) => Some(Action::Redo),
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
        },
    }
}

/// Map a key event when a modal is active. Returns `None` to swallow the key.
pub fn map_modal_event(event: KeyEvent, modal: &Modal) -> Option<Action> {
    // Describe modal has its own key handling (intercepts Esc differently)
    if let Modal::Describe { .. } = modal {
        return match (event.code, event.modifiers) {
            (KeyCode::Char('s') | KeyCode::Enter, KeyModifiers::CONTROL) => {
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

    let is_fuzzy = matches!(modal, Modal::FuzzyFind { .. });

    if is_fuzzy {
        match event.code {
            KeyCode::Backspace => Some(Action::FuzzyBackspace),
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
                Some(Action::FuzzyInput(c))
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
            Some(Action::OpenFuzzyFind)
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
    fn modal_q_dismisses_non_fuzzy() {
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
    fn fuzzy_q_is_text_input() {
        let modal = Modal::FuzzyFind {
            query: String::new(),
            matches: vec![],
            cursor: 0,
        };
        assert_eq!(
            map_modal_event(key(KeyCode::Char('q')), &modal),
            Some(Action::FuzzyInput('q'))
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
    fn fuzzy_ctrl_n_p_navigation() {
        let modal = Modal::FuzzyFind {
            query: String::new(),
            matches: vec![],
            cursor: 0,
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
    fn fuzzy_backspace() {
        let modal = Modal::FuzzyFind {
            query: String::new(),
            matches: vec![],
            cursor: 0,
        };
        assert_eq!(
            map_modal_event(key(KeyCode::Backspace), &modal),
            Some(Action::FuzzyBackspace)
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
        // Squash (capital S)
        assert_eq!(
            map_graph(key_mod(KeyCode::Char('S'), KeyModifiers::SHIFT)),
            Some(Action::Squash)
        );
        // Undo
        assert_eq!(map_graph(key(KeyCode::Char('u'))), Some(Action::Undo));
        // Redo (Ctrl-R)
        assert_eq!(
            map_graph(key_mod(KeyCode::Char('r'), KeyModifiers::CONTROL)),
            Some(Action::Redo)
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
}
