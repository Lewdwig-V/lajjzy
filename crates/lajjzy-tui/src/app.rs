use lajjzy_core::backend::RepoBackend;
use lajjzy_core::types::{ChangeDetail, GraphData};

pub struct AppState {
    pub graph: GraphData,
    pub cursor: usize,
    pub should_quit: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    MoveUp,
    MoveDown,
    Quit,
    Refresh,
    JumpToTop,
    JumpToBottom,
}

impl AppState {
    pub fn new(graph: GraphData) -> Self {
        let cursor = graph
            .working_copy_index
            .unwrap_or_else(|| graph.node_indices().first().copied().unwrap_or(0));
        Self {
            graph,
            cursor,
            should_quit: false,
            error: None,
        }
    }

    pub fn selected_change_id(&self) -> Option<&str> {
        self.graph
            .lines
            .get(self.cursor)
            .and_then(|line| line.change_id.as_deref())
    }

    pub fn selected_detail(&self) -> Option<&ChangeDetail> {
        self.graph.detail_at(self.cursor)
    }
}

pub fn dispatch(state: &mut AppState, action: Action, backend: &dyn RepoBackend) {
    state.error = None;

    match action {
        Action::MoveDown => {
            let nodes = state.graph.node_indices();
            if let Some(next) = nodes.iter().find(|&&i| i > state.cursor) {
                state.cursor = *next;
            }
        }
        Action::MoveUp => {
            let nodes = state.graph.node_indices();
            if let Some(prev) = nodes.iter().rev().find(|&&i| i < state.cursor) {
                state.cursor = *prev;
            }
        }
        Action::JumpToTop => {
            if let Some(&first) = state.graph.node_indices().first() {
                state.cursor = first;
            }
        }
        Action::JumpToBottom => {
            if let Some(&last) = state.graph.node_indices().last() {
                state.cursor = last;
            }
        }
        Action::Quit => {
            state.should_quit = true;
        }
        Action::Refresh => {
            let prev_change_id = state.selected_change_id().map(String::from);
            match backend.load_graph() {
                Ok(new_graph) => {
                    state.graph = new_graph;
                    let nodes = state.graph.node_indices();
                    state.cursor = prev_change_id
                        .as_deref()
                        .and_then(|id| {
                            nodes
                                .iter()
                                .find(|&&i| state.graph.lines[i].change_id.as_deref() == Some(id))
                                .copied()
                        })
                        .or(state.graph.working_copy_index)
                        .or_else(|| nodes.first().copied())
                        .unwrap_or(0);
                }
                Err(e) => {
                    state.error = Some(format!("Refresh failed: {e}"));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::collections::HashMap;

    use lajjzy_core::types::GraphLine;

    struct MockBackend {
        graph: GraphData,
    }

    impl RepoBackend for MockBackend {
        fn load_graph(&self) -> Result<GraphData> {
            Ok(self.graph.clone())
        }
    }

    struct FailingBackend;

    impl RepoBackend for FailingBackend {
        fn load_graph(&self) -> Result<GraphData> {
            anyhow::bail!("connection lost")
        }
    }

    fn sample_graph() -> GraphData {
        GraphData {
            lines: vec![
                GraphLine {
                    raw: "◉  abc".into(),
                    change_id: Some("abc".into()),
                },
                GraphLine {
                    raw: "│  desc1".into(),
                    change_id: None,
                },
                GraphLine {
                    raw: "◉  def".into(),
                    change_id: Some("def".into()),
                },
                GraphLine {
                    raw: "│  desc2".into(),
                    change_id: None,
                },
                GraphLine {
                    raw: "◉  ghi".into(),
                    change_id: Some("ghi".into()),
                },
            ],
            details: HashMap::from([
                (
                    "abc".into(),
                    ChangeDetail {
                        change_id: "abc".into(),
                        commit_id: "a1".into(),
                        author: "a".into(),
                        email: "a@b".into(),
                        timestamp: "1m".into(),
                        description: "desc1".into(),
                        bookmarks: vec![],
                        is_empty: false,
                        has_conflict: false,
                        is_working_copy: true,
                    },
                ),
                (
                    "def".into(),
                    ChangeDetail {
                        change_id: "def".into(),
                        commit_id: "d1".into(),
                        author: "b".into(),
                        email: "b@c".into(),
                        timestamp: "2m".into(),
                        description: "desc2".into(),
                        bookmarks: vec![],
                        is_empty: false,
                        has_conflict: false,
                        is_working_copy: false,
                    },
                ),
                (
                    "ghi".into(),
                    ChangeDetail {
                        change_id: "ghi".into(),
                        commit_id: "g1".into(),
                        author: "c".into(),
                        email: "c@d".into(),
                        timestamp: "3m".into(),
                        description: "desc3".into(),
                        bookmarks: vec![],
                        is_empty: false,
                        has_conflict: false,
                        is_working_copy: false,
                    },
                ),
            ]),
            working_copy_index: Some(0),
        }
    }

    #[test]
    fn initial_cursor_on_working_copy() {
        let state = AppState::new(sample_graph());
        assert_eq!(state.cursor, 0);
        assert_eq!(state.selected_change_id(), Some("abc"));
    }

    #[test]
    fn move_down_skips_connector_lines() {
        let mock = MockBackend {
            graph: sample_graph(),
        };
        let mut state = AppState::new(sample_graph());
        dispatch(&mut state, Action::MoveDown, &mock);
        assert_eq!(state.cursor, 2);
        assert_eq!(state.selected_change_id(), Some("def"));
    }

    #[test]
    fn move_up_skips_connector_lines() {
        let mock = MockBackend {
            graph: sample_graph(),
        };
        let mut state = AppState::new(sample_graph());
        state.cursor = 2;
        dispatch(&mut state, Action::MoveUp, &mock);
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn move_down_at_bottom_stays() {
        let mock = MockBackend {
            graph: sample_graph(),
        };
        let mut state = AppState::new(sample_graph());
        state.cursor = 4;
        dispatch(&mut state, Action::MoveDown, &mock);
        assert_eq!(state.cursor, 4);
    }

    #[test]
    fn move_up_at_top_stays() {
        let mock = MockBackend {
            graph: sample_graph(),
        };
        let mut state = AppState::new(sample_graph());
        dispatch(&mut state, Action::MoveUp, &mock);
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn jump_to_top() {
        let mock = MockBackend {
            graph: sample_graph(),
        };
        let mut state = AppState::new(sample_graph());
        state.cursor = 4;
        dispatch(&mut state, Action::JumpToTop, &mock);
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn jump_to_bottom() {
        let mock = MockBackend {
            graph: sample_graph(),
        };
        let mut state = AppState::new(sample_graph());
        dispatch(&mut state, Action::JumpToBottom, &mock);
        assert_eq!(state.cursor, 4);
    }

    #[test]
    fn quit_sets_flag() {
        let mock = MockBackend {
            graph: sample_graph(),
        };
        let mut state = AppState::new(sample_graph());
        dispatch(&mut state, Action::Quit, &mock);
        assert!(state.should_quit);
    }

    #[test]
    fn refresh_preserves_selected_change() {
        let mock = MockBackend {
            graph: sample_graph(),
        };
        let mut state = AppState::new(sample_graph());
        state.cursor = 2;
        dispatch(&mut state, Action::Refresh, &mock);
        assert_eq!(state.cursor, 2);
        assert_eq!(state.selected_change_id(), Some("def"));
    }

    #[test]
    fn initial_cursor_fallback_without_working_copy() {
        let mut graph = sample_graph();
        graph.working_copy_index = None;
        let state = AppState::new(graph);
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn refresh_falls_back_when_change_disappears() {
        let mut state = AppState::new(sample_graph());
        state.cursor = 2; // at "def"

        let mut new_graph = sample_graph();
        new_graph.lines.remove(3);
        new_graph.lines.remove(2);
        new_graph.details.remove("def");
        let new_mock = MockBackend { graph: new_graph };

        dispatch(&mut state, Action::Refresh, &new_mock);
        assert_eq!(state.cursor, 0);
        assert_eq!(state.selected_change_id(), Some("abc"));
    }

    #[test]
    fn refresh_error_preserves_graph_and_sets_error() {
        let mock = FailingBackend;
        let mut state = AppState::new(sample_graph());
        dispatch(&mut state, Action::Refresh, &mock);
        assert!(state.error.is_some());
        assert!(state.error.as_ref().unwrap().contains("connection lost"));
        assert_eq!(state.graph.lines.len(), 5);
    }
}
