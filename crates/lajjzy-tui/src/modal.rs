use lajjzy_core::types::OpLogEntry;

#[derive(Debug, Clone)]
pub enum Modal {
    OpLog {
        entries: Vec<OpLogEntry>,
        cursor: usize,
        scroll: usize,
    },
    BookmarkPicker {
        bookmarks: Vec<(String, String)>, // (bookmark_name, change_id)
        cursor: usize,
    },
    FuzzyFind {
        query: String,
        matches: Vec<usize>, // graph line indices from node_indices
        cursor: usize,
    },
    Help {
        context: HelpContext,
        scroll: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpContext {
    Graph,
    DetailFileList,
    DetailDiffView,
}

impl HelpContext {
    pub fn line_count(self) -> usize {
        match self {
            Self::Graph => 10,
            Self::DetailFileList => 4,
            Self::DetailDiffView => 3,
        }
    }
}
