use std::collections::HashMap;

/// An entry in the jj operation log.
#[derive(Debug, Clone, PartialEq)]
pub struct OpLogEntry {
    pub id: String,
    pub description: String,
    pub timestamp: String,
}

/// Complete graph data returned by `load_graph()`.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphData {
    /// All lines of graph output.
    pub lines: Vec<GraphLine>,
    /// Details for each change, keyed by change ID.
    pub details: HashMap<String, ChangeDetail>,
    /// Index of the working-copy change's node line in `lines`.
    pub working_copy_index: Option<usize>,
    /// Pre-computed indices of node lines (lines with a `change_id`).
    cached_node_indices: Vec<usize>,
    /// jj operation ID at the time this snapshot was taken.
    /// Used to identify which repo state this graph represents.
    pub op_id: String,
}

/// One line of jj's graph output.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphLine {
    /// The display string (graph glyphs + text), delimiter stripped.
    pub raw: String,
    /// The change ID if this is a node line (first line of a change block).
    /// None for continuation/connector lines.
    pub change_id: Option<String>,
    /// Everything in the display string before the first alphanumeric character.
    /// For node lines this is the graph glyph prefix; for connector lines it is
    /// the entire `raw` string.
    pub glyph_prefix: String,
}

/// Detailed info for the status bar.
#[derive(Debug, Clone, PartialEq)]
pub struct ChangeDetail {
    pub commit_id: String,
    pub author: String,
    pub email: String,
    pub timestamp: String,
    pub description: String,
    pub bookmarks: Vec<String>,
    pub is_empty: bool,
    pub conflict_count: usize,
    pub files: Vec<FileChange>,
    pub parents: Vec<String>,
}

/// A file changed in a change (parsed from `jj log --summary`).
#[derive(Debug, Clone, PartialEq)]
pub struct FileChange {
    pub path: String,
    pub status: FileStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    /// Rename: path contains `{old => new}` format from jj.
    Renamed,
    Conflicted,
    /// Unknown status code from jj — displayed as-is.
    Unknown(char),
}

impl std::fmt::Display for FileStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Added => write!(f, "A"),
            Self::Modified => write!(f, "M"),
            Self::Deleted => write!(f, "D"),
            Self::Renamed => write!(f, "R"),
            Self::Conflicted => write!(f, "C"),
            Self::Unknown(c) => write!(f, "{c}"),
        }
    }
}

/// All hunks for a single file in a change's diff.
#[derive(Debug, Clone, PartialEq)]
pub struct FileDiff {
    pub path: String,
    pub hunks: Vec<DiffHunk>,
}

/// User's hunk selection for a single file (sent to backend for split/squash).
#[derive(Debug, Clone, PartialEq)]
pub struct FileHunkSelection {
    pub path: String,
    pub selected_hunks: Vec<usize>,
    pub total_hunks: usize,
}

/// Structured conflict data for a single file.
#[derive(Debug, Clone, PartialEq)]
pub struct ConflictData {
    pub regions: Vec<ConflictRegion>,
}

/// A region of a conflicted file — either resolved content or a conflict hunk.
#[derive(Debug, Clone, PartialEq)]
pub enum ConflictRegion {
    /// Non-conflicting content between conflict hunks.
    Resolved(String),
    /// A single conflict hunk with its three sides.
    /// An empty string for any side means that side deleted the file/region.
    Conflict {
        base: String,
        left: String,
        right: String,
    },
}

/// A hunk from a file diff (parsed from `jj diff --git`).
#[derive(Debug, Clone, PartialEq)]
pub struct DiffHunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
    Header,
}

impl GraphData {
    pub fn new(
        lines: Vec<GraphLine>,
        details: HashMap<String, ChangeDetail>,
        working_copy_index: Option<usize>,
        op_id: String,
    ) -> Self {
        let cached_node_indices = lines
            .iter()
            .enumerate()
            .filter_map(|(i, line)| line.change_id.as_ref().map(|_| i))
            .collect();
        Self {
            lines,
            details,
            working_copy_index,
            cached_node_indices,
            op_id,
        }
    }

    /// Returns the indices of all node lines (lines with a change ID).
    pub fn node_indices(&self) -> &[usize] {
        &self.cached_node_indices
    }

    /// Returns the `ChangeDetail` for the node line at the given index, if any.
    pub fn detail_at(&self, index: usize) -> Option<&ChangeDetail> {
        self.lines
            .get(index)
            .and_then(|line| line.change_id.as_ref())
            .and_then(|id| self.details.get(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_graph() -> GraphData {
        GraphData::new(
            vec![
                GraphLine {
                    raw: "◉  abc123 alice 2m ago".into(),
                    change_id: Some("abc123".into()),
                    glyph_prefix: String::new(),
                },
                GraphLine {
                    raw: "│  fix: resolve parser bug".into(),
                    change_id: None,
                    glyph_prefix: String::new(),
                },
                GraphLine {
                    raw: "◉  def456 bob 1h ago".into(),
                    change_id: Some("def456".into()),
                    glyph_prefix: String::new(),
                },
                GraphLine {
                    raw: "│  feat: add retry logic".into(),
                    change_id: None,
                    glyph_prefix: String::new(),
                },
                GraphLine {
                    raw: "◉  ghi789 root()".into(),
                    change_id: Some("ghi789".into()),
                    glyph_prefix: String::new(),
                },
            ],
            HashMap::from([
                (
                    "abc123".into(),
                    ChangeDetail {
                        commit_id: "aaa111".into(),
                        author: "alice".into(),
                        email: "alice@example.com".into(),
                        timestamp: "2 minutes ago".into(),
                        description: "fix: resolve parser bug".into(),
                        bookmarks: vec!["main".into()],
                        is_empty: false,
                        conflict_count: 0,
                        files: vec![],
                        parents: vec![],
                    },
                ),
                (
                    "def456".into(),
                    ChangeDetail {
                        commit_id: "bbb222".into(),
                        author: "bob".into(),
                        email: "bob@example.com".into(),
                        timestamp: "1 hour ago".into(),
                        description: "feat: add retry logic".into(),
                        bookmarks: vec![],
                        is_empty: false,
                        conflict_count: 0,
                        files: vec![],
                        parents: vec![],
                    },
                ),
                (
                    "ghi789".into(),
                    ChangeDetail {
                        commit_id: "ccc333".into(),
                        author: "root".into(),
                        email: String::new(),
                        timestamp: String::new(),
                        description: String::new(),
                        bookmarks: vec![],
                        is_empty: true,
                        conflict_count: 0,
                        files: vec![],
                        parents: vec![],
                    },
                ),
            ]),
            Some(0),
            String::new(),
        )
    }

    #[test]
    fn node_indices_returns_only_change_nodes() {
        let graph = sample_graph();
        assert_eq!(graph.node_indices(), vec![0, 2, 4]);
    }

    #[test]
    fn detail_at_returns_detail_for_node_line() {
        let graph = sample_graph();
        let detail = graph.detail_at(0).unwrap();
        assert_eq!(detail.author, "alice");
    }

    #[test]
    fn detail_at_returns_none_for_connector_line() {
        let graph = sample_graph();
        assert!(graph.detail_at(1).is_none());
    }

    #[test]
    fn detail_at_returns_none_for_out_of_bounds() {
        let graph = sample_graph();
        assert!(graph.detail_at(99).is_none());
    }
}
