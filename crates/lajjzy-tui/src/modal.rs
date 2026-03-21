use lajjzy_core::types::OpLogEntry;
use tui_textarea::TextArea;

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
    Omnibar {
        query: String,
        matches: Vec<usize>, // graph line indices from node_indices
        cursor: usize,
    },
    Help {
        context: HelpContext,
        scroll: usize,
    },
    Describe {
        change_id: String,
        editor: Box<TextArea<'static>>,
    },
    BookmarkInput {
        change_id: String,
        input: String,
        completions: Vec<String>,
        cursor: usize,
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
            Self::Graph => 20,
            Self::DetailFileList => 4,
            Self::DetailDiffView => 3,
        }
    }
}
