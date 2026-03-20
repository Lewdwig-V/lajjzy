use lajjzy_core::backend::RepoBackend;
use lajjzy_core::types::{ChangeDetail, DiffHunk, GraphData};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelFocus {
    Graph,
    Detail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailMode {
    FileList,
    DiffView,
}

pub struct AppState {
    pub graph: GraphData,
    cursor: usize,
    pub should_quit: bool,
    pub error: Option<String>,
    pub focus: PanelFocus,
    detail_cursor: usize,
    pub detail_mode: DetailMode,
    pub diff_scroll: usize,
    pub diff_data: Vec<DiffHunk>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    MoveUp,
    MoveDown,
    Quit,
    Refresh,
    JumpToTop,
    JumpToBottom,
    TabFocus,
    BackTabFocus,
    DetailMoveUp,
    DetailMoveDown,
    DetailEnter,
    DetailBack,
    DiffScrollUp,
    DiffScrollDown,
    DiffNextHunk,
    DiffPrevHunk,
    JumpToWorkingCopy,
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
            focus: PanelFocus::Graph,
            detail_cursor: 0,
            detail_mode: DetailMode::FileList,
            diff_scroll: 0,
            diff_data: vec![],
        }
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn detail_cursor(&self) -> usize {
        self.detail_cursor
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

    pub fn reset_detail(&mut self) {
        self.detail_cursor = 0;
        self.detail_mode = DetailMode::FileList;
        self.diff_scroll = 0;
        self.diff_data = vec![];
    }
}

#[allow(clippy::too_many_lines)]
pub fn dispatch(state: &mut AppState, action: Action, backend: &dyn RepoBackend) {
    match action {
        Action::MoveDown => {
            let nodes = state.graph.node_indices();
            if let Some(next) = nodes.iter().find(|&&i| i > state.cursor) {
                state.cursor = *next;
            }
            state.reset_detail();
        }
        Action::MoveUp => {
            let nodes = state.graph.node_indices();
            if let Some(prev) = nodes.iter().rev().find(|&&i| i < state.cursor) {
                state.cursor = *prev;
            }
            state.reset_detail();
        }
        Action::JumpToTop => {
            if let Some(&first) = state.graph.node_indices().first() {
                state.cursor = first;
            }
            state.reset_detail();
        }
        Action::JumpToBottom => {
            if let Some(&last) = state.graph.node_indices().last() {
                state.cursor = last;
            }
            state.reset_detail();
        }
        Action::JumpToWorkingCopy => {
            if let Some(idx) = state.graph.working_copy_index {
                state.cursor = idx;
                state.reset_detail();
            }
        }
        Action::Quit => {
            state.should_quit = true;
        }
        Action::Refresh => {
            state.error = None;
            let prev_change_id = state.selected_change_id().map(String::from);
            match backend.load_graph() {
                Ok(new_graph) => {
                    state.graph = new_graph;
                    state.reset_detail();
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
        Action::TabFocus | Action::BackTabFocus => {
            state.focus = match state.focus {
                PanelFocus::Graph => PanelFocus::Detail,
                PanelFocus::Detail => PanelFocus::Graph,
            };
        }
        Action::DetailMoveDown => {
            if let Some(detail) = state.selected_detail() {
                let max = detail.files.len().saturating_sub(1);
                if state.detail_cursor < max {
                    state.detail_cursor += 1;
                }
            }
        }
        Action::DetailMoveUp => {
            state.detail_cursor = state.detail_cursor.saturating_sub(1);
        }
        Action::DetailEnter => {
            let file_info = state
                .selected_detail()
                .and_then(|d| d.files.get(state.detail_cursor))
                .map(|f| (f.path.clone(), f.status));
            let change_id = state.selected_change_id().map(String::from);

            if let (Some(cid), Some((raw_path, status))) = (change_id, file_info) {
                // For renames, extract the destination path (after "=> ")
                let diff_path = if status == lajjzy_core::types::FileStatus::Renamed {
                    raw_path
                        .split("=> ")
                        .nth(1)
                        .and_then(|s| s.strip_suffix('}'))
                        .unwrap_or(&raw_path)
                        .to_string()
                } else {
                    raw_path.clone()
                };

                match backend.file_diff(&cid, &diff_path) {
                    Ok(hunks) => {
                        state.diff_data = hunks;
                        state.diff_scroll = 0;
                        state.detail_mode = DetailMode::DiffView;
                    }
                    Err(e) => {
                        state.diff_data = vec![];
                        state.error = Some(format!("Failed to load diff for {raw_path}: {e}"));
                    }
                }
            }
        }
        Action::DetailBack => match state.detail_mode {
            DetailMode::DiffView => {
                state.detail_mode = DetailMode::FileList;
                state.diff_scroll = 0;
                state.diff_data = vec![];
            }
            DetailMode::FileList => {
                state.focus = PanelFocus::Graph;
            }
        },
        Action::DiffScrollDown => {
            let total_lines: usize = state
                .diff_data
                .iter()
                .map(|h| 1 + h.lines.len()) // header + lines
                .sum();
            if state.diff_scroll + 1 < total_lines {
                state.diff_scroll += 1;
            }
        }
        Action::DiffScrollUp => {
            state.diff_scroll = state.diff_scroll.saturating_sub(1);
        }
        Action::DiffNextHunk => {
            // Jump to the next hunk header line offset
            let mut offset = 0usize;
            for hunk in &state.diff_data {
                // offset is the position of this hunk's header
                if offset > state.diff_scroll {
                    state.diff_scroll = offset;
                    break;
                }
                offset += 1 + hunk.lines.len();
            }
        }
        Action::DiffPrevHunk => {
            // Jump to the previous hunk header line offset
            let mut offsets = vec![];
            let mut offset = 0usize;
            for hunk in &state.diff_data {
                offsets.push(offset);
                offset += 1 + hunk.lines.len();
            }
            // Find the last offset strictly less than current scroll
            if let Some(&prev) = offsets.iter().rev().find(|&&o| o < state.diff_scroll) {
                state.diff_scroll = prev;
            }
        }
    }

    // Release-mode invariant check: cursor must point to a node line
    if let Some(line) = state.graph.lines.get(state.cursor)
        && line.change_id.is_none()
    {
        state.error = Some("Internal error: cursor on non-change line".to_string());
        if let Some(&first) = state.graph.node_indices().first() {
            state.cursor = first;
        }
    }
}

#[cfg(test)]
impl AppState {
    fn set_cursor_for_test(&mut self, index: usize) {
        self.cursor = index;
    }

    pub fn set_detail_cursor_for_test(&mut self, index: usize) {
        self.detail_cursor = index;
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

        fn file_diff(
            &self,
            _change_id: &str,
            _path: &str,
        ) -> Result<Vec<lajjzy_core::types::DiffHunk>> {
            Ok(vec![])
        }
    }

    struct FailingBackend;

    impl RepoBackend for FailingBackend {
        fn load_graph(&self) -> Result<GraphData> {
            anyhow::bail!("connection lost")
        }

        fn file_diff(
            &self,
            _change_id: &str,
            _path: &str,
        ) -> Result<Vec<lajjzy_core::types::DiffHunk>> {
            anyhow::bail!("connection lost")
        }
    }

    fn sample_graph() -> GraphData {
        GraphData::new(
            vec![
                GraphLine {
                    raw: "◉  abc".into(),
                    change_id: Some("abc".into()),
                    glyph_prefix: String::new(),
                },
                GraphLine {
                    raw: "│  desc1".into(),
                    change_id: None,
                    glyph_prefix: String::new(),
                },
                GraphLine {
                    raw: "◉  def".into(),
                    change_id: Some("def".into()),
                    glyph_prefix: String::new(),
                },
                GraphLine {
                    raw: "│  desc2".into(),
                    change_id: None,
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
                        has_conflict: false,
                        files: vec![],
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
                        has_conflict: false,
                        files: vec![],
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
                        has_conflict: false,
                        files: vec![],
                    },
                ),
            ]),
            Some(0),
        )
    }

    fn sample_graph_with_files() -> GraphData {
        use lajjzy_core::types::{FileChange, FileStatus};
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
                        has_conflict: false,
                        files: vec![
                            FileChange {
                                path: "src/main.rs".into(),
                                status: FileStatus::Modified,
                            },
                            FileChange {
                                path: "src/lib.rs".into(),
                                status: FileStatus::Added,
                            },
                        ],
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
                        has_conflict: false,
                        files: vec![],
                    },
                ),
            ]),
            Some(0),
        )
    }

    /// Mock that returns non-empty hunks from `file_diff`.
    struct DiffMockBackend {
        graph: GraphData,
    }

    impl RepoBackend for DiffMockBackend {
        fn load_graph(&self) -> Result<GraphData> {
            Ok(self.graph.clone())
        }

        fn file_diff(
            &self,
            _change_id: &str,
            _path: &str,
        ) -> Result<Vec<lajjzy_core::types::DiffHunk>> {
            Ok(vec![lajjzy_core::types::DiffHunk {
                header: "@@ -1,1 +1,1 @@".to_string(),
                lines: vec![
                    lajjzy_core::types::DiffLine {
                        kind: lajjzy_core::types::DiffLineKind::Removed,
                        content: "old".to_string(),
                    },
                    lajjzy_core::types::DiffLine {
                        kind: lajjzy_core::types::DiffLineKind::Added,
                        content: "new".to_string(),
                    },
                ],
            }])
        }
    }

    fn mock() -> MockBackend {
        MockBackend {
            graph: sample_graph(),
        }
    }

    #[test]
    fn initial_cursor_on_working_copy() {
        let state = AppState::new(sample_graph());
        assert_eq!(state.cursor(), 0);
        assert_eq!(state.selected_change_id(), Some("abc"));
    }

    #[test]
    fn move_down_skips_connector_lines() {
        let mut state = AppState::new(sample_graph());
        dispatch(&mut state, Action::MoveDown, &mock());
        assert_eq!(state.cursor(), 2);
        assert_eq!(state.selected_change_id(), Some("def"));
    }

    #[test]
    fn move_up_skips_connector_lines() {
        let mut state = AppState::new(sample_graph());
        state.set_cursor_for_test(2);
        dispatch(&mut state, Action::MoveUp, &mock());
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn move_down_at_bottom_stays() {
        let mut state = AppState::new(sample_graph());
        state.set_cursor_for_test(4);
        dispatch(&mut state, Action::MoveDown, &mock());
        assert_eq!(state.cursor(), 4);
    }

    #[test]
    fn move_up_at_top_stays() {
        let mut state = AppState::new(sample_graph());
        dispatch(&mut state, Action::MoveUp, &mock());
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn jump_to_top() {
        let mut state = AppState::new(sample_graph());
        state.set_cursor_for_test(4);
        dispatch(&mut state, Action::JumpToTop, &mock());
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn jump_to_bottom() {
        let mut state = AppState::new(sample_graph());
        dispatch(&mut state, Action::JumpToBottom, &mock());
        assert_eq!(state.cursor(), 4);
    }

    #[test]
    fn quit_sets_flag() {
        let mut state = AppState::new(sample_graph());
        dispatch(&mut state, Action::Quit, &mock());
        assert!(state.should_quit);
    }

    #[test]
    fn refresh_preserves_selected_change() {
        let mut state = AppState::new(sample_graph());
        state.set_cursor_for_test(2);
        dispatch(&mut state, Action::Refresh, &mock());
        assert_eq!(state.cursor(), 2);
        assert_eq!(state.selected_change_id(), Some("def"));
    }

    #[test]
    fn initial_cursor_fallback_without_working_copy() {
        let mut graph = sample_graph();
        graph.working_copy_index = None;
        let state = AppState::new(graph);
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn refresh_falls_back_when_change_disappears() {
        let mut state = AppState::new(sample_graph());
        state.set_cursor_for_test(2); // at "def"

        // Build a new graph without the "def" change
        let sg = sample_graph();
        let mut lines: Vec<GraphLine> = sg.lines.into_iter().collect();
        lines.remove(3);
        lines.remove(2);
        let mut details = sg.details;
        details.remove("def");
        let new_graph = GraphData::new(lines, details, sg.working_copy_index);
        let new_mock = MockBackend { graph: new_graph };

        dispatch(&mut state, Action::Refresh, &new_mock);
        assert_eq!(state.cursor(), 0);
        assert_eq!(state.selected_change_id(), Some("abc"));
    }

    #[test]
    fn refresh_error_preserves_graph_and_sets_error() {
        let mut state = AppState::new(sample_graph());
        dispatch(&mut state, Action::Refresh, &FailingBackend);
        assert!(state.error.is_some());
        assert!(state.error.as_ref().unwrap().contains("connection lost"));
        assert_eq!(state.graph.lines.len(), 5);
    }

    #[test]
    fn navigation_preserves_error() {
        let mut state = AppState::new(sample_graph());
        state.error = Some("old error".into());
        dispatch(&mut state, Action::MoveDown, &mock());
        assert!(state.error.is_some());
        assert!(state.error.as_ref().unwrap().contains("old error"));
    }

    #[test]
    fn refresh_clears_error_on_success() {
        let mut state = AppState::new(sample_graph());
        state.error = Some("old error".into());
        dispatch(&mut state, Action::Refresh, &mock());
        assert!(state.error.is_none());
    }

    // --- New tests for Task 6 ---

    #[test]
    fn new_state_initializes_detail_fields() {
        let state = AppState::new(sample_graph());
        assert_eq!(state.focus, PanelFocus::Graph);
        assert_eq!(state.detail_cursor(), 0);
        assert_eq!(state.detail_mode, DetailMode::FileList);
        assert_eq!(state.diff_scroll, 0);
        assert!(state.diff_data.is_empty());
    }

    #[test]
    fn tab_focus_toggles() {
        let mut state = AppState::new(sample_graph());
        assert_eq!(state.focus, PanelFocus::Graph);
        dispatch(&mut state, Action::TabFocus, &mock());
        assert_eq!(state.focus, PanelFocus::Detail);
        dispatch(&mut state, Action::TabFocus, &mock());
        assert_eq!(state.focus, PanelFocus::Graph);
        // BackTabFocus also toggles
        dispatch(&mut state, Action::BackTabFocus, &mock());
        assert_eq!(state.focus, PanelFocus::Detail);
    }

    #[test]
    fn graph_cursor_move_resets_detail() {
        let mut state = AppState::new(sample_graph_with_files());
        state.set_detail_cursor_for_test(1);
        state.detail_mode = DetailMode::DiffView;
        state.diff_scroll = 5;
        dispatch(&mut state, Action::MoveDown, &mock());
        assert_eq!(state.detail_cursor(), 0);
        assert_eq!(state.detail_mode, DetailMode::FileList);
        assert_eq!(state.diff_scroll, 0);
        assert!(state.diff_data.is_empty());
    }

    #[test]
    fn jump_to_working_copy() {
        let mut state = AppState::new(sample_graph());
        state.set_cursor_for_test(4);
        dispatch(&mut state, Action::JumpToWorkingCopy, &mock());
        assert_eq!(state.cursor(), 0);
        assert_eq!(state.selected_change_id(), Some("abc"));
    }

    #[test]
    fn jump_to_working_copy_noop_when_none() {
        let mut graph = sample_graph();
        graph.working_copy_index = None;
        let mut state = AppState::new(graph);
        state.set_cursor_for_test(4);
        dispatch(&mut state, Action::JumpToWorkingCopy, &mock());
        // cursor stays at 4 since there's no working copy
        assert_eq!(state.cursor(), 4);
    }

    #[test]
    fn detail_back_from_diff_returns_to_file_list() {
        let mut state = AppState::new(sample_graph());
        state.focus = PanelFocus::Detail;
        state.detail_mode = DetailMode::DiffView;
        dispatch(&mut state, Action::DetailBack, &mock());
        assert_eq!(state.detail_mode, DetailMode::FileList);
        assert_eq!(state.focus, PanelFocus::Detail);
    }

    #[test]
    fn detail_back_from_file_list_returns_focus_to_graph() {
        let mut state = AppState::new(sample_graph());
        state.focus = PanelFocus::Detail;
        state.detail_mode = DetailMode::FileList;
        dispatch(&mut state, Action::DetailBack, &mock());
        assert_eq!(state.focus, PanelFocus::Graph);
    }

    #[test]
    fn detail_enter_with_no_files_is_noop() {
        // sample_graph has empty files lists
        let mut state = AppState::new(sample_graph());
        state.focus = PanelFocus::Detail;
        dispatch(&mut state, Action::DetailEnter, &mock());
        // mode stays FileList, no diff data loaded
        assert_eq!(state.detail_mode, DetailMode::FileList);
        assert!(state.diff_data.is_empty());
    }

    #[test]
    fn detail_enter_loads_diff() {
        let graph = sample_graph_with_files();
        let mock = DiffMockBackend {
            graph: graph.clone(),
        };
        let mut state = AppState::new(graph);
        state.focus = PanelFocus::Detail;

        dispatch(&mut state, Action::DetailEnter, &mock);
        assert_eq!(state.detail_mode, DetailMode::DiffView);
        assert!(!state.diff_data.is_empty());
    }

    #[test]
    fn detail_enter_error_sets_state_error() {
        let mut state = AppState::new(sample_graph_with_files());
        state.focus = PanelFocus::Detail;

        dispatch(&mut state, Action::DetailEnter, &FailingBackend);
        assert!(state.error.is_some());
        assert_eq!(state.detail_mode, DetailMode::FileList); // didn't switch
    }

    #[test]
    fn detail_move_down_with_files() {
        let mock = MockBackend {
            graph: sample_graph_with_files(),
        };
        let mut state = AppState::new(sample_graph_with_files());
        assert_eq!(state.detail_cursor(), 0);

        dispatch(&mut state, Action::DetailMoveDown, &mock);
        assert_eq!(state.detail_cursor(), 1);
    }

    #[test]
    fn detail_move_down_at_boundary_stays() {
        let mock = MockBackend {
            graph: sample_graph_with_files(),
        };
        let mut state = AppState::new(sample_graph_with_files());
        let file_count = state.selected_detail().unwrap().files.len();
        for _ in 0..file_count {
            dispatch(&mut state, Action::DetailMoveDown, &mock);
        }
        let cursor_before = state.detail_cursor();
        dispatch(&mut state, Action::DetailMoveDown, &mock);
        assert_eq!(state.detail_cursor(), cursor_before);
    }

    #[test]
    fn detail_move_up_at_zero_stays() {
        let mock = MockBackend {
            graph: sample_graph_with_files(),
        };
        let mut state = AppState::new(sample_graph_with_files());
        dispatch(&mut state, Action::DetailMoveUp, &mock);
        assert_eq!(state.detail_cursor(), 0);
    }

    #[test]
    fn refresh_resets_detail_state() {
        let mock = MockBackend {
            graph: sample_graph(),
        };
        let mut state = AppState::new(sample_graph());
        state.detail_mode = DetailMode::DiffView;
        state.diff_scroll = 5;

        dispatch(&mut state, Action::Refresh, &mock);
        assert_eq!(state.detail_mode, DetailMode::FileList);
        assert_eq!(state.diff_scroll, 0);
        assert_eq!(state.detail_cursor(), 0);
    }
}
