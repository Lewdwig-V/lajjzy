use crate::types::{FileDiff, GraphData};
use anyhow::Result;

/// Abstraction over jj repo access. Implementations may shell out to jj CLI
/// or link against jj-lib directly.
///
/// Contract: Implementations must return a `GraphData` where every `GraphLine`
/// with a `change_id` has a corresponding entry in `details`, and
/// `working_copy_index` (if `Some`) points to a node line.
pub trait RepoBackend: Send + Sync {
    /// Load the full change graph for display.
    /// Returns all graph lines with pre-loaded change details.
    /// If `revset` is `Some`, passes `-r <revset>` to `jj log` to filter results.
    fn load_graph(&self, revset: Option<&str>) -> Result<GraphData>;

    /// Compute diff hunks for a specific file in a change.
    /// Lazy — called only when user drills into a file.
    fn file_diff(&self, change_id: &str, path: &str) -> Result<Vec<crate::types::DiffHunk>>;

    /// Return all file diffs for a change, grouped by file.
    /// Each entry contains the file path and its parsed hunks.
    fn change_diff(&self, change_id: &str) -> Result<Vec<FileDiff>>;

    /// Load the operation log.
    fn op_log(&self) -> Result<Vec<crate::types::OpLogEntry>>;

    /// Set the description on a change.
    fn describe(&self, change_id: &str, text: &str) -> Result<String>;

    /// Create a new empty change after the given revision.
    fn new_change(&self, after: &str) -> Result<String>;

    /// Move the working copy to the given change.
    fn edit_change(&self, change_id: &str) -> Result<String>;

    /// Abandon (delete) the given change.
    fn abandon(&self, change_id: &str) -> Result<String>;

    /// Undo the most recent operation (`jj op restore @-`).
    fn undo(&self) -> Result<String>;

    /// Redo the most recently undone operation (`jj op revert @`).
    fn redo(&self) -> Result<String>;

    /// Create or move a bookmark to the given revision.
    fn bookmark_set(&self, change_id: &str, name: &str) -> Result<String>;

    /// Delete a bookmark by name.
    fn bookmark_delete(&self, name: &str) -> Result<String>;

    /// Push a bookmark to its remote.
    fn git_push(&self, bookmark: &str) -> Result<String>;

    /// Fetch all remotes.
    fn git_fetch(&self) -> Result<String>;

    /// Rebase a single revision onto a new parent, reparenting its descendants.
    fn rebase_single(&self, source: &str, destination: &str) -> Result<String>;

    /// Rebase a revision and all of its descendants onto a new parent.
    fn rebase_with_descendants(&self, source: &str, destination: &str) -> Result<String>;

    /// Split a change into two: selected files move to a new child, unselected
    /// stay in the original.
    ///
    /// `selections` describes every file in the change and which hunks are
    /// selected. A file is considered "fully selected" when
    /// `selected_hunks.len() == total_hunks`.  Fully-selected files end up in
    /// the child; the rest remain in the original.
    fn split(
        &self,
        change_id: &str,
        selections: &[crate::types::FileHunkSelection],
    ) -> Result<String>;

    /// Squash a subset of files from a change into its parent.
    ///
    /// Any file in `selections` with at least one selected hunk is moved to
    /// the parent.  Uses `-u` so jj does not open `$EDITOR` for a combined
    /// description.
    fn squash_partial(
        &self,
        change_id: &str,
        selections: &[crate::types::FileHunkSelection],
    ) -> Result<String>;
}
