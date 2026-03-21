use crate::types::GraphData;
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
    fn load_graph(&self) -> Result<GraphData>;

    /// Compute diff hunks for a specific file in a change.
    /// Lazy — called only when user drills into a file.
    fn file_diff(&self, change_id: &str, path: &str) -> Result<Vec<crate::types::DiffHunk>>;

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

    /// Squash the given change into its parent.
    fn squash(&self, change_id: &str) -> Result<String>;

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
}
