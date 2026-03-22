use lajjzy_core::types::FileHunkSelection;

use crate::action::HunkPickerOp;

/// Effects emitted by dispatch. Defined in lajjzy-tui, executed in lajjzy-cli.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    // Read-only
    LoadGraph {
        /// Reserved for M3 omnibar. Currently always `None` (default revset).
        revset: Option<String>,
    },
    LoadOpLog,
    LoadFileDiff {
        change_id: String,
        path: String,
    },

    // Mutations
    Describe {
        change_id: String,
        text: String,
    },
    New {
        after: String,
    },
    Edit {
        change_id: String,
    },
    Abandon {
        change_id: String,
    },
    LoadChangeDiff {
        change_id: String,
        operation: HunkPickerOp,
    },
    Split {
        change_id: String,
        selections: Vec<FileHunkSelection>,
    },
    SquashPartial {
        change_id: String,
        selections: Vec<FileHunkSelection>,
    },
    Undo,
    Redo,
    BookmarkSet {
        change_id: String,
        name: String,
    },
    BookmarkDelete {
        name: String,
    },
    GitPush {
        bookmark: String,
    },
    GitFetch,
    RebaseSingle {
        source: String,
        destination: String,
    },
    RebaseWithDescendants {
        source: String,
        destination: String,
    },

    /// Try evaluating a revset expression. Executor calls `load_graph(Some(&query))`.
    EvalRevset {
        query: String,
    },

    // M7 mutations
    Absorb {
        change_id: String,
    },
    Duplicate {
        change_id: String,
    },
    Revert {
        change_id: String,
    },

    // Conflict handling
    LoadConflictData {
        change_id: String,
        path: String,
    },
    ResolveFile {
        change_id: String,
        path: String,
        content: Vec<u8>,
    },
    LaunchMergeTool {
        change_id: String,
        path: String,
    },

    // Forge
    FetchForgeStatus,
    /// Try to open the PR in a browser; if no PR exists, suspend and run `gh pr create`.
    /// The executor handles the routing — dispatch doesn't need the PR cache to be warm.
    OpenOrCreatePr {
        bookmark: String,
    },

    // Non-repo
    SuspendForEditor {
        change_id: String,
        initial_text: String,
    },
}
