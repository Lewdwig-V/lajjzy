"""Pure unit tests for the MVU core.

These exercise every state transition with plain values — no Textual, no jj
subprocess, no asyncio, no event loop. This is the dividend the architecture
exists for: the entire logic of navigation, the mutation gate, the rebase state
machine, and the epoch guard is testable in microseconds against data.
"""

from __future__ import annotations

from lajjzy.backend.types import ChangeDetail, GraphData, GraphLine
from lajjzy.core import (
    Abandon,
    CursorBottom,
    CursorDown,
    CursorTop,
    CursorUp,
    DescribeAborted,
    DescribeReady,
    DescribeRequested,
    EditMessage,
    GraphLoaded,
    GraphLoadFailed,
    LoadGraph,
    Model,
    MutationCompleted,
    MutationFailed,
    NewChange,
    RebaseCancel,
    RebaseConfirm,
    RebaseStart,
    ReloadRequested,
    RunMutation,
    update,
)


def _detail(desc: str = "") -> ChangeDetail:
    return ChangeDetail(
        commit_id="c0",
        author="a",
        email="a@example.com",
        timestamp="now",
        description=desc,
        bookmarks=[],
        is_empty=False,
        has_conflict=False,
        files=[],
        parents=[],
    )


def _graph(*change_ids: str, working: int | None = 0) -> GraphData:
    """Build a graph with one node line per id (no connector lines)."""
    lines = [GraphLine(raw=cid, change_id=cid, glyph_prefix="") for cid in change_ids]
    details = {cid: _detail(desc=f"desc {cid}") for cid in change_ids}
    return GraphData(lines=lines, details=details, working_copy_index=working, op_id="op")


def _loaded(*change_ids: str, working: int | None = 0) -> Model:
    """A model with a graph already loaded at epoch 1, cursor on the working copy."""
    g = _graph(*change_ids, working=working)
    return Model(graph=g, cursor=working or 0, graph_epoch=1)


# --- navigation -------------------------------------------------------------


def test_cursor_down_and_up_move_between_nodes():
    m = _loaded("aaa", "bbb", "ccc", working=0)
    m1, cmds = update(m, CursorDown())
    assert m1.cursor == 1
    assert cmds == []
    m2, _ = update(m1, CursorUp())
    assert m2.cursor == 0


def test_cursor_clamps_at_ends():
    m = _loaded("aaa", "bbb", working=0)
    top, _ = update(m, CursorUp())
    assert top.cursor == 0  # already at top
    bottom, _ = update(update(m, CursorDown())[0], CursorDown())
    assert bottom.cursor == 1  # clamped at last node


def test_cursor_top_and_bottom():
    m = _loaded("aaa", "bbb", "ccc", working=1)
    assert update(m, CursorTop())[0].cursor == 0
    assert update(m, CursorBottom())[0].cursor == 2


def test_navigation_is_noop_without_graph():
    m = Model()
    assert update(m, CursorDown()) == (m, [])
    assert update(m, CursorTop()) == (m, [])


# --- reload + epoch guard ---------------------------------------------------


def test_reload_requested_bumps_epoch_and_emits_loadgraph():
    m = Model(graph_epoch=3)
    m1, cmds = update(m, ReloadRequested())
    assert m1.graph_epoch == 4
    assert cmds == [LoadGraph(4)]


def test_graph_loaded_applies_when_epoch_matches():
    m = Model(graph_epoch=2)
    g = _graph("aaa", "bbb", working=1)
    m1, cmds = update(m, GraphLoaded(2, g))
    assert m1.graph is g
    assert m1.cursor == 1  # lands on working copy
    assert m1.error is None
    assert cmds == []


def test_graph_loaded_discarded_when_epoch_stale():
    m = Model(graph_epoch=5, error="old")
    g = _graph("aaa")
    m1, _ = update(m, GraphLoaded(4, g))  # stale
    assert m1 is m  # untouched, error not cleared


def test_graph_load_failed_sets_error():
    m = Model(graph_epoch=1)
    m1, _ = update(m, GraphLoadFailed("boom"))
    assert m1.error == "boom"


# --- mutation gate ----------------------------------------------------------


def test_new_change_starts_mutation_and_arms_gate():
    m = _loaded("aaa", "bbb", working=0)
    m1, cmds = update(m, NewChange())
    assert m1.pending_mutation is True
    assert m1.graph_epoch == 2
    assert cmds == [RunMutation(2, "new", ("aaa",))]


def test_second_mutation_rejected_while_pending():
    m = _loaded("aaa", working=0)
    armed, _ = update(m, NewChange())
    blocked, cmds = update(armed, Abandon())
    assert cmds == []
    assert blocked.error == "A mutation is already in progress"
    assert blocked.pending_mutation is True


def test_mutation_without_selection_errors():
    m = Model(graph=_graph(working=None), cursor=0)  # empty graph, nothing selected
    m1, cmds = update(m, NewChange())
    assert cmds == []
    assert m1.error == "No change selected"


def test_mutation_failed_reopens_gate_and_keeps_graph():
    m = _loaded("aaa", working=0)
    armed, _ = update(m, Abandon())
    done, cmds = update(armed, MutationFailed("nope"))
    assert done.pending_mutation is False
    assert done.error == "nope"
    assert done.graph is m.graph  # graph untouched on op failure
    assert cmds == []


def test_mutation_completed_applies_fresh_graph_and_reports_message():
    m = _loaded("aaa", working=0)
    armed, [cmd] = update(m, NewChange())
    new_graph = _graph("aaa", "bbb", working=1)
    done, _ = update(armed, MutationCompleted(cmd.epoch, "Created", new_graph, None))
    assert done.pending_mutation is False
    assert done.graph is new_graph
    assert done.cursor == 1
    assert done.error == "Created"


def test_mutation_completed_keeps_message_but_discards_stale_reload():
    m = _loaded("aaa", working=0)
    armed, [cmd] = update(m, NewChange())
    # A newer graph-producing op bumps the epoch before this reload lands.
    superseded, _ = update(armed, ReloadRequested())
    stale = _graph("aaa", "bbb")
    done, _ = update(superseded, MutationCompleted(cmd.epoch, "Created", stale, None))
    assert done.pending_mutation is False
    assert done.error == "Created"
    assert done.graph is m.graph  # stale reload not applied


def test_mutation_completed_load_error_takes_precedence():
    m = _loaded("aaa", working=0)
    armed, [cmd] = update(m, NewChange())
    done, _ = update(armed, MutationCompleted(cmd.epoch, "Created", None, "load failed"))
    assert done.pending_mutation is False
    assert done.error == "load failed"


# --- describe (editor round-trip) -------------------------------------------


def test_describe_requested_emits_editmessage_with_seed():
    m = _loaded("aaa", working=0)
    _, cmds = update(m, DescribeRequested())
    assert cmds == [EditMessage("aaa", "desc aaa")]


def test_describe_ready_starts_describe_mutation():
    m = _loaded("aaa", working=0)
    m1, cmds = update(m, DescribeReady("aaa", "new message"))
    assert m1.pending_mutation is True
    assert cmds == [RunMutation(m1.graph_epoch, "describe", ("aaa", "new message"))]


def test_describe_aborted_with_error_sets_error():
    m = _loaded("aaa", working=0)
    assert update(m, DescribeAborted("No $EDITOR set"))[0].error == "No $EDITOR set"


def test_describe_aborted_silently_when_no_error():
    m = _loaded("aaa", working=0)
    assert update(m, DescribeAborted(None)) == (m, [])


# --- rebase state machine ---------------------------------------------------


def test_rebase_start_arms_source_and_prompts():
    m = _loaded("aaa", "bbb", working=0)
    armed, cmds = update(m, RebaseStart(descendants=False))
    assert armed.rebase_source == "aaa"
    assert "pick a destination" in armed.error
    assert cmds == []


def test_rebase_confirm_launches_mutation_for_valid_destination():
    m = _loaded("aaa", "bbb", working=0)
    armed, _ = update(m, RebaseStart(descendants=False))
    moved, _ = update(armed, CursorDown())  # select bbb as destination
    done, cmds = update(moved, RebaseConfirm())
    assert done.rebase_source is None
    assert cmds == [RunMutation(done.graph_epoch, "rebase", ("aaa", "bbb"))]


def test_rebase_confirm_descendants_uses_descendants_kind():
    m = _loaded("aaa", "bbb", working=0)
    armed, _ = update(m, RebaseStart(descendants=True))
    moved, _ = update(armed, CursorDown())
    _, cmds = update(moved, RebaseConfirm())
    assert cmds[0].kind == "rebase_descendants"


def test_rebase_confirm_same_destination_cancels():
    m = _loaded("aaa", "bbb", working=0)
    armed, _ = update(m, RebaseStart(descendants=False))
    done, cmds = update(armed, RebaseConfirm())  # cursor still on source
    assert cmds == []
    assert done.rebase_source is None
    assert "cancelled" in done.error.lower()


def test_rebase_confirm_is_noop_when_not_armed():
    m = _loaded("aaa", working=0)
    assert update(m, RebaseConfirm()) == (m, [])


def test_rebase_cancel_clears_source():
    m = _loaded("aaa", working=0)
    armed, _ = update(m, RebaseStart(descendants=False))
    done, _ = update(armed, RebaseCancel())
    assert done.rebase_source is None
    assert done.error == "Rebase cancelled"
