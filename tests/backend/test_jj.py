import pytest

from lajjzy.backend.jj import change_diff, load_graph, run_jj
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
