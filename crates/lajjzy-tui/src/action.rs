use lajjzy_core::forge::PrInfo;
use lajjzy_core::types::{ConflictData, DiffHunk, FileDiff, GraphData, OpLogEntry};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arity {
    Nullary,  // insert "()" — complete
    Optional, // insert "(" — user can close or add arg
    Required, // insert "(" — needs argument
}

/// A single completion candidate for the omnibar.
#[derive(Debug, Clone, PartialEq)]
pub struct CompletionItem {
    /// The text to insert (e.g., "ancestors(", "`mine()`", "main")
    pub insert_text: String,
    /// The text to display in the dropdown (e.g., "ksqxwpml — refactor: extract trait")
    pub display_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebaseMode {
    Single,
    WithDescendants,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelFocus {
    Graph,
    Detail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailMode {
    FileList,
    DiffView,
    HunkPicker,
    ConflictView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MutationKind {
    Describe,
    New,
    Edit,
    Abandon,
    Split,
    SquashPartial,
    Undo,
    Redo,
    BookmarkSet,
    BookmarkDelete,
    GitPush,
    GitFetch,
    RebaseSingle,
    RebaseWithDescendants,
    ResolveConflict,
    Absorb,
    Duplicate,
    Revert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackgroundKind {
    Push,
    Fetch,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HunkPickerOp {
    Split { source: String },
    Squash { source: String, destination: String },
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
    OpenOmnibar,
    OpenHelp,
    ModalDismiss,
    ModalMoveUp,
    ModalMoveDown,
    ModalEnter,
    OmnibarInput(char),
    OmnibarBackspace,
    OmnibarAcceptCompletion,

    // Effect result actions
    /// `generation` is a monotonic counter assigned by the executor at load time.
    /// Dispatch rejects snapshots with generation < current to prevent stale overwrites.
    GraphLoaded {
        generation: u64,
        result: Result<GraphData, String>,
    },
    OpLogLoaded(Result<Vec<OpLogEntry>, String>),
    FileDiffLoaded(Result<Vec<DiffHunk>, String>),
    ChangeDiffLoaded {
        operation: HunkPickerOp,
        result: Result<Vec<FileDiff>, String>,
    },
    RepoOpSuccess {
        op: MutationKind,
        message: String,
        /// Refreshed graph bundled with success so gate clears atomically
        /// with graph replacement. None only if `load_graph` failed post-mutation.
        /// The `u64` is the generation counter for staleness rejection.
        graph: Option<(u64, Result<GraphData, String>)>,
    },
    RepoOpFailed {
        op: MutationKind,
        error: String,
    },
    EditorComplete {
        change_id: String,
        text: String,
    },
    RevsetLoaded {
        query: String,
        generation: u64,
        result: Result<GraphData, String>,
    },

    // Mutation trigger actions
    Abandon,
    Split,
    SquashPartial,
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
    RebaseSingle,
    RebaseWithDescendants,
    Absorb,
    DuplicateChange,
    Revert,
    PickConfirm,
    PickCancel,
    PickFilterChar(char),
    PickFilterBackspace,

    // Hunk picker actions
    HunkToggle,
    HunkSelectAll,
    HunkDeselectAll,
    HunkNextFile,
    HunkPrevFile,
    HunkConfirm,
    HunkCancel,

    // Conflict view actions
    ConflictAcceptLeft,
    ConflictAcceptRight,
    ConflictConfirm,
    ConflictLaunchMerge,
    ConflictNextHunk,
    ConflictPrevHunk,
    ConflictScrollDown,
    ConflictScrollUp,

    // File list conflict navigation
    NextConflictFile,
    PrevConflictFile,

    // Conflict effect results
    ConflictDataLoaded {
        change_id: String,
        path: String,
        result: Result<ConflictData, String>,
    },
    MergeToolComplete {
        path: String,
        graph: Option<(u64, Result<GraphData, String>)>,
    },
    MergeToolFailed {
        path: String,
        error: String,
    },

    // Forge actions
    FetchForgeStatus,
    OpenOrCreatePr,
    ForgeStatusLoaded(Result<Option<Vec<PrInfo>>, String>),
    PrViewUrl {
        url: String,
    },
    PrCreateComplete,
    PrCreateFailed {
        error: String,
    },

    // Mouse actions
    ClickGraphNode {
        line_index: usize,
    },
    ClickDetailItem {
        index: usize,
    },
    ClickFocusGraph,
    ClickFocusDetail,
    ScrollUp {
        count: usize,
    },
    ScrollDown {
        count: usize,
    },
}
