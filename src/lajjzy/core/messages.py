from __future__ import annotations

from dataclasses import dataclass

from lajjzy.backend.types import (
    Bookmark,
    ConflictData,
    FileRef,
    GraphData,
    HunkResolutionValue,
    OpLogEntry,
)

# ---------------------------------------------------------------------------
# User intents — produced by key bindings in the rendering backend.
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class CursorDown:
    pass


@dataclass(frozen=True)
class CursorUp:
    pass


@dataclass(frozen=True)
class CursorTop:
    pass


@dataclass(frozen=True)
class CursorBottom:
    pass


@dataclass(frozen=True)
class ReloadRequested:
    pass


@dataclass(frozen=True)
class NewChange:
    pass


@dataclass(frozen=True)
class Abandon:
    pass


@dataclass(frozen=True)
class EditChange:
    pass


@dataclass(frozen=True)
class Squash:
    pass


@dataclass(frozen=True)
class DescribeRequested:
    pass


@dataclass(frozen=True)
class RebaseStart:
    descendants: bool


@dataclass(frozen=True)
class RebaseConfirm:
    pass


@dataclass(frozen=True)
class RebaseCancel:
    pass


# ---------------------------------------------------------------------------
# Effect results — produced by the backend when a Cmd finishes.
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class GraphLoaded:
    epoch: int
    graph: GraphData


@dataclass(frozen=True)
class GraphLoadFailed:
    error: str


@dataclass(frozen=True)
class DescribeReady:
    change_id: str
    text: str


@dataclass(frozen=True)
class DescribeAborted:
    error: str | None


@dataclass(frozen=True)
class MutationFailed:
    error: str


@dataclass(frozen=True)
class MutationCompleted:
    """A mutation op finished and its follow-up graph reload was attempted.

    ``message`` is the op's success text. ``graph`` is the reloaded graph (or
    None if the reload failed), and ``load_error`` carries the reload failure.
    ``epoch`` is the graph epoch assigned when the mutation launched, used to
    discard the reload if a newer graph-producing op has since superseded it.
    ``bookmarks`` is the refreshed bookmarks list for bookmark-kind mutations,
    or None if not applicable / the refresh failed.
    """

    epoch: int
    message: str
    graph: GraphData | None
    load_error: str | None
    bookmarks: list[Bookmark] | None = None


# --- undo / redo -------------------------------------------------------
@dataclass(frozen=True)
class Undo:
    pass


@dataclass(frozen=True)
class Redo:
    pass


# --- omnibar -----------------------------------------------------------
@dataclass(frozen=True)
class OpenOmnibar:
    pass


@dataclass(frozen=True)
class OmnibarInput:
    char: str


@dataclass(frozen=True)
class OmnibarBackspace:
    pass


@dataclass(frozen=True)
class OmnibarAcceptCompletion:
    pass


@dataclass(frozen=True)
class OmnibarSubmit:
    revset: str | None  # None = clear filter, empty string = no-op, non-empty = apply


@dataclass(frozen=True)
class OmnibarCancel:
    pass


# --- bookmarks ---------------------------------------------------------
@dataclass(frozen=True)
class OpenBookmarkSet:
    pass


@dataclass(frozen=True)
class OpenBookmarkPicker:
    pass


@dataclass(frozen=True)
class BookmarkInputConfirm:
    name: str


@dataclass(frozen=True)
class BookmarkInputCancel:
    pass


@dataclass(frozen=True)
class BookmarkDelete:
    name: str


@dataclass(frozen=True)
class BookmarkMove:
    name: str


@dataclass(frozen=True)
class BookmarkMoveConfirm:
    name: str
    dest_change_id: str


@dataclass(frozen=True)
class BookmarksLoaded:
    bookmarks: list[Bookmark]


@dataclass(frozen=True)
class BookmarksLoadFailed:
    error: str


# --- operation log -----------------------------------------------------
@dataclass(frozen=True)
class OpenOpLog:
    pass


@dataclass(frozen=True)
class OpLogClose:
    pass


@dataclass(frozen=True)
class OpLogRestore:
    op_id: str


@dataclass(frozen=True)
class OpLogLoaded:
    entries: list[OpLogEntry]


@dataclass(frozen=True)
class OpLogLoadFailed:
    error: str


# --- conflict view -----------------------------------------------------
@dataclass(frozen=True)
class OpenConflictView:
    path: str


@dataclass(frozen=True)
class ConflictViewClose:
    pass


@dataclass(frozen=True)
class ApplyResolutions:
    path: str
    resolutions: list[HunkResolutionValue]  # one HunkResolution.* value per conflict region


@dataclass(frozen=True)
class ConflictDataLoaded:
    data: ConflictData


@dataclass(frozen=True)
class ConflictDataLoadFailed:
    error: str


# --- hunk picker (split / partial squash) ------------------------------
@dataclass(frozen=True)
class DetailBack:
    pass


@dataclass(frozen=True)
class DetailFileDown:
    pass


@dataclass(frozen=True)
class DetailFileUp:
    pass


@dataclass(frozen=True)
class Split:
    pass


@dataclass(frozen=True)
class SquashPartial:
    pass


@dataclass(frozen=True)
class HunkPickerClose:
    pass


@dataclass(frozen=True)
class SplitConfirm:
    source: str
    files: list[FileRef]


@dataclass(frozen=True)
class SquashPartialConfirm:
    source: str
    files: list[FileRef]


Msg = (
    CursorDown
    | CursorUp
    | CursorTop
    | CursorBottom
    | DetailBack
    | DetailFileDown
    | DetailFileUp
    | ReloadRequested
    | NewChange
    | Abandon
    | EditChange
    | Squash
    | DescribeRequested
    | RebaseStart
    | RebaseConfirm
    | RebaseCancel
    | GraphLoaded
    | GraphLoadFailed
    | DescribeReady
    | DescribeAborted
    | MutationFailed
    | MutationCompleted
    | Undo
    | Redo
    | OpenOmnibar
    | OmnibarInput
    | OmnibarBackspace
    | OmnibarAcceptCompletion
    | OmnibarSubmit
    | OmnibarCancel
    | OpenBookmarkSet
    | OpenBookmarkPicker
    | BookmarkInputConfirm
    | BookmarkInputCancel
    | BookmarkDelete
    | BookmarkMove
    | BookmarkMoveConfirm
    | BookmarksLoaded
    | BookmarksLoadFailed
    | OpenOpLog
    | OpLogClose
    | OpLogRestore
    | OpLogLoaded
    | OpLogLoadFailed
    | OpenConflictView
    | ConflictViewClose
    | ApplyResolutions
    | ConflictDataLoaded
    | ConflictDataLoadFailed
    | Split
    | SquashPartial
    | HunkPickerClose
    | SplitConfirm
    | SquashPartialConfirm
)
