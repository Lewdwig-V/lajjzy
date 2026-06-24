from __future__ import annotations

from dataclasses import dataclass, field, replace
from typing import Literal

from lajjzy.backend.types import Bookmark, ConflictData, FileDiff, GraphData, OpLogEntry

Modal = Literal[
    "omnibar",
    "bookmark_input",
    "bookmark_picker",
    "op_log",
    "conflict_view",
    "hunk_picker",
]


@dataclass(frozen=True, slots=True)
class DetailState:
    """The detail pane's logic-bearing state, owned by the Model (not the
    widget). ``diff`` is loaded through the MVU loop (LoadChangeDiff →
    ChangeDiffLoaded); it is None until the load lands and while in files mode.

    ``mode == 'diff'`` with ``diff is None`` is the valid in-flight state
    between DetailOpenFile and ChangeDiffLoaded; the renderer shows a loading
    placeholder during that window.
    """

    file_cursor: int = 0
    mode: Literal["files", "diff"] = "files"
    diff: list[FileDiff] | None = None

    def __post_init__(self) -> None:
        if self.mode == "files" and self.diff is not None:
            raise ValueError("DetailState: diff must be None in files mode")
        if self.file_cursor < 0:
            raise ValueError(f"DetailState: file_cursor must be >= 0, got {self.file_cursor}")


@dataclass(frozen=True)
class Model:
    """The complete application state, as immutable plain data.

    This is the single source of truth for the change-graph view. It holds no
    Textual, asyncio, or jj references — it is constructed and transformed only
    by the pure ``update`` function, which makes every state transition testable
    without a renderer or a subprocess.
    """

    graph: GraphData | None = None
    cursor: int = 0
    error: str | None = None
    rebase_source: str | None = None
    rebase_descendants: bool = False
    pending_mutation: bool = False
    # Monotonic counter guarding graph loads: a load result is applied only if
    # its epoch still matches the model's, so a stale load cannot clobber a fresh
    # one. Incremented whenever the model launches a graph-producing effect.
    graph_epoch: int = 0
    # --- phase 1a additions ---
    op_log_entries: list[OpLogEntry] | None = None
    bookmarks: list[Bookmark] | None = None
    revset: str | None = None
    conflict_data: ConflictData | None = None
    conflict_path: str | None = None
    modal: Modal | None = None
    detail: DetailState = field(default_factory=DetailState)


def selected_change_id(model: Model) -> str | None:
    if model.graph is None:
        return None
    return model.graph.change_id_at(model.cursor)


def select_change(model: Model, cursor: int) -> Model:
    """Move the change-graph cursor. Resets the detail pane to a fresh
    DetailState whenever the selected line actually changes, so a stale diff or
    file cursor never carries across to a different change."""
    if cursor == model.cursor:
        return model  # no selection change → nothing to reset
    return replace(model, cursor=cursor, detail=DetailState())


def step_cursor(model: Model, delta: int) -> int:
    """Cursor index after moving ``delta`` change nodes, skipping connector lines.

    ``cursor`` indexes ``graph.lines`` (which includes graph-art connector lines);
    navigation steps between ``node_indices`` entries and clamps at the ends.
    """
    graph = model.graph
    if graph is None or not graph.node_indices:
        return model.cursor
    nodes = graph.node_indices
    try:
        pos = nodes.index(model.cursor)
    except ValueError:
        pos = 0
    pos = max(0, min(len(nodes) - 1, pos + delta))
    return nodes[pos]


def cursor_after_reload(graph: GraphData) -> int:
    """Where the cursor lands after a fresh graph load: the working copy if
    known, else the first change node, else 0."""
    if graph.working_copy_index is not None:
        return graph.working_copy_index
    if graph.node_indices:
        return graph.node_indices[0]
    return 0
