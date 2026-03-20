use std::collections::HashMap;

/// Complete graph data returned by load_graph().
#[derive(Debug, Clone)]
pub struct GraphData {
    /// All lines of graph output.
    pub lines: Vec<GraphLine>,
    /// Details for each change, keyed by change ID.
    pub details: HashMap<String, ChangeDetail>,
    /// Index of the working-copy change's node line in `lines`.
    pub working_copy_index: Option<usize>,
}

/// One line of jj's graph output.
#[derive(Debug, Clone)]
pub struct GraphLine {
    /// The display string (graph glyphs + text), delimiter stripped.
    pub raw: String,
    /// The change ID if this is a node line (first line of a change block).
    /// None for continuation/connector lines.
    pub change_id: Option<String>,
}

/// Detailed info for the status bar.
#[derive(Debug, Clone)]
pub struct ChangeDetail {
    pub change_id: String,
    pub commit_id: String,
    pub author: String,
    pub email: String,
    pub timestamp: String,
    pub description: String,
    pub bookmarks: Vec<String>,
    pub is_empty: bool,
    pub has_conflict: bool,
    pub is_working_copy: bool,
}

impl GraphData {
    /// Returns the indices of all node lines (lines with a change ID).
    pub fn node_indices(&self) -> Vec<usize> {
        self.lines
            .iter()
            .enumerate()
            .filter_map(|(i, line)| line.change_id.as_ref().map(|_| i))
            .collect()
    }

    /// Returns the ChangeDetail for the node line at the given index, if any.
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
        GraphData {
            lines: vec![
                GraphLine {
                    raw: "◉  abc123 alice 2m ago".into(),
                    change_id: Some("abc123".into()),
                },
                GraphLine {
                    raw: "│  fix: resolve parser bug".into(),
                    change_id: None,
                },
                GraphLine {
                    raw: "◉  def456 bob 1h ago".into(),
                    change_id: Some("def456".into()),
                },
                GraphLine {
                    raw: "│  feat: add retry logic".into(),
                    change_id: None,
                },
                GraphLine {
                    raw: "◉  ghi789 root()".into(),
                    change_id: Some("ghi789".into()),
                },
            ],
            details: HashMap::from([
                (
                    "abc123".into(),
                    ChangeDetail {
                        change_id: "abc123".into(),
                        commit_id: "aaa111".into(),
                        author: "alice".into(),
                        email: "alice@example.com".into(),
                        timestamp: "2 minutes ago".into(),
                        description: "fix: resolve parser bug".into(),
                        bookmarks: vec!["main".into()],
                        is_empty: false,
                        has_conflict: false,
                        is_working_copy: true,
                    },
                ),
                (
                    "def456".into(),
                    ChangeDetail {
                        change_id: "def456".into(),
                        commit_id: "bbb222".into(),
                        author: "bob".into(),
                        email: "bob@example.com".into(),
                        timestamp: "1 hour ago".into(),
                        description: "feat: add retry logic".into(),
                        bookmarks: vec![],
                        is_empty: false,
                        has_conflict: false,
                        is_working_copy: false,
                    },
                ),
                (
                    "ghi789".into(),
                    ChangeDetail {
                        change_id: "ghi789".into(),
                        commit_id: "ccc333".into(),
                        author: "root".into(),
                        email: "".into(),
                        timestamp: "".into(),
                        description: "".into(),
                        bookmarks: vec![],
                        is_empty: true,
                        has_conflict: false,
                        is_working_copy: false,
                    },
                ),
            ]),
            working_copy_index: Some(0),
        }
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
        assert_eq!(detail.change_id, "abc123");
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
