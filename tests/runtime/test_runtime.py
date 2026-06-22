"""Drive the full MVU loop through a headless Backend — no Textual, no jj.

This is the swappability claim made concrete: the same core (Model + update) and
the same Runtime run end-to-end against an in-memory backend that fakes the
effects. If this passes, the renderer truly is a swappable detail.
"""

from __future__ import annotations

from collections.abc import Callable

from lajjzy.backend.types import ChangeDetail, GraphData, GraphLine
from lajjzy.core import (
    Cmd,
    EditMessage,
    GraphLoaded,
    LoadGraph,
    Model,
    MutationCompleted,
    Msg,
    NewChange,
    ReloadRequested,
    RunMutation,
)
from lajjzy.runtime import Runtime


def _graph(*ids: str) -> GraphData:
    lines = [GraphLine(raw=i, change_id=i, glyph_prefix="") for i in ids]
    details = {i: ChangeDetail("c", "a", "a@x", "now", "", [], False, False, [], []) for i in ids}
    return GraphData(lines=lines, details=details, working_copy_index=0, op_id="op")


class FakeBackend:
    """A Backend that records presented models and synchronously fakes effects."""

    def __init__(self, graphs: list[GraphData]) -> None:
        self.presented: list[Model] = []
        self.ops: list[tuple[str, tuple]] = []
        self.editor_text: str | None = None
        self._graphs = graphs

    def present(self, model: Model) -> None:
        self.presented.append(model)

    def run_cmd(self, cmd: Cmd, dispatch: Callable[[Msg], None]) -> None:
        if isinstance(cmd, LoadGraph):
            dispatch(GraphLoaded(cmd.epoch, self._graphs.pop(0)))
        elif isinstance(cmd, RunMutation):
            self.ops.append((cmd.kind, cmd.args))
            graph = self._graphs.pop(0) if self._graphs else None
            dispatch(MutationCompleted(cmd.epoch, f"did {cmd.kind}", graph, None))
        elif isinstance(cmd, EditMessage):
            # An editor that always returns fixed text would go here; unused.
            pass


def test_runtime_loads_graph_through_backend():
    backend = FakeBackend([_graph("aaa", "bbb")])
    rt = Runtime(backend)
    rt.dispatch(ReloadRequested())
    assert rt.model.graph is not None
    assert [ln.change_id for ln in rt.model.graph.lines] == ["aaa", "bbb"]
    # present() was called for the reload intent and again for the load result.
    assert len(backend.presented) == 2


def test_runtime_runs_mutation_end_to_end():
    backend = FakeBackend([_graph("aaa"), _graph("aaa", "bbb")])
    rt = Runtime(backend)
    rt.dispatch(ReloadRequested())  # initial graph: just "aaa", cursor on it
    rt.dispatch(NewChange())
    assert backend.ops == [("new", ("aaa",))]
    # Gate opened again and the reloaded graph (with "bbb") is now in the model.
    assert rt.model.pending_mutation is False
    assert [ln.change_id for ln in rt.model.graph.lines] == ["aaa", "bbb"]
    assert rt.model.error == "did new"
