use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::Action;

/// Map a crossterm key event to an Action.
pub fn map_event(event: KeyEvent) -> Option<Action> {
    match (event.code, event.modifiers) {
        (KeyCode::Char('q'), KeyModifiers::NONE) => Some(Action::Quit),
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => Some(Action::Quit),
        (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => Some(Action::MoveDown),
        (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => Some(Action::MoveUp),
        (KeyCode::Char('R'), _) => Some(Action::Refresh),
        (KeyCode::Char('g'), KeyModifiers::NONE) => Some(Action::JumpToTop),
        (KeyCode::Char('G'), _) => Some(Action::JumpToBottom),
        _ => None,
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

    #[test]
    fn quit_keys() {
        assert_eq!(map_event(key(KeyCode::Char('q'))), Some(Action::Quit));
        assert_eq!(
            map_event(key_mod(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(Action::Quit)
        );
    }

    #[test]
    fn navigation_keys() {
        assert_eq!(map_event(key(KeyCode::Char('j'))), Some(Action::MoveDown));
        assert_eq!(map_event(key(KeyCode::Down)), Some(Action::MoveDown));
        assert_eq!(map_event(key(KeyCode::Char('k'))), Some(Action::MoveUp));
        assert_eq!(map_event(key(KeyCode::Up)), Some(Action::MoveUp));
    }

    #[test]
    fn jump_keys() {
        assert_eq!(map_event(key(KeyCode::Char('g'))), Some(Action::JumpToTop));
        assert_eq!(
            map_event(key(KeyCode::Char('G'))),
            Some(Action::JumpToBottom)
        );
    }

    #[test]
    fn refresh_key() {
        assert_eq!(map_event(key(KeyCode::Char('R'))), Some(Action::Refresh));
    }

    #[test]
    fn unmapped_key_returns_none() {
        assert_eq!(map_event(key(KeyCode::Char('x'))), None);
    }
}
