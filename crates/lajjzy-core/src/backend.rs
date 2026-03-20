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
}
