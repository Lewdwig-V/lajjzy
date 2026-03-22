use std::collections::HashSet;

use lajjzy_core::types::{
    ChangeDetail, ConflictData, DiffHunk, DiffLine, GraphData, HunkResolution,
};

pub use crate::action::{Action, BackgroundKind, DetailMode, MutationKind, PanelFocus};
use crate::action::{HunkPickerOp, RebaseMode};
pub use crate::modal::{HelpContext, Modal};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickingMode {
    Browsing,
    Filtering { query: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TargetPick {
    pub source: String,
    pub mode: RebaseMode,
    pub excluded: HashSet<String>,
    pub picking: PickingMode,
    /// Change ID at cursor when picking started — restored by identity on cancel.
    /// Using ID rather than index survives graph refreshes that shift positions.
    pub original_change_id: String,
    pub descendant_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HunkPicker {
    pub operation: HunkPickerOp,
    pub files: Vec<PickerFile>,
    pub cursor: usize,
    pub scroll: usize,
    /// Viewport height for scroll computation. Set by the event loop
    /// before dispatching, so dispatch can adjust scroll after cursor movement.
    pub viewport_height: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PickerFile {
    pub path: String,
    pub hunks: Vec<PickerHunk>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PickerHunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConflictView {
    pub change_id: String,
    pub path: String,
    pub data: ConflictData,
    /// Per conflict hunk resolution state. Parallel to Conflict variants only.
    pub resolutions: Vec<HunkResolution>,
    /// Cursor indexes into conflict hunks only (0..N).
    pub cursor: usize,
    pub scroll: usize,
    pub viewport_height: usize,
}

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
    pub(crate) pending_mutation: Option<MutationKind>,
    pub(crate) pending_background: HashSet<BackgroundKind>,
    pub status_message: Option<String>,
    pub(crate) cursor_follows_working_copy: bool,
    /// Monotonic counter for graph snapshot versioning.
    /// Dispatch rejects `GraphLoaded` with generation < this value.
    pub(crate) graph_generation: u64,
    /// The currently active revset filter, or `None` for the default revset.
    pub active_revset: Option<String>,
    /// Saved cursor position for restoring focus when exiting a revset filter.
    pub(crate) omnibar_fallback_idx: Option<usize>,
    pub target_pick: Option<TargetPick>,
    pub hunk_picker: Option<HunkPicker>,
    pub conflict_view: Option<ConflictView>,
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
            pending_mutation: None,
            pending_background: HashSet::new(),
            status_message: None,
            cursor_follows_working_copy: false,
            graph_generation: 0,
            active_revset: None,
            omnibar_fallback_idx: None,
            target_pick: None,
            hunk_picker: None,
            conflict_view: None,
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
