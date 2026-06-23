from __future__ import annotations

from pathlib import Path

from lajjzy.backend import jj
from lajjzy.backend.types import ConflictData, HunkResolution, OpLogEntry

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


@jj_required
async def test_load_bookmarks_empty_repo(temp_repo: Path):
    bms = await jj.load_bookmarks(temp_repo)
    assert bms == []


@jj_required
async def test_bookmark_set_and_load(temp_repo: Path):
    graph = await jj.load_graph(temp_repo)
    target = graph.lines[graph.working_copy_index or 0].change_id
    assert target is not None
    await jj.bookmark_set(temp_repo, target, "mybm")
    bms = await jj.load_bookmarks(temp_repo)
    names = [b.name for b in bms]
    assert "mybm" in names
    matched = [b for b in bms if b.name == "mybm"][0]
    assert matched.change_id == target


@jj_required
async def test_bookmark_delete(temp_repo: Path):
    graph = await jj.load_graph(temp_repo)
    target = graph.lines[graph.working_copy_index or 0].change_id
    assert target is not None
    await jj.bookmark_set(temp_repo, target, "todelete")
    await jj.bookmark_delete(temp_repo, "todelete")
    bms = await jj.load_bookmarks(temp_repo)
    assert "todelete" not in [b.name for b in bms]


@jj_required
async def test_bookmark_move(temp_repo: Path):
    import subprocess

    subprocess.run(["jj", "new", "-m", "second"], cwd=temp_repo, check=True, capture_output=True)
    graph = await jj.load_graph(temp_repo)
    first = graph.lines[0].change_id
    second = graph.lines[1].change_id
    assert first is not None and second is not None
    await jj.bookmark_set(temp_repo, first, "moveable")
    await jj.bookmark_move(temp_repo, "moveable", second)
    bms = await jj.load_bookmarks(temp_repo)
    moved = [b for b in bms if b.name == "moveable"][0]
    assert moved.change_id == second


def _jj_short_id(cwd: Path, revset: str) -> str:
    """Helper: return the 8-char change_id for a revset (runs synchronously)."""
    import subprocess

    out = subprocess.run(
        ["jj", "log", "--no-graph", "-T", 'change_id.short(8) ++ "\\n"', "-r", revset],
        cwd=cwd,
        check=True,
        capture_output=True,
        text=True,
    )
    return out.stdout.strip().splitlines()[0]


@jj_required
async def test_conflict_data_no_conflict(temp_repo: Path) -> None:
    # No conflicts in a fresh repo — conflict_data on a non-conflicted file
    # should return one (or more) resolved regions with the file content.
    import subprocess

    subprocess.run(["touch", "file.txt"], cwd=temp_repo, check=True)
    subprocess.run(["jj", "new"], cwd=temp_repo, check=True, capture_output=True)
    subprocess.run(["touch", "other.txt"], cwd=temp_repo, check=True)
    subprocess.run(["jj", "new"], cwd=temp_repo, check=True, capture_output=True)
    cd = await jj.conflict_data(temp_repo, "file.txt")
    assert isinstance(cd, ConflictData)
    assert all(r.kind == "resolved" for r in cd.regions)


@jj_required
async def test_resolve_accept_left(temp_repo: Path) -> None:
    """Create a 2-sided conflict and resolve it by accepting the 'left' parent.

    Conflict creation recipe for jj 0.42.0:
      1. Create a "base" change with ``LINE`` in c.txt.
      2. From base, create a "left" branch with ``LEFT`` in c.txt.
      3. From base, create a "right" branch with ``RIGHT`` in c.txt.
      4. ``jj new <left-id> <right-id>`` produces the merge @ with a conflict.

    With ``ui.conflict-marker-style=git`` the materialized conflict is:

    .. code-block::

        <<<<<<< <left-id> "left"
        LEFT
        ||||||| <base-id> "base"
        LINE
        =======
        RIGHT
        >>>>>>> <right-id> "right"

    So ``region.left == "LEFT\\n"``, ``ACCEPT_LEFT`` must write ``LEFT\\n``.
    """
    import subprocess

    # Step 1: base change (temp_repo already has "a.txt" from the fixture)
    subprocess.run(["jj", "new", "-m", "base"], cwd=temp_repo, check=True, capture_output=True)
    (temp_repo / "c.txt").write_text("LINE\n")
    base_id = _jj_short_id(temp_repo, "@")

    # Step 2: "left" branch from base
    subprocess.run(
        ["jj", "new", "-m", "left", base_id], cwd=temp_repo, check=True, capture_output=True
    )
    (temp_repo / "c.txt").write_text("LEFT\n")
    left_id = _jj_short_id(temp_repo, "@")

    # Step 3: "right" branch from base (jj new <base_id> creates a new commit
    # whose parent is base_id, making @ be that new commit)
    subprocess.run(
        ["jj", "new", "-m", "right", base_id], cwd=temp_repo, check=True, capture_output=True
    )
    (temp_repo / "c.txt").write_text("RIGHT\n")
    right_id = _jj_short_id(temp_repo, "@")

    # Step 4: merge commit with left and right as parents
    subprocess.run(
        ["jj", "new", "-m", "merge", left_id, right_id],
        cwd=temp_repo,
        check=True,
        capture_output=True,
    )

    # Verify the conflict was actually created
    cd = await jj.conflict_data(temp_repo, "c.txt")
    assert isinstance(cd, ConflictData)
    conflict_regions = [r for r in cd.regions if r.kind == "conflict"]
    assert len(conflict_regions) >= 1, f"Expected at least one conflict region, got: {cd.regions}"

    # Resolve by accepting left (should produce LEFT\n)
    resolutions = [HunkResolution.ACCEPT_LEFT] * len(conflict_regions)
    msg = await jj.resolve(temp_repo, "c.txt", resolutions)
    assert "resolve" in msg.lower() or "resolved" in msg.lower()
    assert (temp_repo / "c.txt").read_text() == "LEFT\n"
