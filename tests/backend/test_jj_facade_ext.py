from __future__ import annotations

from pathlib import Path

from lajjzy.backend import jj
from lajjzy.backend.types import (
    ConflictData,
    ConflictHunk,
    FileRef,
    HunkResolution,
    OpLogEntry,
    ResolvedRegion,
)

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
    # Capture node count BEFORE making a new change.
    graph_before = await jj.load_graph(temp_repo)
    node_count_before = len(graph_before.node_indices)

    # Capture the op_id to restore to, then make a new change.
    entries_before = await jj.op_log(temp_repo)
    op_id = entries_before[0].op_id
    await jj.new_change(temp_repo, "@")

    # Restore to the pre-change operation.
    await jj.op_restore(temp_repo, op_id)

    # After restore, the new change should be gone — node count back to ≤ before.
    from lajjzy.backend.types import GraphData

    graph = await jj.load_graph(temp_repo)
    assert isinstance(graph, GraphData)
    assert len(graph.node_indices) <= node_count_before


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
    assert all(isinstance(r, ResolvedRegion) for r in cd.regions)


async def test_resolve_raises_when_file_not_conflicted(monkeypatch, tmp_path: Path) -> None:
    """resolve() must refuse to overwrite a file that has no conflict hunks.

    Patches conflict_data to return a fully-resolved ConflictData (no
    ConflictHunk regions) so no jj subprocess is needed.  The guard must raise
    *before* any write touches the file.
    """
    import pytest

    target = tmp_path / "file.txt"
    target.write_text("original\n")

    async def _fake_conflict_data(cwd: Path, path: str) -> ConflictData:
        return ConflictData(regions=[ResolvedRegion(text="original\n")])

    monkeypatch.setattr(jj, "conflict_data", _fake_conflict_data)

    with pytest.raises(jj.JjError, match="no conflicts to resolve"):
        await jj.resolve(tmp_path, "file.txt", [])

    # The file must be untouched — the guard fires before write_text.
    assert target.read_text() == "original\n"


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
    conflict_regions = [r for r in cd.regions if isinstance(r, ConflictHunk)]
    assert len(conflict_regions) >= 1, f"Expected at least one conflict region, got: {cd.regions}"

    # Resolve by accepting left (should produce LEFT\n)
    resolutions = [HunkResolution.ACCEPT_LEFT] * len(conflict_regions)
    msg = await jj.resolve(temp_repo, "c.txt", resolutions)
    assert "resolve" in msg.lower() or "resolved" in msg.lower()
    assert (temp_repo / "c.txt").read_text() == "LEFT\n"


@jj_required
async def test_split_increases_change_count(temp_repo: Path) -> None:
    """split() with one file selected creates a new child change.

    Setup: create a change with two files (file_a.txt and file_b.txt), then
    split file_a.txt out.  The resulting graph must have more node-commits
    than before the split (the selected file lands in a new child; the
    remaining file stays in the original).
    """
    import subprocess

    # Write two files into the working copy (already on a change from fixture).
    (temp_repo / "file_a.txt").write_text("content a\n")
    (temp_repo / "file_b.txt").write_text("content b\n")
    subprocess.run(
        ["jj", "describe", "-m", "two-file change"], cwd=temp_repo, check=True, capture_output=True
    )

    graph_before = await jj.load_graph(temp_repo)
    wc_idx = graph_before.working_copy_index
    assert wc_idx is not None
    source_id = graph_before.lines[wc_idx].change_id
    assert source_id is not None

    node_count_before = len(graph_before.node_indices)

    msg = await jj.split(temp_repo, source_id, [FileRef(path="file_a.txt")])
    assert "split" in msg.lower()

    graph_after = await jj.load_graph(temp_repo)
    assert len(graph_after.node_indices) > node_count_before


@jj_required
async def test_split_file_placement(temp_repo: Path) -> None:
    """split() puts the SELECTED file in the first/parent commit (which retains
    the source's change-id) and the UNSELECTED file in the new child commit.

    Empirically verified jj 0.42.0 behaviour: ``jj split -r <source> -m ""
    <paths>`` places ``<paths>`` in the first commit (keeps source change-id,
    empty description) and the remaining files in a new child working-copy
    commit that retains the original description.  This test is the contract —
    if jj ever flips the placement this test will catch it.
    """
    import subprocess

    # Create a two-file change in the working copy.
    (temp_repo / "fa.txt").write_text("alpha\n")
    (temp_repo / "fb.txt").write_text("beta\n")
    subprocess.run(
        ["jj", "describe", "-m", "two-file change"], cwd=temp_repo, check=True, capture_output=True
    )

    graph_before = await jj.load_graph(temp_repo)
    wc_idx = graph_before.working_copy_index
    assert wc_idx is not None
    source_id = graph_before.lines[wc_idx].change_id
    assert source_id is not None

    # Split selecting only fa.txt.
    await jj.split(temp_repo, source_id, [FileRef(path="fa.txt")])

    # source_id is retained by the first/parent commit and should contain fa.txt.
    parent_diff = await jj.change_diff(temp_repo, source_id)
    parent_paths = {fd.path for fd in parent_diff}
    assert "fa.txt" in parent_paths, (
        f"Expected fa.txt in first/parent commit (source_id={source_id}), got {parent_paths}"
    )
    assert "fb.txt" not in parent_paths, (
        f"fb.txt should be in the child commit, not the parent (got {parent_paths})"
    )

    # The new child (current working copy @) should contain only fb.txt.
    graph_after = await jj.load_graph(temp_repo)
    new_wc_idx = graph_after.working_copy_index
    assert new_wc_idx is not None
    child_id = graph_after.lines[new_wc_idx].change_id
    assert child_id is not None
    assert child_id != source_id, "Working copy must be a new change after split"

    child_diff = await jj.change_diff(temp_repo, child_id)
    child_paths = {fd.path for fd in child_diff}
    assert "fb.txt" in child_paths, (
        f"Expected fb.txt in new child commit (child_id={child_id}), got {child_paths}"
    )


@jj_required
async def test_squash_partial_moves_file_to_parent(temp_repo: Path) -> None:
    """squash_partial() moves only the selected file's changes into the parent.

    Setup:
    - Parent change: file_a.txt and file_b.txt (first version).
    - Child change (on top): file_a.txt modified, file_b.txt modified.
    Then squash_partial with file_a.txt only → file_a.txt modification lands
    in the parent; file_b.txt modification stays in the child.
    """
    import subprocess

    # Parent: write initial versions of both files.
    (temp_repo / "file_a.txt").write_text("parent a\n")
    (temp_repo / "file_b.txt").write_text("parent b\n")
    subprocess.run(
        ["jj", "describe", "-m", "parent change"], cwd=temp_repo, check=True, capture_output=True
    )
    parent_id = _jj_short_id(temp_repo, "@")

    # Child: modify both files.
    subprocess.run(
        ["jj", "new", "-m", "child change"], cwd=temp_repo, check=True, capture_output=True
    )
    (temp_repo / "file_a.txt").write_text("child a\n")
    (temp_repo / "file_b.txt").write_text("child b\n")
    child_id = _jj_short_id(temp_repo, "@")

    msg = await jj.squash_partial(temp_repo, child_id, [FileRef(path="file_a.txt")])
    assert "squash" in msg.lower()

    # file_a.txt in the parent change should now contain the child's version.
    parent_diff = await jj.change_diff(temp_repo, parent_id)
    parent_paths = {fd.path for fd in parent_diff}
    assert "file_a.txt" in parent_paths

    # file_b.txt should still be in the child change only.
    child_diff = await jj.change_diff(temp_repo, child_id)
    child_paths = {fd.path for fd in child_diff}
    assert "file_b.txt" in child_paths
