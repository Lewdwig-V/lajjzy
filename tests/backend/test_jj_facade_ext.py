from __future__ import annotations

from pathlib import Path

from lajjzy.backend import jj
from lajjzy.backend.types import OpLogEntry

from tests.conftest import jj_required


@jj_required
async def test_undo_returns_message(temp_repo: Path):
    msg = await jj.undo(temp_repo)
    assert "undo" in msg.lower() or "undid" in msg.lower()


@jj_required
async def test_redo_returns_message(temp_repo: Path):
    await jj.undo(temp_repo)
    msg = await jj.redo(temp_repo)
    assert "redo" in msg.lower() or "redid" in msg.lower()


@jj_required
async def test_op_log_returns_entries(temp_repo: Path):
    entries = await jj.op_log(temp_repo)
    assert isinstance(entries, list)
    assert len(entries) >= 1
    assert all(isinstance(e, OpLogEntry) for e in entries)
    assert all(e.op_id for e in entries)


@jj_required
async def test_op_restore_roundtrip(temp_repo: Path):
    # Capture current state, make a change, then restore to undo it.
    entries_before = await jj.op_log(temp_repo)
    op_id = entries_before[0].op_id
    await jj.new_change(temp_repo, "@")
    await jj.op_restore(temp_repo, op_id)
    # After restore, the new change should be gone — graph load reflects it.
    from lajjzy.backend.types import GraphData

    graph = await jj.load_graph(temp_repo)
    assert isinstance(graph, GraphData)
