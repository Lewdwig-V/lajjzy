use lajjzy_core::types::{DiffHunk, GraphData, OpLogEntry};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MutationKind {
    Describe,
    New,
    Edit,
    Abandon,
    Squash,
    Undo,
    Redo,
    BookmarkSet,
    BookmarkDelete,
    GitPush,
    GitFetch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackgroundKind {
    Push,
    Fetch,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    // Navigation
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
    ToggleOpLog,
    OpenBookmarks,
    OpenFuzzyFind,
    OpenHelp,
    ModalDismiss,
    ModalMoveUp,
    ModalMoveDown,
    ModalEnter,
    FuzzyInput(char),
    FuzzyBackspace,

    // Effect result actions
    GraphLoaded(Result<GraphData, String>),
    OpLogLoaded(Result<Vec<OpLogEntry>, String>),
    FileDiffLoaded(Result<Vec<DiffHunk>, String>),
    RepoOpSuccess {
        op: MutationKind,
        message: String,
        /// Refreshed graph bundled with success so gate clears atomically
        /// with graph replacement. None only if `load_graph` failed post-mutation.
        graph: Option<Result<GraphData, String>>,
    },
    RepoOpFailed {
        op: MutationKind,
        error: String,
    },
    EditorComplete {
        change_id: String,
        text: String,
    },

    // Mutation trigger actions
    Abandon,
    Squash,
    NewChange,
    EditChange,
    OpenDescribe,
    Undo,
    Redo,
    OpenBookmarkSet,
    BookmarkInputChar(char),
    BookmarkInputBackspace,
    BookmarkInputConfirm,
    BookmarkDelete,
    GitPush,
    GitFetch,
    DescribeSave,
    DescribeEscalateEditor,
}
