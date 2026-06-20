import pytest

from lajjzy.backend.jj import abandon, change_diff, load_graph, new_change, run_jj
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
