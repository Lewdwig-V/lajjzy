use lajjzy_core::types::GraphData;
use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::action::{Action, DetailMode, PanelFocus};
use crate::app::AppState;
use crate::effect::Effect;
use crate::modal::{HelpContext, Modal};

#[allow(clippy::too_many_lines)]
#[allow(clippy::needless_pass_by_value)]
pub fn dispatch(state: &mut AppState, action: Action) -> Vec<Effect> {
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
            return vec![Effect::LoadGraph { revset: None }];
        }
        Action::GraphLoaded(result) => match result {
            Ok(new_graph) => {
                let prev_id = state.selected_change_id().map(String::from);
                state.graph = new_graph;
                state.reset_detail();

                if state.cursor_follows_working_copy {
                    state.cursor_follows_working_copy = false;
                    state.cursor = state
                        .graph
                        .working_copy_index
                        .or_else(|| state.graph.node_indices().first().copied())
                        .unwrap_or(0);
                } else {
                    let nodes = state.graph.node_indices();
                    state.cursor = prev_id
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
            }
            Err(e) => {
                state.error = Some(format!("Failed to load graph: {e}"));
            }
        },
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

                return vec![Effect::LoadFileDiff {
                    change_id: cid,
                    path: diff_path,
                }];
            }
        }
        Action::FileDiffLoaded(result) => match result {
            Ok(hunks) => {
                state.diff_data = hunks;
                state.diff_scroll = 0;
                state.detail_mode = DetailMode::DiffView;
            }
            Err(e) => {
                state.diff_data = vec![];
                state.error = Some(format!("Failed to load diff: {e}"));
            }
        },
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
        Action::ToggleOpLog => {
            if matches!(state.modal, Some(Modal::OpLog { .. })) {
                state.modal = None;
            } else {
                return vec![Effect::LoadOpLog];
            }
        }
        Action::OpLogLoaded(result) => match result {
            Ok(entries) => {
                state.modal = Some(Modal::OpLog {
                    entries,
                    cursor: 0,
                    scroll: 0,
                });
            }
            Err(e) => {
                state.error = Some(format!("Failed to load op log: {e}"));
            }
        },
        Action::OpenBookmarks => {
            let mut bookmarks = Vec::new();
            for &idx in state.graph.node_indices() {
                if let Some(cid) = state.graph.lines[idx].change_id.as_ref()
                    && let Some(detail) = state.graph.details.get(cid)
                {
                    for bm in &detail.bookmarks {
                        bookmarks.push((bm.clone(), cid.clone()));
                    }
                }
            }
            state.modal = Some(Modal::BookmarkPicker {
                bookmarks,
                cursor: 0,
            });
        }
        Action::OpenFuzzyFind => {
            let matches = state.graph.node_indices().to_vec();
            state.modal = Some(Modal::FuzzyFind {
                query: String::new(),
                matches,
                cursor: 0,
            });
        }
        Action::OpenHelp => {
            let context = match state.focus {
                PanelFocus::Graph => HelpContext::Graph,
                PanelFocus::Detail => match state.detail_mode {
                    DetailMode::FileList => HelpContext::DetailFileList,
                    DetailMode::DiffView => HelpContext::DetailDiffView,
                },
            };
            state.modal = Some(Modal::Help { context, scroll: 0 });
        }
        Action::ModalDismiss => {
            state.modal = None;
        }
        Action::ModalMoveDown => {
            if let Some(ref mut modal) = state.modal {
                match modal {
                    Modal::OpLog {
                        entries, cursor, ..
                    } => {
                        if *cursor + 1 < entries.len() {
                            *cursor += 1;
                        }
                    }
                    Modal::BookmarkPicker {
                        bookmarks, cursor, ..
                    } => {
                        if *cursor + 1 < bookmarks.len() {
                            *cursor += 1;
                        }
                    }
                    Modal::FuzzyFind {
                        matches, cursor, ..
                    } => {
                        if *cursor + 1 < matches.len() {
                            *cursor += 1;
                        }
                    }
                    Modal::Help { context, scroll } => {
                        if *scroll + 1 < context.line_count() {
                            *scroll += 1;
                        }
                    }
                    Modal::Describe { .. } | Modal::BookmarkInput { .. } => {}
                }
            }
        }
        Action::ModalMoveUp => {
            if let Some(ref mut modal) = state.modal {
                match modal {
                    Modal::OpLog { cursor, .. }
                    | Modal::BookmarkPicker { cursor, .. }
                    | Modal::FuzzyFind { cursor, .. } => {
                        *cursor = cursor.saturating_sub(1);
                    }
                    Modal::Help { scroll, .. } => {
                        *scroll = scroll.saturating_sub(1);
                    }
                    Modal::Describe { .. } | Modal::BookmarkInput { .. } => {}
                }
            }
        }
        Action::ModalEnter => {
            let modal = state.modal.take();
            match modal {
                Some(Modal::BookmarkPicker {
                    bookmarks, cursor, ..
                }) => {
                    if let Some((_, change_id)) = bookmarks.get(cursor)
                        && let Some(&idx) = state.graph.node_indices().iter().find(|&&i| {
                            state.graph.lines[i].change_id.as_deref() == Some(change_id)
                        })
                    {
                        state.cursor = idx;
                        state.reset_detail();
                    }
                }
                Some(Modal::FuzzyFind {
                    matches, cursor, ..
                }) => {
                    if let Some(&idx) = matches.get(cursor) {
                        state.cursor = idx;
                        state.reset_detail();
                    }
                }
                other => {
                    state.modal = other;
                }
            }
        }
        Action::FuzzyInput(c) => {
            if let Some(Modal::FuzzyFind {
                query,
                matches,
                cursor,
            }) = &mut state.modal
            {
                query.push(c);
                *matches = fuzzy_match(query, &state.graph);
                *cursor = 0;
            }
        }
        Action::FuzzyBackspace => {
            if let Some(Modal::FuzzyFind {
                query,
                matches,
                cursor,
            }) = &mut state.modal
            {
                query.pop();
                *matches = fuzzy_match(query, &state.graph);
                *cursor = 0;
            }
        }
        // M2 actions — handled in later tasks; no-op for now.
        Action::RepoOpSuccess { .. }
        | Action::RepoOpFailed { .. }
        | Action::EditorComplete { .. }
        | Action::Abandon
        | Action::Squash
        | Action::NewChange
        | Action::EditChange
        | Action::OpenDescribe
        | Action::Undo
        | Action::Redo
        | Action::OpenBookmarkSet
        | Action::BookmarkInputChar(_)
        | Action::BookmarkInputBackspace
        | Action::BookmarkInputConfirm
        | Action::BookmarkDelete
        | Action::GitPush
        | Action::GitFetch
        | Action::DescribeSave
        | Action::DescribeEscalateEditor => {}
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

    vec![]
}

fn fuzzy_match(query: &str, graph: &GraphData) -> Vec<usize> {
    if query.is_empty() {
        return graph.node_indices().to_vec();
    }

    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::new(
        query,
        CaseMatching::Smart,
        Normalization::Smart,
        AtomKind::Fuzzy,
    );

    let mut scored: Vec<(usize, u32)> = Vec::new();
    for &idx in graph.node_indices() {
        if let Some(cid) = graph.lines[idx].change_id.as_ref()
            && let Some(detail) = graph.details.get(cid)
        {
            let haystack = format!("{cid} {} {}", detail.author, detail.description);
            let mut buf: Vec<char> = Vec::new();
            if let Some(score) = pattern.score(Utf32Str::new(&haystack, &mut buf), &mut matcher) {
                scored.push((idx, score));
            }
        }
    }
    scored.sort_by(|a, b| b.1.cmp(&a.1));
    scored.into_iter().map(|(idx, _)| idx).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use lajjzy_core::types::{ChangeDetail, GraphData, GraphLine};

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

    fn sample_graph_with_bookmarks() -> GraphData {
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
                        bookmarks: vec!["main".into()],
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
                        bookmarks: vec!["feature".into()],
                        is_empty: false,
                        has_conflict: false,
                        files: vec![],
                    },
                ),
            ]),
            Some(0),
        )
    }

    fn test_graph_with_changes(change_ids: &[&str]) -> GraphData {
        let lines: Vec<GraphLine> = change_ids
            .iter()
            .map(|id| GraphLine {
                raw: format!("◉  {id}"),
                change_id: Some((*id).to_string()),
                glyph_prefix: String::new(),
            })
            .collect();
        let details: HashMap<String, ChangeDetail> = change_ids
            .iter()
            .map(|id| {
                (
                    (*id).to_string(),
                    ChangeDetail {
                        commit_id: format!("{id}_commit"),
                        author: "test".into(),
                        email: "test@test".into(),
                        timestamp: "0m".into(),
                        description: format!("desc for {id}"),
                        bookmarks: vec![],
                        is_empty: false,
                        has_conflict: false,
                        files: vec![],
                    },
                )
            })
            .collect();
        GraphData::new(lines, details, Some(0))
    }

    // --- Navigation tests ---

    #[test]
    fn initial_cursor_on_working_copy() {
        let state = AppState::new(sample_graph());
        assert_eq!(state.cursor(), 0);
        assert_eq!(state.selected_change_id(), Some("abc"));
    }

    #[test]
    fn move_down_skips_connector_lines() {
        let mut state = AppState::new(sample_graph());
        let effects = dispatch(&mut state, Action::MoveDown);
        assert!(effects.is_empty());
        assert_eq!(state.cursor(), 2);
        assert_eq!(state.selected_change_id(), Some("def"));
    }

    #[test]
    fn move_up_skips_connector_lines() {
        let mut state = AppState::new(sample_graph());
        state.set_cursor_for_test(2);
        let effects = dispatch(&mut state, Action::MoveUp);
        assert!(effects.is_empty());
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn move_down_at_bottom_stays() {
        let mut state = AppState::new(sample_graph());
        state.set_cursor_for_test(4);
        let effects = dispatch(&mut state, Action::MoveDown);
        assert!(effects.is_empty());
        assert_eq!(state.cursor(), 4);
    }

    #[test]
    fn move_up_at_top_stays() {
        let mut state = AppState::new(sample_graph());
        let effects = dispatch(&mut state, Action::MoveUp);
        assert!(effects.is_empty());
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn jump_to_top() {
        let mut state = AppState::new(sample_graph());
        state.set_cursor_for_test(4);
        let effects = dispatch(&mut state, Action::JumpToTop);
        assert!(effects.is_empty());
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn jump_to_bottom() {
        let mut state = AppState::new(sample_graph());
        let effects = dispatch(&mut state, Action::JumpToBottom);
        assert!(effects.is_empty());
        assert_eq!(state.cursor(), 4);
    }

    #[test]
    fn quit_sets_flag() {
        let mut state = AppState::new(sample_graph());
        let effects = dispatch(&mut state, Action::Quit);
        assert!(effects.is_empty());
        assert!(state.should_quit);
    }

    // --- Refresh / GraphLoaded tests ---

    #[test]
    fn refresh_emits_load_graph() {
        let mut state = AppState::new(sample_graph());
        state.error = Some("old error".into());
        let effects = dispatch(&mut state, Action::Refresh);
        assert_eq!(effects, vec![Effect::LoadGraph { revset: None }]);
        assert!(state.error.is_none()); // error cleared
    }

    #[test]
    fn graph_loaded_success_updates_graph() {
        let mut state = AppState::new(sample_graph());
        let new_graph = test_graph_with_changes(&["xxx", "yyy"]);
        let effects = dispatch(&mut state, Action::GraphLoaded(Ok(new_graph)));
        assert!(effects.is_empty());
        assert_eq!(state.graph.lines.len(), 2);
        assert_eq!(state.selected_change_id(), Some("xxx"));
    }

    #[test]
    fn graph_loaded_error_sets_error() {
        let mut state = AppState::new(sample_graph());
        let effects = dispatch(&mut state, Action::GraphLoaded(Err("boom".into())));
        assert!(effects.is_empty());
        assert!(state.error.as_deref().unwrap().contains("boom"));
        // Graph unchanged
        assert_eq!(state.graph.lines.len(), 5);
    }

    #[test]
    fn graph_loaded_preserves_selected_change() {
        let mut state = AppState::new(sample_graph());
        state.set_cursor_for_test(2); // at "def"
        let new_graph = sample_graph();
        let effects = dispatch(&mut state, Action::GraphLoaded(Ok(new_graph)));
        assert!(effects.is_empty());
        assert_eq!(state.cursor(), 2);
        assert_eq!(state.selected_change_id(), Some("def"));
    }

    #[test]
    fn graph_loaded_falls_back_when_change_disappears() {
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

        let effects = dispatch(&mut state, Action::GraphLoaded(Ok(new_graph)));
        assert!(effects.is_empty());
        assert_eq!(state.cursor(), 0);
        assert_eq!(state.selected_change_id(), Some("abc"));
    }

    #[test]
    fn graph_loaded_follows_working_copy() {
        let mut state = AppState::new(sample_graph());
        state.cursor_follows_working_copy = true;
        // New graph has working copy at index 0 (first node)
        let new_graph = test_graph_with_changes(&["zzz", "yyy"]);
        let effects = dispatch(&mut state, Action::GraphLoaded(Ok(new_graph)));
        assert!(effects.is_empty());
        assert!(!state.cursor_follows_working_copy); // flag cleared
        assert_eq!(state.cursor(), 0);
        assert_eq!(state.selected_change_id(), Some("zzz"));
    }

    #[test]
    fn graph_loaded_resets_detail_state() {
        let mut state = AppState::new(sample_graph());
        state.detail_mode = DetailMode::DiffView;
        state.diff_scroll = 5;
        let new_graph = sample_graph();
        let effects = dispatch(&mut state, Action::GraphLoaded(Ok(new_graph)));
        assert!(effects.is_empty());
        assert_eq!(state.detail_mode, DetailMode::FileList);
        assert_eq!(state.diff_scroll, 0);
        assert_eq!(state.detail_cursor(), 0);
    }

    #[test]
    fn initial_cursor_fallback_without_working_copy() {
        let mut graph = sample_graph();
        graph.working_copy_index = None;
        let state = AppState::new(graph);
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn navigation_preserves_error() {
        let mut state = AppState::new(sample_graph());
        state.error = Some("old error".into());
        let effects = dispatch(&mut state, Action::MoveDown);
        assert!(effects.is_empty());
        assert!(state.error.as_ref().unwrap().contains("old error"));
    }

    // --- Detail / FileDiff tests ---

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
        dispatch(&mut state, Action::TabFocus);
        assert_eq!(state.focus, PanelFocus::Detail);
        dispatch(&mut state, Action::TabFocus);
        assert_eq!(state.focus, PanelFocus::Graph);
        dispatch(&mut state, Action::BackTabFocus);
        assert_eq!(state.focus, PanelFocus::Detail);
    }

    #[test]
    fn graph_cursor_move_resets_detail() {
        let mut state = AppState::new(sample_graph_with_files());
        state.set_detail_cursor_for_test(1);
        state.detail_mode = DetailMode::DiffView;
        state.diff_scroll = 5;
        dispatch(&mut state, Action::MoveDown);
        assert_eq!(state.detail_cursor(), 0);
        assert_eq!(state.detail_mode, DetailMode::FileList);
        assert_eq!(state.diff_scroll, 0);
        assert!(state.diff_data.is_empty());
    }

    #[test]
    fn jump_to_working_copy() {
        let mut state = AppState::new(sample_graph());
        state.set_cursor_for_test(4);
        dispatch(&mut state, Action::JumpToWorkingCopy);
        assert_eq!(state.cursor(), 0);
        assert_eq!(state.selected_change_id(), Some("abc"));
    }

    #[test]
    fn jump_to_working_copy_noop_when_none() {
        let mut graph = sample_graph();
        graph.working_copy_index = None;
        let mut state = AppState::new(graph);
        state.set_cursor_for_test(4);
        dispatch(&mut state, Action::JumpToWorkingCopy);
        assert_eq!(state.cursor(), 4);
    }

    #[test]
    fn detail_back_from_diff_returns_to_file_list() {
        let mut state = AppState::new(sample_graph());
        state.focus = PanelFocus::Detail;
        state.detail_mode = DetailMode::DiffView;
        dispatch(&mut state, Action::DetailBack);
        assert_eq!(state.detail_mode, DetailMode::FileList);
        assert_eq!(state.focus, PanelFocus::Detail);
    }

    #[test]
    fn detail_back_from_file_list_returns_focus_to_graph() {
        let mut state = AppState::new(sample_graph());
        state.focus = PanelFocus::Detail;
        state.detail_mode = DetailMode::FileList;
        dispatch(&mut state, Action::DetailBack);
        assert_eq!(state.focus, PanelFocus::Graph);
    }

    #[test]
    fn detail_enter_with_no_files_is_noop() {
        let mut state = AppState::new(sample_graph());
        state.focus = PanelFocus::Detail;
        let effects = dispatch(&mut state, Action::DetailEnter);
        assert!(effects.is_empty());
        assert_eq!(state.detail_mode, DetailMode::FileList);
        assert!(state.diff_data.is_empty());
    }

    #[test]
    fn detail_enter_emits_load_file_diff() {
        let mut state = AppState::new(sample_graph_with_files());
        state.focus = PanelFocus::Detail;
        let effects = dispatch(&mut state, Action::DetailEnter);
        assert_eq!(
            effects,
            vec![Effect::LoadFileDiff {
                change_id: "abc".into(),
                path: "src/main.rs".into(),
            }]
        );
    }

    #[test]
    fn file_diff_loaded_success_updates_state() {
        use lajjzy_core::types::{DiffHunk, DiffLine, DiffLineKind};
        let mut state = AppState::new(sample_graph());
        let hunks = vec![DiffHunk {
            header: "@@ -1,1 +1,1 @@".into(),
            lines: vec![
                DiffLine {
                    kind: DiffLineKind::Removed,
                    content: "old".into(),
                },
                DiffLine {
                    kind: DiffLineKind::Added,
                    content: "new".into(),
                },
            ],
        }];
        let effects = dispatch(&mut state, Action::FileDiffLoaded(Ok(hunks.clone())));
        assert!(effects.is_empty());
        assert_eq!(state.detail_mode, DetailMode::DiffView);
        assert_eq!(state.diff_data, hunks);
        assert_eq!(state.diff_scroll, 0);
    }

    #[test]
    fn file_diff_loaded_error_sets_error() {
        let mut state = AppState::new(sample_graph());
        let effects = dispatch(&mut state, Action::FileDiffLoaded(Err("disk error".into())));
        assert!(effects.is_empty());
        assert!(state.error.as_deref().unwrap().contains("disk error"));
        assert!(state.diff_data.is_empty());
    }

    #[test]
    fn detail_move_down_with_files() {
        let mut state = AppState::new(sample_graph_with_files());
        assert_eq!(state.detail_cursor(), 0);
        dispatch(&mut state, Action::DetailMoveDown);
        assert_eq!(state.detail_cursor(), 1);
    }

    #[test]
    fn detail_move_down_at_boundary_stays() {
        let mut state = AppState::new(sample_graph_with_files());
        let file_count = state.selected_detail().unwrap().files.len();
        for _ in 0..file_count {
            dispatch(&mut state, Action::DetailMoveDown);
        }
        let cursor_before = state.detail_cursor();
        dispatch(&mut state, Action::DetailMoveDown);
        assert_eq!(state.detail_cursor(), cursor_before);
    }

    #[test]
    fn detail_move_up_at_zero_stays() {
        let mut state = AppState::new(sample_graph_with_files());
        dispatch(&mut state, Action::DetailMoveUp);
        assert_eq!(state.detail_cursor(), 0);
    }

    // --- ToggleOpLog / OpLogLoaded tests ---

    #[test]
    fn toggle_op_log_emits_load_op_log() {
        let mut state = AppState::new(sample_graph());
        assert!(state.modal.is_none());
        let effects = dispatch(&mut state, Action::ToggleOpLog);
        assert_eq!(effects, vec![Effect::LoadOpLog]);
    }

    #[test]
    fn toggle_op_log_closes_when_open() {
        let mut state = AppState::new(sample_graph());
        state.modal = Some(Modal::OpLog {
            entries: vec![],
            cursor: 0,
            scroll: 0,
        });
        let effects = dispatch(&mut state, Action::ToggleOpLog);
        assert!(effects.is_empty());
        assert!(state.modal.is_none());
    }

    #[test]
    fn op_log_loaded_success_opens_modal() {
        use lajjzy_core::types::OpLogEntry;
        let mut state = AppState::new(sample_graph());
        let entries = vec![OpLogEntry {
            id: "op1".into(),
            description: "test op".into(),
            timestamp: "now".into(),
        }];
        let effects = dispatch(&mut state, Action::OpLogLoaded(Ok(entries.clone())));
        assert!(effects.is_empty());
        match &state.modal {
            Some(Modal::OpLog {
                entries: e,
                cursor,
                scroll,
            }) => {
                assert_eq!(e.len(), 1);
                assert_eq!(*cursor, 0);
                assert_eq!(*scroll, 0);
            }
            _ => panic!("Expected OpLog modal"),
        }
    }

    #[test]
    fn op_log_loaded_error_sets_error() {
        let mut state = AppState::new(sample_graph());
        let effects = dispatch(&mut state, Action::OpLogLoaded(Err("op fail".into())));
        assert!(effects.is_empty());
        assert!(state.error.as_deref().unwrap().contains("op fail"));
        assert!(state.modal.is_none());
    }

    // --- Modal system tests ---

    #[test]
    fn modal_dismiss_clears_modal() {
        let mut state = AppState::new(sample_graph());
        state.modal = Some(Modal::OpLog {
            entries: vec![],
            cursor: 0,
            scroll: 0,
        });
        dispatch(&mut state, Action::ModalDismiss);
        assert!(state.modal.is_none());
        assert!(!state.should_quit);
    }

    #[test]
    fn open_help_captures_context() {
        let mut state = AppState::new(sample_graph());
        state.focus = PanelFocus::Detail;
        state.detail_mode = DetailMode::DiffView;
        dispatch(&mut state, Action::OpenHelp);
        match &state.modal {
            Some(Modal::Help { context, .. }) => assert_eq!(*context, HelpContext::DetailDiffView),
            _ => panic!("Expected Help modal"),
        }
    }

    #[test]
    fn open_bookmarks_collects_from_graph() {
        let mut state = AppState::new(sample_graph());
        dispatch(&mut state, Action::OpenBookmarks);
        match &state.modal {
            Some(Modal::BookmarkPicker { bookmarks, .. }) => {
                assert!(bookmarks.is_empty());
            }
            _ => panic!("Expected BookmarkPicker modal"),
        }
    }

    #[test]
    fn fuzzy_find_opens_with_all_matches() {
        let mut state = AppState::new(sample_graph());
        dispatch(&mut state, Action::OpenFuzzyFind);
        match &state.modal {
            Some(Modal::FuzzyFind { matches, query, .. }) => {
                assert!(query.is_empty());
                assert_eq!(matches.len(), state.graph.node_indices().len());
            }
            _ => panic!("Expected FuzzyFind modal"),
        }
    }

    #[test]
    fn modal_move_down_and_up() {
        let mut state = AppState::new(sample_graph());
        dispatch(&mut state, Action::OpenFuzzyFind);
        dispatch(&mut state, Action::ModalMoveDown);
        match &state.modal {
            Some(Modal::FuzzyFind { cursor, .. }) => assert_eq!(*cursor, 1),
            _ => panic!("Expected FuzzyFind modal"),
        }
        dispatch(&mut state, Action::ModalMoveUp);
        match &state.modal {
            Some(Modal::FuzzyFind { cursor, .. }) => assert_eq!(*cursor, 0),
            _ => panic!("Expected FuzzyFind modal"),
        }
    }

    #[test]
    fn fuzzy_input_and_backspace() {
        let mut state = AppState::new(sample_graph());
        dispatch(&mut state, Action::OpenFuzzyFind);
        dispatch(&mut state, Action::FuzzyInput('a'));
        dispatch(&mut state, Action::FuzzyInput('b'));
        match &state.modal {
            Some(Modal::FuzzyFind { query, .. }) => assert_eq!(query, "ab"),
            _ => panic!("Expected FuzzyFind modal"),
        }
        dispatch(&mut state, Action::FuzzyBackspace);
        match &state.modal {
            Some(Modal::FuzzyFind { query, .. }) => assert_eq!(query, "a"),
            _ => panic!("Expected FuzzyFind modal"),
        }
    }

    #[test]
    fn modal_enter_on_fuzzy_find_jumps_cursor() {
        let mut state = AppState::new(sample_graph());
        dispatch(&mut state, Action::OpenFuzzyFind);
        dispatch(&mut state, Action::ModalMoveDown);
        dispatch(&mut state, Action::ModalEnter);
        assert!(state.modal.is_none());
        assert_eq!(state.cursor(), 2);
    }

    #[test]
    fn bookmark_enter_jumps_cursor() {
        let mut state = AppState::new(sample_graph_with_bookmarks());
        dispatch(&mut state, Action::OpenBookmarks);
        assert!(matches!(state.modal, Some(Modal::BookmarkPicker { .. })));

        if let Some(Modal::BookmarkPicker { ref bookmarks, .. }) = state.modal {
            assert!(!bookmarks.is_empty());
        }

        dispatch(&mut state, Action::ModalMoveDown);
        dispatch(&mut state, Action::ModalEnter);
        assert!(state.modal.is_none());
        assert_eq!(state.cursor(), 2);
        assert_eq!(state.selected_change_id(), Some("def"));
    }

    #[test]
    fn fuzzy_input_narrows_matches() {
        let mut state = AppState::new(sample_graph());
        dispatch(&mut state, Action::OpenFuzzyFind);

        let initial_count = match &state.modal {
            Some(Modal::FuzzyFind { matches, .. }) => matches.len(),
            _ => panic!("Expected FuzzyFind"),
        };

        dispatch(&mut state, Action::FuzzyInput('d'));
        dispatch(&mut state, Action::FuzzyInput('e'));
        dispatch(&mut state, Action::FuzzyInput('s'));
        dispatch(&mut state, Action::FuzzyInput('c'));

        match &state.modal {
            Some(Modal::FuzzyFind { matches, .. }) => {
                assert!(matches.len() <= initial_count);
            }
            _ => panic!("Expected FuzzyFind"),
        }
    }

    #[test]
    fn help_scroll_clamped_to_content() {
        let mut state = AppState::new(sample_graph());
        state.focus = PanelFocus::Detail;
        state.detail_mode = DetailMode::DiffView;
        dispatch(&mut state, Action::OpenHelp);

        for _ in 0..20 {
            dispatch(&mut state, Action::ModalMoveDown);
        }
        match &state.modal {
            Some(Modal::Help { scroll, context }) => {
                assert!(
                    *scroll < context.line_count(),
                    "scroll {} should be < {}",
                    scroll,
                    context.line_count()
                );
            }
            _ => panic!("Expected Help modal"),
        }
    }

    #[test]
    fn modal_enter_on_help_keeps_modal() {
        let mut state = AppState::new(sample_graph());
        dispatch(&mut state, Action::OpenHelp);
        dispatch(&mut state, Action::ModalEnter);
        assert!(matches!(state.modal, Some(Modal::Help { .. })));
    }
}
