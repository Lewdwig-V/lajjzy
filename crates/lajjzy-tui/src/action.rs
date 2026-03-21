#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelFocus {
    Graph,
    Detail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailMode {
    FileList,
    DiffView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    MoveUp,
    MoveDown,
    Quit,
    Refresh,
    JumpToTop,
    JumpToBottom,
    TabFocus,
    BackTabFocus,
    DetailMoveUp,
    DetailMoveDown,
    DetailEnter,
    DetailBack,
    DiffScrollUp,
    DiffScrollDown,
    DiffNextHunk,
    DiffPrevHunk,
    JumpToWorkingCopy,
    ToggleOpLog,
    OpenBookmarks,
    OpenFuzzyFind,
    OpenHelp,
    ModalDismiss,
    ModalMoveUp,
    ModalMoveDown,
    ModalEnter,
    FuzzyInput(char),
    FuzzyBackspace,
}
