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
    Squash {
        change_id: String,
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

    /// Try evaluating a revset expression. Executor calls `load_graph(Some(&query))`.
    EvalRevset {
        query: String,
    },

    // Non-repo
    SuspendForEditor {
        change_id: String,
        initial_text: String,
    },
}
