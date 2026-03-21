use lajjzy_core::types::GraphData;
use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::action::{Action, BackgroundKind, DetailMode, MutationKind, PanelFocus};
use crate::app::AppState;
use crate::effect::Effect;
use crate::modal::{HelpContext, Modal};

/// Clear the appropriate concurrency gate for a completed operation.
/// Uses exhaustive matching — adding a new `MutationKind` variant is a compile error.
fn clear_op_gate(state: &mut AppState, op: MutationKind) {
    match op {
        MutationKind::GitPush => {
            state.pending_background.remove(&BackgroundKind::Push);
        }
        MutationKind::GitFetch => {
            state.pending_background.remove(&BackgroundKind::Fetch);
        }
        MutationKind::Describe
        | MutationKind::New
        | MutationKind::Edit
        | MutationKind::Abandon
        | MutationKind::Squash
        | MutationKind::Undo
        | MutationKind::Redo
        | MutationKind::BookmarkSet
        | MutationKind::BookmarkDelete
        | MutationKind::RebaseSingle
        | MutationKind::RebaseWithDescendants => {
            state.pending_mutation = None;
        }
    }
}

#[expect(clippy::too_many_lines)]
pub fn dispatch(state: &mut AppState, action: Action) -> Vec<Effect> {
    // Clear stale omnibar fallback on any action except RevsetLoaded.
    // Prevents a slow EvalRevset error from jumping the cursor after the
    // user has already navigated elsewhere.
    if !matches!(action, Action::RevsetLoaded { .. }) {
        state.omnibar_fallback_idx = None;
    }

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
            return vec![Effect::LoadGraph {
                revset: state.active_revset.clone(),
            }];
        }
        Action::GraphLoaded { generation, result } => {
            // Reject stale snapshots from concurrent loads
            if generation < state.graph_generation {
                return vec![];
            }
            state.graph_generation = generation;
            match result {
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
                                    .find(|&&i| {
                                        state.graph.lines[i].change_id.as_deref() == Some(id)
                                    })
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
        Action::OpenOmnibar => {
            let query = state.active_revset.clone().unwrap_or_default();
            let matches = if query.is_empty() {
                state.graph.node_indices().to_vec()
            } else {
                fuzzy_match(&query, &state.graph)
            };
            state.modal = Some(Modal::Omnibar {
                query,
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
            // Clear omnibar fallback to prevent stale cursor jumps
            // from in-flight EvalRevset results arriving after dismiss.
            if matches!(state.modal, Some(Modal::Omnibar { .. })) {
                state.omnibar_fallback_idx = None;
            }
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
                    Modal::Omnibar {
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
                    | Modal::Omnibar { cursor, .. } => {
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
                Some(Modal::Omnibar {
                    query,
                    matches,
                    cursor,
                }) => {
                    if query.is_empty() {
                        if state.active_revset.is_some() {
                            state.active_revset = None;
                            return vec![Effect::LoadGraph { revset: None }];
                        }
                        // No active revset + empty query: just close (modal already taken)
                    } else {
                        // Non-empty: store fuzzy fallback and try as revset
                        state.omnibar_fallback_idx = matches.get(cursor).copied();
                        state.status_message = Some("Evaluating revset\u{2026}".into());
                        return vec![Effect::EvalRevset { query }];
                    }
                }
                other => {
                    state.modal = other;
                }
            }
        }
        Action::OmnibarInput(c) => {
            if let Some(Modal::Omnibar {
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
        Action::OmnibarBackspace => {
            if let Some(Modal::Omnibar {
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
        Action::Abandon => {
            if state.pending_mutation.is_some() {
                state.status_message = Some("Operation in progress\u{2026}".into());
                return vec![];
            }
            if let Some(cid) = state.selected_change_id().map(String::from) {
                state.pending_mutation = Some(MutationKind::Abandon);
                return vec![Effect::Abandon { change_id: cid }];
            }
        }
        Action::Squash => {
            if state.pending_mutation.is_some() {
                state.status_message = Some("Operation in progress\u{2026}".into());
                return vec![];
            }
            if let Some(cid) = state.selected_change_id().map(String::from) {
                state.pending_mutation = Some(MutationKind::Squash);
                return vec![Effect::Squash { change_id: cid }];
            }
        }
        Action::NewChange => {
            if state.pending_mutation.is_some() {
                state.status_message = Some("Operation in progress\u{2026}".into());
                return vec![];
            }
            if let Some(cid) = state.selected_change_id().map(String::from) {
                state.pending_mutation = Some(MutationKind::New);
                state.cursor_follows_working_copy = true;
                return vec![Effect::New { after: cid }];
            }
        }
        Action::EditChange => {
            if state.pending_mutation.is_some() {
                state.status_message = Some("Operation in progress\u{2026}".into());
                return vec![];
            }
            if let Some(cid) = state.selected_change_id().map(String::from) {
                state.pending_mutation = Some(MutationKind::Edit);
                return vec![Effect::Edit { change_id: cid }];
            }
        }
        Action::Undo => {
            if state.pending_mutation.is_some() {
                state.status_message = Some("Operation in progress\u{2026}".into());
                return vec![];
            }
            state.pending_mutation = Some(MutationKind::Undo);
            return vec![Effect::Undo];
        }
        Action::Redo => {
            if state.pending_mutation.is_some() {
                state.status_message = Some("Operation in progress\u{2026}".into());
                return vec![];
            }
            state.pending_mutation = Some(MutationKind::Redo);
            return vec![Effect::Redo];
        }
        Action::RepoOpSuccess { op, message, graph } => {
            // Install refreshed graph BEFORE clearing the gate so no mutation
            // can fire against stale state between gate-clear and graph-replace.
            //
            // Mutation results bypass the generation check — they are always authoritative
            // because load_graph() ran AFTER the mutation committed. A concurrent Refresh
            // with a higher generation is genuinely stale relative to the mutation result.
            if let Some((generation, graph_result)) = graph {
                // Force-accept by ensuring generation >= current
                state.graph_generation = generation;
                let nested = dispatch(
                    state,
                    Action::GraphLoaded {
                        generation,
                        result: graph_result,
                    },
                );
                debug_assert!(nested.is_empty(), "GraphLoaded should not produce effects");
            }
            clear_op_gate(state, op);
            // Only show success if graph load didn't set an error
            if state.error.is_none() {
                state.status_message = Some(message);
            }
        }
        Action::RepoOpFailed { op, error } => {
            clear_op_gate(state, op);
            state.error = Some(error);
        }
        Action::GitPush => {
            if state.pending_background.contains(&BackgroundKind::Push) {
                return vec![];
            }
            let bookmark = state
                .selected_detail()
                .and_then(|d| d.bookmarks.first())
                .cloned();
            match bookmark {
                Some(b) => {
                    state.pending_background.insert(BackgroundKind::Push);
                    return vec![Effect::GitPush { bookmark: b }];
                }
                None => {
                    if state.selected_detail().is_some() {
                        state.error = Some("No bookmark on selected change".into());
                    }
                }
            }
        }
        Action::GitFetch => {
            if state.pending_background.contains(&BackgroundKind::Fetch) {
                return vec![];
            }
            state.pending_background.insert(BackgroundKind::Fetch);
            return vec![Effect::GitFetch];
        }
        Action::OpenDescribe => {
            if state.pending_mutation.is_some() {
                state.status_message = Some("Operation in progress\u{2026}".into());
                return vec![];
            }
            if let Some(cid) = state.selected_change_id().map(String::from) {
                let text = state
                    .selected_detail()
                    .map(|d| d.description.clone())
                    .unwrap_or_default();
                let lines: Vec<String> = if text.is_empty() {
                    vec![String::new()]
                } else {
                    text.lines().map(String::from).collect()
                };
                let editor = Box::new(tui_textarea::TextArea::new(lines));
                state.modal = Some(Modal::Describe {
                    change_id: cid,
                    editor,
                });
            }
        }
        Action::EditorComplete { change_id, text } => {
            if state.pending_mutation.is_some() {
                return vec![];
            }
            state.pending_mutation = Some(MutationKind::Describe);
            return vec![Effect::Describe { change_id, text }];
        }
        Action::DescribeSave => {
            if state.pending_mutation.is_some() {
                return vec![];
            }
            if let Some(Modal::Describe { change_id, editor }) = state.modal.take() {
                let text = editor.lines().join("\n");
                state.pending_mutation = Some(MutationKind::Describe);
                return vec![Effect::Describe { change_id, text }];
            }
        }
        Action::DescribeEscalateEditor => {
            if let Some(Modal::Describe { change_id, editor }) = state.modal.take() {
                let text = editor.lines().join("\n");
                return vec![Effect::SuspendForEditor {
                    change_id,
                    initial_text: text,
                }];
            }
        }
        Action::RevsetLoaded {
            query,
            generation,
            result,
        } => {
            // Reject stale revset results
            if generation < state.graph_generation {
                return vec![];
            }

            match result {
                Ok(new_graph) => {
                    state.omnibar_fallback_idx = None;
                    if new_graph.node_indices().is_empty() {
                        // Maintain staleness invariant even for empty results
                        state.graph_generation = generation;
                        state.status_message = Some(format!("No changes match: {query}"));
                    } else {
                        let count = new_graph.node_indices().len();
                        state.active_revset = Some(query);
                        state.status_message = Some(format!(
                            "{count} change{} matched",
                            if count == 1 { "" } else { "s" }
                        ));
                        let nested = dispatch(
                            state,
                            Action::GraphLoaded {
                                generation,
                                result: Ok(new_graph),
                            },
                        );
                        assert!(
                            nested.is_empty(),
                            "RevsetLoaded: nested GraphLoaded must not produce effects"
                        );
                    }
                }
                Err(err_msg) => {
                    // Show the revset error so the user knows why it failed
                    state.status_message = Some(format!("Invalid revset: {err_msg}"));
                    // Fall back to fuzzy jump if available
                    if let Some(idx) = state.omnibar_fallback_idx.take() {
                        state.cursor = idx;
                        state.reset_detail();
                    }
                }
            }
        }
        Action::OpenBookmarkSet => {
            if state.pending_mutation.is_some() {
                state.status_message = Some("Operation in progress\u{2026}".into());
                return vec![];
            }
            if let Some(cid) = state.selected_change_id().map(String::from) {
                let existing = state
                    .selected_detail()
                    .and_then(|d| d.bookmarks.first().cloned())
                    .unwrap_or_default();
                let all_bookmarks: Vec<String> = state
                    .graph
                    .details
                    .values()
                    .flat_map(|d| d.bookmarks.iter().cloned())
                    .collect();
                state.modal = Some(Modal::BookmarkInput {
                    change_id: cid,
                    input: existing,
                    completions: all_bookmarks,
                    cursor: 0,
                });
            }
        }
        Action::BookmarkInputChar(c) => {
            if let Some(Modal::BookmarkInput { input, .. }) = &mut state.modal {
                input.push(c);
            }
        }
        Action::BookmarkInputBackspace => {
            if let Some(Modal::BookmarkInput { input, .. }) = &mut state.modal {
                input.pop();
            }
        }
        Action::BookmarkInputConfirm => {
            if state.pending_mutation.is_some() {
                return vec![];
            }
            if let Some(Modal::BookmarkInput {
                change_id, input, ..
            }) = state.modal.take()
                && !input.is_empty()
            {
                state.pending_mutation = Some(MutationKind::BookmarkSet);
                return vec![Effect::BookmarkSet {
                    change_id,
                    name: input,
                }];
            }
        }
        Action::BookmarkDelete => {
            if state.pending_mutation.is_some() {
                state.status_message = Some("Operation in progress\u{2026}".into());
                return vec![];
            }
            if let Some(Modal::BookmarkPicker {
                ref bookmarks,
                cursor,
                ..
            }) = state.modal
                && let Some((name, _)) = bookmarks.get(cursor)
            {
                let name = name.clone();
                state.modal = None;
                state.pending_mutation = Some(MutationKind::BookmarkDelete);
                return vec![Effect::BookmarkDelete { name }];
            }
        }
        // Placeholder arms — behavior implemented in Task 5
        Action::RebaseSingle
        | Action::RebaseWithDescendants
        | Action::PickConfirm
        | Action::PickCancel
        | Action::PickFilterChar(_)
        | Action::PickFilterBackspace => {}
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
                        has_conflict: false,
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
                        has_conflict: false,
                        files: vec![],
                        parents: vec![],
                    },
                ),
            ]),
            Some(0),
            String::new(),
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
                        has_conflict: false,
                        files: vec![],
                        parents: vec![],
                    },
                ),
            ]),
            Some(0),
            String::new(),
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
                        bookmarks: vec!["feature".into()],
                        is_empty: false,
                        has_conflict: false,
                        files: vec![],
                        parents: vec![],
                    },
                ),
            ]),
            Some(0),
            String::new(),
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
                        parents: vec![],
                    },
                )
            })
            .collect();
        GraphData::new(lines, details, Some(0), String::new())
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
        let effects = dispatch(
            &mut state,
            Action::GraphLoaded {
                generation: 1,
                result: Ok(new_graph),
            },
        );
        assert!(effects.is_empty());
        assert_eq!(state.graph.lines.len(), 2);
        assert_eq!(state.selected_change_id(), Some("xxx"));
    }

    #[test]
    fn graph_loaded_error_sets_error() {
        let mut state = AppState::new(sample_graph());
        let effects = dispatch(
            &mut state,
            Action::GraphLoaded {
                generation: 1,
                result: Err("boom".into()),
            },
        );
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
        let effects = dispatch(
            &mut state,
            Action::GraphLoaded {
                generation: 1,
                result: Ok(new_graph),
            },
        );
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
        let new_graph = GraphData::new(lines, details, sg.working_copy_index, String::new());

        let effects = dispatch(
            &mut state,
            Action::GraphLoaded {
                generation: 1,
                result: Ok(new_graph),
            },
        );
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
        let effects = dispatch(
            &mut state,
            Action::GraphLoaded {
                generation: 1,
                result: Ok(new_graph),
            },
        );
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
        let effects = dispatch(
            &mut state,
            Action::GraphLoaded {
                generation: 1,
                result: Ok(new_graph),
            },
        );
        assert!(effects.is_empty());
        assert_eq!(state.detail_mode, DetailMode::FileList);
        assert_eq!(state.diff_scroll, 0);
        assert_eq!(state.detail_cursor(), 0);
    }

    #[test]
    fn stale_graph_loaded_rejected() {
        let mut state = AppState::new(sample_graph());
        // Accept a fresh graph at generation 2
        let new_graph = test_graph_with_changes(&["xxx"]);
        dispatch(
            &mut state,
            Action::GraphLoaded {
                generation: 2,
                result: Ok(new_graph),
            },
        );
        assert_eq!(state.graph_generation, 2);
        assert_eq!(state.selected_change_id(), Some("xxx"));

        // A stale graph at generation 1 arrives later — must be rejected
        let stale_graph = test_graph_with_changes(&["stale"]);
        dispatch(
            &mut state,
            Action::GraphLoaded {
                generation: 1,
                result: Ok(stale_graph),
            },
        );
        // Graph unchanged — stale snapshot was dropped
        assert_eq!(state.graph_generation, 2);
        assert_eq!(state.selected_change_id(), Some("xxx"));
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
    fn omnibar_opens_with_all_matches() {
        let mut state = AppState::new(sample_graph());
        dispatch(&mut state, Action::OpenOmnibar);
        match &state.modal {
            Some(Modal::Omnibar { matches, query, .. }) => {
                assert!(query.is_empty());
                assert_eq!(matches.len(), state.graph.node_indices().len());
            }
            _ => panic!("Expected Omnibar modal"),
        }
    }

    #[test]
    fn modal_move_down_and_up() {
        let mut state = AppState::new(sample_graph());
        dispatch(&mut state, Action::OpenOmnibar);
        dispatch(&mut state, Action::ModalMoveDown);
        match &state.modal {
            Some(Modal::Omnibar { cursor, .. }) => assert_eq!(*cursor, 1),
            _ => panic!("Expected Omnibar modal"),
        }
        dispatch(&mut state, Action::ModalMoveUp);
        match &state.modal {
            Some(Modal::Omnibar { cursor, .. }) => assert_eq!(*cursor, 0),
            _ => panic!("Expected Omnibar modal"),
        }
    }

    #[test]
    fn omnibar_input_and_backspace() {
        let mut state = AppState::new(sample_graph());
        dispatch(&mut state, Action::OpenOmnibar);
        dispatch(&mut state, Action::OmnibarInput('a'));
        dispatch(&mut state, Action::OmnibarInput('b'));
        match &state.modal {
            Some(Modal::Omnibar { query, .. }) => assert_eq!(query, "ab"),
            _ => panic!("Expected Omnibar modal"),
        }
        dispatch(&mut state, Action::OmnibarBackspace);
        match &state.modal {
            Some(Modal::Omnibar { query, .. }) => assert_eq!(query, "a"),
            _ => panic!("Expected Omnibar modal"),
        }
    }

    #[test]
    fn modal_enter_on_omnibar_empty_closes() {
        let mut state = AppState::new(sample_graph());
        dispatch(&mut state, Action::OpenOmnibar);
        dispatch(&mut state, Action::ModalMoveDown);
        let effects = dispatch(&mut state, Action::ModalEnter);
        assert!(state.modal.is_none());
        assert!(effects.is_empty());
        // Cursor unchanged — empty query without active revset just closes
        assert_eq!(state.cursor(), 0);
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
    fn omnibar_input_narrows_matches() {
        let mut state = AppState::new(sample_graph());
        dispatch(&mut state, Action::OpenOmnibar);

        let initial_count = match &state.modal {
            Some(Modal::Omnibar { matches, .. }) => matches.len(),
            _ => panic!("Expected Omnibar"),
        };

        dispatch(&mut state, Action::OmnibarInput('d'));
        dispatch(&mut state, Action::OmnibarInput('e'));
        dispatch(&mut state, Action::OmnibarInput('s'));
        dispatch(&mut state, Action::OmnibarInput('c'));

        match &state.modal {
            Some(Modal::Omnibar { matches, .. }) => {
                assert!(matches.len() <= initial_count);
            }
            _ => panic!("Expected Omnibar"),
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

    // --- Instant mutation dispatch tests ---

    #[test]
    fn abandon_emits_effect_and_sets_gate() {
        let mut state = AppState::new(sample_graph());
        assert_eq!(state.selected_change_id(), Some("abc"));
        let effects = dispatch(&mut state, Action::Abandon);
        assert_eq!(
            effects,
            vec![Effect::Abandon {
                change_id: "abc".into()
            }]
        );
        assert_eq!(state.pending_mutation, Some(MutationKind::Abandon));
    }

    #[test]
    fn squash_emits_effect_and_sets_gate() {
        let mut state = AppState::new(sample_graph());
        let effects = dispatch(&mut state, Action::Squash);
        assert_eq!(
            effects,
            vec![Effect::Squash {
                change_id: "abc".into()
            }]
        );
        assert_eq!(state.pending_mutation, Some(MutationKind::Squash));
    }

    #[test]
    fn edit_change_emits_effect_and_sets_gate() {
        let mut state = AppState::new(sample_graph());
        let effects = dispatch(&mut state, Action::EditChange);
        assert_eq!(
            effects,
            vec![Effect::Edit {
                change_id: "abc".into()
            }]
        );
        assert_eq!(state.pending_mutation, Some(MutationKind::Edit));
    }

    #[test]
    fn undo_emits_effect_and_sets_gate() {
        let mut state = AppState::new(sample_graph());
        let effects = dispatch(&mut state, Action::Undo);
        assert_eq!(effects, vec![Effect::Undo]);
        assert_eq!(state.pending_mutation, Some(MutationKind::Undo));
    }

    #[test]
    fn redo_emits_effect_and_sets_gate() {
        let mut state = AppState::new(sample_graph());
        let effects = dispatch(&mut state, Action::Redo);
        assert_eq!(effects, vec![Effect::Redo]);
        assert_eq!(state.pending_mutation, Some(MutationKind::Redo));
    }

    #[test]
    fn new_change_sets_cursor_follows_flag() {
        let mut state = AppState::new(sample_graph());
        assert!(!state.cursor_follows_working_copy);
        let effects = dispatch(&mut state, Action::NewChange);
        assert_eq!(
            effects,
            vec![Effect::New {
                after: "abc".into()
            }]
        );
        assert_eq!(state.pending_mutation, Some(MutationKind::New));
        assert!(state.cursor_follows_working_copy);
    }

    #[test]
    fn mutation_suppressed_while_pending() {
        let mut state = AppState::new(sample_graph());
        // Set the gate manually as if a prior mutation is in flight
        state.pending_mutation = Some(MutationKind::Abandon);
        let effects = dispatch(&mut state, Action::Squash);
        assert!(effects.is_empty());
        // Gate must still be the original Abandon — not overwritten
        assert_eq!(state.pending_mutation, Some(MutationKind::Abandon));
    }

    #[test]
    fn undo_suppressed_while_pending() {
        let mut state = AppState::new(sample_graph());
        state.pending_mutation = Some(MutationKind::Edit);
        let effects = dispatch(&mut state, Action::Undo);
        assert!(effects.is_empty());
        assert_eq!(state.pending_mutation, Some(MutationKind::Edit));
    }

    #[test]
    fn repo_op_success_clears_gate_and_sets_status() {
        let mut state = AppState::new(sample_graph());
        state.pending_mutation = Some(MutationKind::Abandon);
        let effects = dispatch(
            &mut state,
            Action::RepoOpSuccess {
                op: MutationKind::Abandon,
                message: "abandoned".into(),
                graph: Some((1, Ok(sample_graph()))),
            },
        );
        assert!(effects.is_empty());
        assert!(state.pending_mutation.is_none());
        assert_eq!(state.status_message.as_deref(), Some("abandoned"));
    }

    #[test]
    fn repo_op_success_without_graph_still_clears_gate() {
        let mut state = AppState::new(sample_graph());
        state.pending_mutation = Some(MutationKind::Abandon);
        let effects = dispatch(
            &mut state,
            Action::RepoOpSuccess {
                op: MutationKind::Abandon,
                message: "abandoned".into(),
                graph: None,
            },
        );
        assert!(effects.is_empty());
        assert!(state.pending_mutation.is_none());
    }

    #[test]
    fn repo_op_success_installs_graph_before_clearing_gate() {
        // Verifies the ordering: graph replaced, THEN gate cleared.
        // If gate cleared first, a fast mutation could fire against stale graph.
        let mut state = AppState::new(sample_graph());
        state.pending_mutation = Some(MutationKind::Abandon);

        let new_graph = test_graph_with_changes(&["xxx"]);
        dispatch(
            &mut state,
            Action::RepoOpSuccess {
                op: MutationKind::Abandon,
                message: "abandoned".into(),
                graph: Some((1, Ok(new_graph))),
            },
        );

        // Gate cleared AND graph replaced atomically
        assert!(state.pending_mutation.is_none());
        assert_eq!(state.graph.node_indices().len(), 1);
        assert_eq!(state.selected_change_id(), Some("xxx"));
    }

    #[test]
    fn repo_op_failed_clears_gate_and_sets_error() {
        let mut state = AppState::new(sample_graph());
        state.pending_mutation = Some(MutationKind::Squash);
        let effects = dispatch(
            &mut state,
            Action::RepoOpFailed {
                op: MutationKind::Squash,
                error: "squash failed".into(),
            },
        );
        assert!(effects.is_empty());
        assert!(state.pending_mutation.is_none());
        assert_eq!(state.error.as_deref(), Some("squash failed"));
    }

    #[test]
    fn repo_op_success_push_clears_background_not_gate() {
        use crate::action::BackgroundKind;
        let mut state = AppState::new(sample_graph());
        state.pending_background.insert(BackgroundKind::Push);
        state.pending_mutation = Some(MutationKind::Abandon); // should be untouched
        dispatch(
            &mut state,
            Action::RepoOpSuccess {
                op: MutationKind::GitPush,
                message: "pushed".into(),
                graph: Some((1, Ok(sample_graph()))),
            },
        );
        assert!(!state.pending_background.contains(&BackgroundKind::Push));
        // pending_mutation preserved — push doesn't clear it
        assert_eq!(state.pending_mutation, Some(MutationKind::Abandon));
    }

    #[test]
    fn repo_op_failed_fetch_clears_background_not_gate() {
        use crate::action::BackgroundKind;
        let mut state = AppState::new(sample_graph());
        state.pending_background.insert(BackgroundKind::Fetch);
        state.pending_mutation = Some(MutationKind::Edit);
        dispatch(
            &mut state,
            Action::RepoOpFailed {
                op: MutationKind::GitFetch,
                error: "fetch failed".into(),
            },
        );
        assert!(!state.pending_background.contains(&BackgroundKind::Fetch));
        assert_eq!(state.pending_mutation, Some(MutationKind::Edit));
    }

    #[test]
    fn navigation_unaffected_by_pending_mutation() {
        let mut state = AppState::new(sample_graph());
        state.pending_mutation = Some(MutationKind::Abandon);
        // Navigation should work normally even with gate set
        let effects = dispatch(&mut state, Action::MoveDown);
        assert!(effects.is_empty());
        assert_eq!(state.cursor(), 2);
        assert_eq!(state.selected_change_id(), Some("def"));
        // Gate untouched
        assert_eq!(state.pending_mutation, Some(MutationKind::Abandon));
    }

    // --- Background mutation (push/fetch) tests ---

    fn sample_graph_bookmarked() -> GraphData {
        GraphData::new(
            vec![GraphLine {
                raw: "◉  abc".into(),
                change_id: Some("abc".into()),
                glyph_prefix: String::new(),
            }],
            HashMap::from([(
                "abc".into(),
                ChangeDetail {
                    commit_id: "a1".into(),
                    author: "alice".into(),
                    email: "alice@example.com".into(),
                    timestamp: "1m".into(),
                    description: "feat: add thing".into(),
                    bookmarks: vec!["main".into()],
                    is_empty: false,
                    has_conflict: false,
                    files: vec![],
                    parents: vec![],
                },
            )]),
            Some(0),
            String::new(),
        )
    }

    #[test]
    fn push_uses_background_gate() {
        let mut state = AppState::new(sample_graph_bookmarked());
        let effects = dispatch(&mut state, Action::GitPush);
        assert_eq!(
            effects,
            vec![Effect::GitPush {
                bookmark: "main".into()
            }]
        );
        assert!(state.pending_background.contains(&BackgroundKind::Push));
        // pending_mutation lane untouched
        assert!(state.pending_mutation.is_none());
    }

    #[test]
    fn push_suppressed_while_pushing() {
        let mut state = AppState::new(sample_graph_bookmarked());
        state.pending_background.insert(BackgroundKind::Push);
        let effects = dispatch(&mut state, Action::GitPush);
        assert!(effects.is_empty());
        // Gate still set, nothing changed
        assert!(state.pending_background.contains(&BackgroundKind::Push));
    }

    #[test]
    fn push_no_bookmark_sets_error() {
        // sample_graph has no bookmarks on "abc"
        let mut state = AppState::new(sample_graph());
        let effects = dispatch(&mut state, Action::GitPush);
        assert!(effects.is_empty());
        assert!(state.error.as_deref().unwrap().contains("No bookmark"));
        assert!(!state.pending_background.contains(&BackgroundKind::Push));
    }

    #[test]
    fn fetch_uses_background_gate() {
        let mut state = AppState::new(sample_graph());
        let effects = dispatch(&mut state, Action::GitFetch);
        assert_eq!(effects, vec![Effect::GitFetch]);
        assert!(state.pending_background.contains(&BackgroundKind::Fetch));
        assert!(state.pending_mutation.is_none());
    }

    #[test]
    fn fetch_suppressed_while_fetching() {
        let mut state = AppState::new(sample_graph());
        state.pending_background.insert(BackgroundKind::Fetch);
        let effects = dispatch(&mut state, Action::GitFetch);
        assert!(effects.is_empty());
        assert!(state.pending_background.contains(&BackgroundKind::Fetch));
    }

    #[test]
    fn fetch_concurrent_with_local_mutation() {
        // A local mutation is in flight; fetch must still proceed on its independent lane.
        let mut state = AppState::new(sample_graph());
        state.pending_mutation = Some(MutationKind::Abandon);
        let effects = dispatch(&mut state, Action::GitFetch);
        assert_eq!(effects, vec![Effect::GitFetch]);
        assert!(state.pending_background.contains(&BackgroundKind::Fetch));
        // Local mutation gate untouched
        assert_eq!(state.pending_mutation, Some(MutationKind::Abandon));
    }

    #[test]
    fn push_concurrent_with_fetch() {
        // Fetch is already in flight; push must still proceed independently.
        let mut state = AppState::new(sample_graph_bookmarked());
        state.pending_background.insert(BackgroundKind::Fetch);
        let effects = dispatch(&mut state, Action::GitPush);
        assert_eq!(
            effects,
            vec![Effect::GitPush {
                bookmark: "main".into()
            }]
        );
        assert!(state.pending_background.contains(&BackgroundKind::Push));
        // Fetch gate untouched
        assert!(state.pending_background.contains(&BackgroundKind::Fetch));
    }

    // --- OpenDescribe / EditorComplete tests ---

    #[test]
    fn open_describe_opens_modal() {
        let mut state = AppState::new(sample_graph());
        // Cursor starts at "abc" (working copy) with description "desc1"
        assert_eq!(state.selected_change_id(), Some("abc"));
        let effects = dispatch(&mut state, Action::OpenDescribe);
        assert!(effects.is_empty()); // no effect — opens modal
        assert!(matches!(state.modal, Some(Modal::Describe { .. })));
        if let Some(Modal::Describe {
            change_id, editor, ..
        }) = &state.modal
        {
            assert_eq!(change_id, "abc");
            assert_eq!(editor.lines(), &["desc1"]);
        }
        // pending_mutation NOT set — user hasn't saved yet
        assert!(state.pending_mutation.is_none());
    }

    #[test]
    fn open_describe_with_empty_description() {
        use lajjzy_core::types::{ChangeDetail, GraphLine};
        let graph = GraphData::new(
            vec![GraphLine {
                raw: "◉  nodesc".into(),
                change_id: Some("nodesc".into()),
                glyph_prefix: String::new(),
            }],
            HashMap::from([(
                "nodesc".into(),
                ChangeDetail {
                    commit_id: "n1".into(),
                    author: "x".into(),
                    email: "x@y".into(),
                    timestamp: "0m".into(),
                    description: String::new(),
                    bookmarks: vec![],
                    is_empty: true,
                    has_conflict: false,
                    files: vec![],
                    parents: vec![],
                },
            )]),
            Some(0),
            String::new(),
        );
        let mut state = AppState::new(graph);
        let effects = dispatch(&mut state, Action::OpenDescribe);
        assert!(effects.is_empty());
        assert!(matches!(state.modal, Some(Modal::Describe { .. })));
        if let Some(Modal::Describe { editor, .. }) = &state.modal {
            assert_eq!(editor.lines(), &[""]);
        }
    }

    #[test]
    fn describe_save_emits_effect_and_closes_modal() {
        let mut state = AppState::new(sample_graph());
        // Open the modal first
        dispatch(&mut state, Action::OpenDescribe);
        assert!(matches!(state.modal, Some(Modal::Describe { .. })));
        // Save
        let effects = dispatch(&mut state, Action::DescribeSave);
        assert_eq!(
            effects,
            vec![Effect::Describe {
                change_id: "abc".into(),
                text: "desc1".into(),
            }]
        );
        assert!(state.modal.is_none());
        assert_eq!(state.pending_mutation, Some(MutationKind::Describe));
    }

    #[test]
    fn describe_escalate_editor_emits_suspend() {
        let mut state = AppState::new(sample_graph());
        // Open the modal first
        dispatch(&mut state, Action::OpenDescribe);
        assert!(matches!(state.modal, Some(Modal::Describe { .. })));
        // Escalate to editor
        let effects = dispatch(&mut state, Action::DescribeEscalateEditor);
        assert_eq!(
            effects,
            vec![Effect::SuspendForEditor {
                change_id: "abc".into(),
                initial_text: "desc1".into(),
            }]
        );
        assert!(state.modal.is_none());
    }

    #[test]
    fn open_describe_suppressed_while_pending() {
        let mut state = AppState::new(sample_graph());
        state.pending_mutation = Some(MutationKind::Abandon);
        let effects = dispatch(&mut state, Action::OpenDescribe);
        assert!(effects.is_empty());
        // Gate remains unchanged
        assert_eq!(state.pending_mutation, Some(MutationKind::Abandon));
    }

    #[test]
    fn editor_complete_emits_describe_effect() {
        let mut state = AppState::new(sample_graph());
        let effects = dispatch(
            &mut state,
            Action::EditorComplete {
                change_id: "abc".into(),
                text: "updated message".into(),
            },
        );
        assert_eq!(
            effects,
            vec![Effect::Describe {
                change_id: "abc".into(),
                text: "updated message".into(),
            }]
        );
        assert_eq!(state.pending_mutation, Some(MutationKind::Describe));
    }

    // --- Bookmark set modal tests ---

    #[test]
    fn open_bookmark_set_opens_modal() {
        let mut state = AppState::new(sample_graph_with_bookmarks());
        // cursor is at "abc" which has bookmark "main"
        let effects = dispatch(&mut state, Action::OpenBookmarkSet);
        assert!(effects.is_empty());
        match &state.modal {
            Some(Modal::BookmarkInput {
                change_id, input, ..
            }) => {
                assert_eq!(change_id, "abc");
                assert_eq!(input, "main"); // pre-filled with existing bookmark
            }
            other => panic!("expected BookmarkInput modal, got {other:?}"),
        }
    }

    #[test]
    fn bookmark_input_char_appends() {
        let mut state = AppState::new(sample_graph());
        state.modal = Some(Modal::BookmarkInput {
            change_id: "abc".into(),
            input: "ma".into(),
            completions: vec![],
            cursor: 0,
        });
        let effects = dispatch(&mut state, Action::BookmarkInputChar('i'));
        assert!(effects.is_empty());
        match &state.modal {
            Some(Modal::BookmarkInput { input, .. }) => assert_eq!(input, "mai"),
            other => panic!("expected BookmarkInput modal, got {other:?}"),
        }
    }

    #[test]
    fn bookmark_input_backspace_removes() {
        let mut state = AppState::new(sample_graph());
        state.modal = Some(Modal::BookmarkInput {
            change_id: "abc".into(),
            input: "main".into(),
            completions: vec![],
            cursor: 0,
        });
        let effects = dispatch(&mut state, Action::BookmarkInputBackspace);
        assert!(effects.is_empty());
        match &state.modal {
            Some(Modal::BookmarkInput { input, .. }) => assert_eq!(input, "mai"),
            other => panic!("expected BookmarkInput modal, got {other:?}"),
        }
    }

    #[test]
    fn bookmark_input_confirm_emits_effect() {
        let mut state = AppState::new(sample_graph());
        state.modal = Some(Modal::BookmarkInput {
            change_id: "abc".into(),
            input: "new-branch".into(),
            completions: vec![],
            cursor: 0,
        });
        let effects = dispatch(&mut state, Action::BookmarkInputConfirm);
        assert_eq!(
            effects,
            vec![Effect::BookmarkSet {
                change_id: "abc".into(),
                name: "new-branch".into(),
            }]
        );
        assert_eq!(state.pending_mutation, Some(MutationKind::BookmarkSet));
        assert!(
            state.modal.is_none(),
            "modal should be closed after confirm"
        );
    }

    #[test]
    fn bookmark_input_confirm_empty_does_nothing() {
        let mut state = AppState::new(sample_graph());
        state.modal = Some(Modal::BookmarkInput {
            change_id: "abc".into(),
            input: String::new(),
            completions: vec![],
            cursor: 0,
        });
        let effects = dispatch(&mut state, Action::BookmarkInputConfirm);
        assert!(effects.is_empty(), "empty input must not emit effect");
        assert!(
            state.pending_mutation.is_none(),
            "no pending mutation on empty confirm"
        );
        // modal is consumed by take() but no effect emitted — that is correct
    }

    #[test]
    fn bookmark_delete_from_picker_emits_effect() {
        let mut state = AppState::new(sample_graph_with_bookmarks());
        state.modal = Some(Modal::BookmarkPicker {
            bookmarks: vec![
                ("main".into(), "abc".into()),
                ("feature".into(), "def".into()),
            ],
            cursor: 0,
        });
        let effects = dispatch(&mut state, Action::BookmarkDelete);
        assert_eq!(
            effects,
            vec![Effect::BookmarkDelete {
                name: "main".into()
            }]
        );
        assert_eq!(state.pending_mutation, Some(MutationKind::BookmarkDelete));
        assert!(state.modal.is_none(), "modal should be closed after delete");
    }

    #[test]
    fn bookmark_delete_suppressed_while_pending() {
        let mut state = AppState::new(sample_graph_with_bookmarks());
        state.pending_mutation = Some(MutationKind::Abandon);
        state.modal = Some(Modal::BookmarkPicker {
            bookmarks: vec![("main".into(), "abc".into())],
            cursor: 0,
        });
        let effects = dispatch(&mut state, Action::BookmarkDelete);
        assert!(
            effects.is_empty(),
            "delete should be suppressed while mutation pending"
        );
        // modal still open, pending unchanged
        assert!(state.modal.is_some());
        assert_eq!(state.pending_mutation, Some(MutationKind::Abandon));
    }

    // --- Omnibar revset dispatch tests ---

    #[test]
    fn open_omnibar_prefills_active_revset() {
        let mut state = AppState::new(sample_graph());
        state.active_revset = Some("mine()".into());
        dispatch(&mut state, Action::OpenOmnibar);
        match &state.modal {
            Some(Modal::Omnibar { query, .. }) => assert_eq!(query, "mine()"),
            _ => panic!("Expected Omnibar modal"),
        }
    }

    #[test]
    fn omnibar_enter_empty_clears_revset() {
        let mut state = AppState::new(sample_graph());
        state.active_revset = Some("mine()".into());
        state.modal = Some(Modal::Omnibar {
            query: String::new(),
            matches: vec![],
            cursor: 0,
        });
        let effects = dispatch(&mut state, Action::ModalEnter);
        assert_eq!(effects, vec![Effect::LoadGraph { revset: None }]);
        assert!(state.active_revset.is_none());
    }

    #[test]
    fn omnibar_enter_empty_no_revset_just_closes() {
        let mut state = AppState::new(sample_graph());
        state.modal = Some(Modal::Omnibar {
            query: String::new(),
            matches: vec![],
            cursor: 0,
        });
        let effects = dispatch(&mut state, Action::ModalEnter);
        assert!(effects.is_empty());
        assert!(state.modal.is_none());
    }

    #[test]
    fn omnibar_enter_nonempty_emits_eval_revset() {
        let mut state = AppState::new(sample_graph());
        let node_idx = state.graph.node_indices()[0];
        state.modal = Some(Modal::Omnibar {
            query: "mine()".into(),
            matches: vec![node_idx],
            cursor: 0,
        });
        let effects = dispatch(&mut state, Action::ModalEnter);
        assert_eq!(
            effects,
            vec![Effect::EvalRevset {
                query: "mine()".into()
            }]
        );
        assert_eq!(state.omnibar_fallback_idx, Some(node_idx));
    }

    #[test]
    fn revset_loaded_success_sets_active_revset() {
        let mut state = AppState::new(sample_graph());
        let filtered = sample_graph();
        let effects = dispatch(
            &mut state,
            Action::RevsetLoaded {
                query: "mine()".into(),
                generation: 1,
                result: Ok(filtered),
            },
        );
        assert!(effects.is_empty());
        assert_eq!(state.active_revset.as_deref(), Some("mine()"));
    }

    #[test]
    fn revset_loaded_empty_graph_shows_feedback() {
        let mut state = AppState::new(sample_graph());
        let empty_graph = GraphData::new(vec![], HashMap::new(), None, String::new());
        let effects = dispatch(
            &mut state,
            Action::RevsetLoaded {
                query: "nobody()".into(),
                generation: 1,
                result: Ok(empty_graph),
            },
        );
        assert!(effects.is_empty());
        assert!(state.active_revset.is_none());
        assert!(
            state
                .status_message
                .as_deref()
                .unwrap()
                .contains("nobody()")
        );
    }

    #[test]
    fn revset_loaded_failure_falls_back_to_fuzzy_jump() {
        let mut state = AppState::new(sample_graph());
        let fallback = state.graph.node_indices()[1];
        state.omnibar_fallback_idx = Some(fallback);
        let effects = dispatch(
            &mut state,
            Action::RevsetLoaded {
                query: "garbage".into(),
                generation: 1,
                result: Err("parse error".into()),
            },
        );
        assert!(effects.is_empty());
        assert_eq!(state.cursor(), fallback);
        assert!(state.omnibar_fallback_idx.is_none());
    }

    #[test]
    fn revset_loaded_stale_generation_rejected() {
        let mut state = AppState::new(sample_graph());
        state.graph_generation = 5;
        let effects = dispatch(
            &mut state,
            Action::RevsetLoaded {
                query: "mine()".into(),
                generation: 3, // older than current
                result: Ok(sample_graph()),
            },
        );
        assert!(effects.is_empty());
        assert!(state.active_revset.is_none()); // not set for stale result
    }

    #[test]
    fn refresh_respects_active_revset() {
        let mut state = AppState::new(sample_graph());
        state.active_revset = Some("mine()".into());
        let effects = dispatch(&mut state, Action::Refresh);
        assert_eq!(
            effects,
            vec![Effect::LoadGraph {
                revset: Some("mine()".into())
            }]
        );
    }
}
