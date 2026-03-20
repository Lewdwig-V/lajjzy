use crate::types::GraphData;
use anyhow::Result;

/// Abstraction over jj repo access. Implementations may shell out to jj CLI
/// or link against jj-lib directly.
pub trait RepoBackend {
    /// Load the full change graph for display.
    /// Returns all graph lines with pre-loaded change details.
    fn load_graph(&self) -> Result<GraphData>;
}
