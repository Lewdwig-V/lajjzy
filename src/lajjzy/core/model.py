from __future__ import annotations

from dataclasses import dataclass

from lajjzy.backend.types import Bookmark, ConflictData, GraphData, OpLogEntry


@dataclass(frozen=True)
class Model:
    """The complete application state, as immutable plain data.

    This is the single source of truth for the change-graph view. It holds no
    Textual, asyncio, or jj references — it is constructed and transformed only
    by the pure ``update`` function, which makes every state transition testable
    without a renderer or a subprocess.

    Detail-pane browsing state (selected file, files/diff mode, fetched diff)
    is deliberately *not* here: it is ephemeral view-local state owned by the
    rendering backend, not core application logic.
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
    modal: str | None = (
        None  # "omnibar"|"bookmark_input"|"bookmark_picker"|"op_log"|"conflict_view"|"hunk_picker"|None
    )


def selected_change_id(model: Model) -> str | None:
    if model.graph is None:
        return None
    return model.graph.change_id_at(model.cursor)


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
