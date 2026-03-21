use lajjzy_core::types::{ChangeDetail, DiffHunk, GraphData};

pub use crate::action::{Action, DetailMode, PanelFocus};
pub use crate::modal::{HelpContext, Modal};

pub struct AppState {
    pub graph: GraphData,
    pub(crate) cursor: usize,
    pub should_quit: bool,
    pub error: Option<String>,
    pub focus: PanelFocus,
    pub(crate) detail_cursor: usize,
    pub detail_mode: DetailMode,
    pub diff_scroll: usize,
    pub diff_data: Vec<DiffHunk>,
    pub modal: Option<Modal>,
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
            modal: None,
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

#[cfg(test)]
impl AppState {
    pub fn set_cursor_for_test(&mut self, index: usize) {
        self.cursor = index;
    }

    pub fn set_detail_cursor_for_test(&mut self, index: usize) {
        self.detail_cursor = index;
    }
}
