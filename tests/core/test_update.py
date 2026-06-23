"""Pure unit tests for the MVU core.

These exercise every state transition with plain values — no Textual, no jj
subprocess, no asyncio, no event loop. This is the dividend the architecture
exists for: the entire logic of navigation, the mutation gate, the rebase state
machine, and the epoch guard is testable in microseconds against data.
"""

from __future__ import annotations

from dataclasses import replace

from lajjzy.backend.types import Bookmark, ChangeDetail, GraphData, GraphLine
from lajjzy.core import (
    Abandon,
    BookmarkDelete,
    BookmarkInputCancel,
    BookmarkInputConfirm,
    BookmarkMoveConfirm,
    BookmarksLoadFailed,
    BookmarksLoaded,
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
    LoadBookmarks,
    LoadGraph,
    Model,
    MutationCompleted,
    MutationFailed,
    NewChange,
    OmnibarCancel,
    OmnibarSubmit,
    OpenBookmarkPicker,
    OpenBookmarkSet,
    OpenOmnibar,
    RebaseCancel,
    RebaseConfirm,
    RebaseStart,
    Redo,
    ReloadRequested,
    RunMutation,
    Undo,
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


def test_reload_during_mutation_is_ignored_and_mutation_graph_wins():
    m = _loaded("aaa", working=0)
    armed, [cmd] = update(m, NewChange())
    # Reload while the gate is held is dropped: the mutation's own follow-up
    # reload brings the fresh graph, so a concurrent refresh must not bump the
    # epoch and later discard the mutation's result as "stale".
    after_reload, cmds = update(armed, ReloadRequested())
    assert cmds == []
    assert after_reload is armed  # unchanged — no epoch bump, no LoadGraph
    fresh = _graph("aaa", "bbb", working=1)
    done, _ = update(after_reload, MutationCompleted(cmd.epoch, "Created", fresh, None))
    assert done.pending_mutation is False
    assert done.error == "Created"
    assert done.graph is fresh  # mutation's fresh graph applied, not discarded


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


# --- phase 1a: importability smoke test ------------------------------------


from lajjzy.core import (  # noqa: E402, F401, F811
    # new in phase 1a:
    ApplyResolutions,
    BookmarkDelete,
    BookmarkInputCancel,
    BookmarkInputConfirm,
    BookmarkMove,
    BookmarkMoveConfirm,
    BookmarksLoaded,
    BookmarksLoadFailed,
    ConflictDataLoadFailed,
    ConflictDataLoaded,
    ConflictViewClose,
    HunkPickerClose,
    LoadBookmarks,
    LoadConflictData,
    LoadOpLog,
    OpenBookmarkPicker,
    OpenBookmarkSet,
    OpenConflictView,
    OpenOmnibar,
    OpenOpLog,
    OmnibarAcceptCompletion,
    OmnibarBackspace,
    OmnibarCancel,
    OmnibarInput,
    OmnibarSubmit,
    OpLogClose,
    OpLogLoaded,
    OpLogLoadFailed,
    OpLogRestore,
    Redo,
    Split,
    SplitConfirm,
    SquashPartial,
    SquashPartialConfirm,
    Undo,
)
from lajjzy.backend.types import (  # noqa: E402, F401
    CompletionItem,
    ConflictData,
    HunkRef,
    HunkResolution,
    OpLogEntry,
)


def test_msg_types_importable():
    # Smoke test — just constructing each is enough to verify the import + dataclass shape.
    assert Undo() is not None
    assert Redo() is not None
    assert OpenOmnibar() is not None
    assert OmnibarInput("x") is not None
    assert OmnibarSubmit("mine()") is not None
    assert OpenBookmarkSet() is not None
    assert OpenBookmarkPicker() is not None
    assert BookmarkInputConfirm("main") is not None
    assert BookmarkDelete("main") is not None
    assert BookmarkMove("main") is not None
    assert BookmarkMoveConfirm("main", "ksqxwpml") is not None
    assert OpenOpLog() is not None
    assert OpLogRestore("abc123") is not None
    assert OpLogClose() is not None
    assert OpenConflictView("file.txt") is not None
    assert ConflictViewClose() is not None
    assert ApplyResolutions("file.txt", [HunkResolution.ACCEPT_LEFT]) is not None
    assert Split() is not None
    assert SquashPartial() is not None
    assert SplitConfirm("ksqxwpml", [HunkRef("file.txt", 0)]) is not None
    assert SquashPartialConfirm("ksqxwpml", [HunkRef("file.txt", 0)]) is not None
    # result Msgs
    assert OpLogLoaded([]) is not None
    assert OpLogLoadFailed("boom") is not None
    assert BookmarksLoaded([]) is not None
    assert BookmarksLoadFailed("boom") is not None
    assert ConflictDataLoaded(ConflictData(regions=[])) is not None
    assert ConflictDataLoadFailed("boom") is not None
    # cmds
    assert LoadOpLog() is not None
    assert LoadBookmarks() is not None
    assert LoadConflictData("file.txt") is not None


# --- task 10: Cmd types + Model fields -------------------------------------


def test_loadgraph_has_revset_field():
    cmd = LoadGraph(epoch=1, revset="mine()")
    assert cmd.epoch == 1
    assert cmd.revset == "mine()"


def test_loadgraph_revset_defaults_none():
    cmd = LoadGraph(epoch=1)
    assert cmd.revset is None


def test_loadoplog_cmd():
    cmd = LoadOpLog()
    assert cmd is not None


def test_loadbookmarks_cmd():
    cmd = LoadBookmarks()
    assert cmd is not None


def test_loadconflictdata_cmd():
    cmd = LoadConflictData(path="file.txt")
    assert cmd.path == "file.txt"


def test_model_new_fields_default_none():
    m = Model()
    assert m.op_log_entries is None
    assert m.bookmarks is None
    assert m.revset is None
    assert m.conflict_data is None
    assert m.conflict_path is None
    assert m.modal is None


# --- undo / redo -------------------------------------------------------


def test_undo_starts_mutation():
    m = _loaded("aaa", working=0)
    m1, cmds = update(m, Undo())
    assert m1.pending_mutation is True
    assert m1.graph_epoch == 2
    assert cmds == [RunMutation(2, "undo", ())]


def test_undo_blocked_while_pending():
    m = _loaded("aaa", working=0)
    armed, _ = update(m, NewChange())
    blocked, cmds = update(armed, Undo())
    assert cmds == []
    assert blocked.error == "A mutation is already in progress"
    assert blocked.pending_mutation is True


def test_redo_starts_mutation():
    m = _loaded("aaa", working=0)
    m1, cmds = update(m, Redo())
    assert m1.pending_mutation is True
    assert cmds == [RunMutation(2, "redo", ())]


# --- omnibar -----------------------------------------------------------


def test_open_omnibar_sets_modal():
    m = _loaded("aaa", working=0)
    m1, cmds = update(m, OpenOmnibar())
    assert m1.modal == "omnibar"
    assert cmds == []


def test_omnibar_cancel_clears_modal():
    m = _loaded("aaa", working=0)
    opened, _ = update(m, OpenOmnibar())
    cancelled, _ = update(opened, OmnibarCancel())
    assert cancelled.modal is None


def test_omnibar_submit_with_revset_loads_filtered_graph():
    m = _loaded("aaa", working=0)
    opened, _ = update(m, OpenOmnibar())
    submitted, cmds = update(opened, OmnibarSubmit("mine()"))
    assert submitted.modal is None
    assert submitted.revset == "mine()"
    assert cmds == [LoadGraph(submitted.graph_epoch, "mine()")]


def test_omnibar_submit_none_clears_revset():
    m = _loaded("aaa", working=0)
    # precondition: a revset is active
    pre = replace(m, revset="mine()")
    opened, _ = update(pre, OpenOmnibar())
    submitted, cmds = update(opened, OmnibarSubmit(None))
    assert submitted.modal is None
    assert submitted.revset is None
    assert cmds == [LoadGraph(submitted.graph_epoch, None)]


# --- bookmarks ---------------------------------------------------------


def test_open_bookmark_set_sets_modal():
    m = _loaded("aaa", working=0)
    m1, _ = update(m, OpenBookmarkSet())
    assert m1.modal == "bookmark_input"


def test_open_bookmark_picker_sets_modal_and_loads():
    m = _loaded("aaa", working=0)
    m1, cmds = update(m, OpenBookmarkPicker())
    assert m1.modal == "bookmark_picker"
    assert cmds == [LoadBookmarks()]


def test_bookmark_input_confirm_starts_mutation():
    m = _loaded("aaa", working=0)
    opened, _ = update(m, OpenBookmarkSet())
    confirmed, cmds = update(opened, BookmarkInputConfirm("main"))
    assert confirmed.modal is None
    assert confirmed.pending_mutation is True
    assert cmds == [RunMutation(confirmed.graph_epoch, "bookmark_set", ("aaa", "main"))]


def test_bookmark_input_cancel_clears_modal():
    m = _loaded("aaa", working=0)
    opened, _ = update(m, OpenBookmarkSet())
    cancelled, _ = update(opened, BookmarkInputCancel())
    assert cancelled.modal is None


def test_bookmark_delete_starts_mutation():
    m = _loaded("aaa", working=0)
    m1, cmds = update(m, BookmarkDelete("main"))
    assert m1.pending_mutation is True
    assert cmds == [RunMutation(2, "bookmark_delete", ("main",))]


def test_bookmark_move_confirm_starts_mutation():
    m = _loaded("aaa", "bbb", working=0)
    confirmed, cmds = update(m, BookmarkMoveConfirm("main", "bbb"))
    assert confirmed.pending_mutation is True
    assert cmds == [RunMutation(2, "bookmark_move", ("main", "bbb"))]


def test_bookmarks_loaded_stores_entries():
    m = _loaded("aaa", working=0)
    bms = [Bookmark(name="main", change_id="aaa", change_description="d")]
    m1, _ = update(m, BookmarksLoaded(bms))
    assert m1.bookmarks == bms


def test_bookmarks_load_failed_sets_error():
    m = _loaded("aaa", working=0)
    m1, _ = update(m, BookmarksLoadFailed("boom"))
    assert m1.error == "boom"


# --- operation log -----------------------------------------------------


def test_open_op_log_sets_modal_and_loads():
    m = _loaded("aaa", working=0)
    m1, cmds = update(m, OpenOpLog())
    assert m1.modal == "op_log"
    assert cmds == [LoadOpLog()]


def test_op_log_close_clears_modal():
    m = _loaded("aaa", working=0)
    opened, _ = update(m, OpenOpLog())
    closed, _ = update(opened, OpLogClose())
    assert closed.modal is None


def test_op_log_restore_starts_mutation():
    m = _loaded("aaa", working=0)
    opened, _ = update(m, OpenOpLog())
    restored, cmds = update(opened, OpLogRestore("abc123"))
    assert restored.pending_mutation is True
    assert restored.modal is None
    assert cmds == [RunMutation(restored.graph_epoch, "op_restore", ("abc123",))]


def test_op_log_loaded_stores_entries():
    m = _loaded("aaa", working=0)
    entries = [OpLogEntry(op_id="abc", timestamp="now", description="d")]
    m1, _ = update(m, OpLogLoaded(entries))
    assert m1.op_log_entries == entries


def test_op_log_load_failed_sets_error():
    m = _loaded("aaa", working=0)
    m1, _ = update(m, OpLogLoadFailed("boom"))
    assert m1.error == "boom"


# --- conflict view -----------------------------------------------------


def test_open_conflict_view_sets_modal_and_loads():
    m = _loaded("aaa", working=0)
    m1, cmds = update(m, OpenConflictView("file.txt"))
    assert m1.modal == "conflict_view"
    assert m1.conflict_path == "file.txt"
    assert cmds == [LoadConflictData("file.txt")]


def test_conflict_view_close_clears_modal_and_path():
    m = _loaded("aaa", working=0)
    opened, _ = update(m, OpenConflictView("file.txt"))
    closed, _ = update(opened, ConflictViewClose())
    assert closed.modal is None
    assert closed.conflict_path is None
    assert closed.conflict_data is None


def test_apply_resolutions_starts_mutation():
    m = _loaded("aaa", working=0)
    opened, _ = update(m, OpenConflictView("file.txt"))
    applied, cmds = update(opened, ApplyResolutions("file.txt", [HunkResolution.ACCEPT_LEFT]))
    assert applied.pending_mutation is True
    assert applied.modal is None
    assert cmds == [
        RunMutation(applied.graph_epoch, "resolve", ("file.txt", [HunkResolution.ACCEPT_LEFT]))
    ]


def test_conflict_data_loaded_stores_data():
    m = _loaded("aaa", working=0)
    data = ConflictData(regions=[])
    m1, _ = update(m, ConflictDataLoaded(data))
    assert m1.conflict_data is data


def test_conflict_data_load_failed_sets_error():
    m = _loaded("aaa", working=0)
    m1, _ = update(m, ConflictDataLoadFailed("boom"))
    assert m1.error == "boom"
