import pytest

from lajjzy.backend.jj import abandon, change_diff, describe, edit_change, load_graph, new_change, rebase_single, run_jj, squash
from lajjzy.backend.types import JjError
from tests.conftest import jj_required


@jj_required
async def test_run_jj_returns_stdout(temp_repo):
    out = await run_jj(["log", "--no-graph", "-T", "change_id.short()"], temp_repo)
    assert out.strip() != ""


@jj_required
async def test_run_jj_raises_on_bad_args(temp_repo):
    with pytest.raises(JjError):
        await run_jj(["log", "-r", "nonexistent_revset_xyz"], temp_repo)


@jj_required
async def test_load_graph_has_working_copy_and_details(temp_repo):
    g = await load_graph(temp_repo)
    assert g.working_copy_index is not None
    assert len(g.details) >= 1
    wc_line = g.lines[g.working_copy_index]
    assert wc_line.change_id in g.details


@jj_required
async def test_load_graph_revset_filters(temp_repo):
    g = await load_graph(temp_repo, revset="root()")
    assert len(g.details) == 1


@jj_required
async def test_change_diff_returns_files(temp_repo):
    g = await load_graph(temp_repo)
    wc = g.lines[g.working_copy_index].change_id
    files = await change_diff(temp_repo, wc)
    assert isinstance(files, list)


@jj_required
async def test_new_change_adds_node(temp_repo):
    before = len((await load_graph(temp_repo)).details)
    wc = (await load_graph(temp_repo)).lines[
        (await load_graph(temp_repo)).working_copy_index].change_id
    await new_change(temp_repo, wc)
    after = len((await load_graph(temp_repo)).details)
    assert after == before + 1


@jj_required
async def test_edit_moves_working_copy(temp_repo):
    import subprocess
    subprocess.run(["jj", "new", "-m", "child"], cwd=temp_repo, check=True,
                   capture_output=True)
    g = await load_graph(temp_repo)
    # pick a non-@ node (the parent)
    parents = g.details[g.lines[g.working_copy_index].change_id].parents
    assert parents
    await edit_change(temp_repo, parents[0])
    g2 = await load_graph(temp_repo)
    assert g2.lines[g2.working_copy_index].change_id == parents[0]


@jj_required
async def test_describe_sets_message(temp_repo):
    g = await load_graph(temp_repo)
    wc = g.lines[g.working_copy_index].change_id
    await describe(temp_repo, wc, "a brand new message")
    g2 = await load_graph(temp_repo)
    assert g2.details[wc].description == "a brand new message"


@jj_required
async def test_squash_collapses_into_parent(temp_repo):
    import subprocess
    # Create a child with content, then switch @ to a new grandchild so the
    # child is no longer the working copy.  Squashing a non-@ change lets jj
    # truly abandon it (squashing @ replaces it with a new empty commit, which
    # keeps the count stable — not a bug, just jj semantics on 0.42).
    subprocess.run(["jj", "new", "-m", "child"], cwd=temp_repo, check=True,
                   capture_output=True)
    (temp_repo / "b.txt").write_text("more\n")
    g = await load_graph(temp_repo)
    child = g.lines[g.working_copy_index].change_id
    # Move @ to a grandchild so child becomes a non-@ intermediate commit.
    subprocess.run(["jj", "new", "-m", "grandchild"], cwd=temp_repo, check=True,
                   capture_output=True)
    g2 = await load_graph(temp_repo)
    before = len(g2.details)
    await squash(temp_repo, child)
    after = len((await load_graph(temp_repo)).details)
    assert after == before - 1


@jj_required
async def test_rebase_single_reparents(temp_repo):
    import subprocess
    # root → A; create sibling B off root; rebase B onto A
    subprocess.run(["jj", "new", "-m", "A"], cwd=temp_repo, check=True,
                   capture_output=True)
    g = await load_graph(temp_repo)
    a = g.lines[g.working_copy_index].change_id
    subprocess.run(["jj", "new", "root()", "-m", "B"], cwd=temp_repo,
                   check=True, capture_output=True)
    g = await load_graph(temp_repo)
    b = g.lines[g.working_copy_index].change_id
    await rebase_single(temp_repo, b, a)
    g2 = await load_graph(temp_repo)
    assert a in g2.details[b].parents


@jj_required
async def test_abandon_removes_node(temp_repo):
    import subprocess
    # Create a child so "first change" is a non-WC parent — abandoning it is safe.
    subprocess.run(["jj", "new", "-m", "child"], cwd=temp_repo, check=True,
                   capture_output=True)
    g = await load_graph(temp_repo)
    # Find a non-WC, non-root change to abandon ("first change").
    wc_id = g.lines[g.working_copy_index].change_id
    root_id = next(
        cid for cid, det in g.details.items()
        if not det.parents  # root has no parents
    )
    target = next(
        cid for cid in g.details
        if cid != wc_id and cid != root_id
    )
    before = len(g.details)
    await abandon(temp_repo, target)
    after = len((await load_graph(temp_repo)).details)
    assert after == before - 1
