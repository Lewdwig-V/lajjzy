import pytest

from lajjzy.backend.jj import run_jj
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
