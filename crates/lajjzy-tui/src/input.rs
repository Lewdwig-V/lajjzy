use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{Action, DetailMode, PanelFocus};

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
        _ => {}
    }

    match focus {
        PanelFocus::Graph => match (event.code, event.modifiers) {
            (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => Some(Action::MoveDown),
            (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => Some(Action::MoveUp),
            (KeyCode::Char('g'), KeyModifiers::NONE) => Some(Action::JumpToTop),
            (KeyCode::Char('G'), _) => Some(Action::JumpToBottom),
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
}
