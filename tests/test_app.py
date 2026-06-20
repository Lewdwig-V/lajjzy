from pathlib import Path

from lajjzy.app import LajjzyApp
from tests.conftest import jj_required


@jj_required
async def test_app_loads_graph_on_mount(temp_repo: Path):
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test():
        await app.workers.wait_for_complete()
        assert app.graph is not None
        assert app.selected_change_id() is not None


@jj_required
async def test_j_k_move_over_nodes_only(temp_repo: Path):
    # Build a 3-change stack so there is more than one node.
    import subprocess

    subprocess.run(["jj", "new", "-m", "second"], cwd=temp_repo, check=True, capture_output=True)
    subprocess.run(["jj", "new", "-m", "third"], cwd=temp_repo, check=True, capture_output=True)

    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test() as pilot:
        await app.workers.wait_for_complete()
        start = app.cursor
        assert start in app.graph.node_indices
        await pilot.press("j")
        assert app.cursor in app.graph.node_indices
        assert app.cursor != start
        await pilot.press("k")
        assert app.cursor == start


@jj_required
async def test_detail_panel_shows_selected_change_files(temp_repo: Path):
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test():
        await app.workers.wait_for_complete()
        from lajjzy.widgets.detail import DetailPanel

        panel = app.query_one(DetailPanel)
        rendered = panel.render()
        # working copy has a.txt added/modified in the fixture
        assert "a.txt" in str(rendered)


@jj_required
async def test_enter_opens_diff_then_esc_returns(temp_repo: Path):
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test() as pilot:
        await app.workers.wait_for_complete()
        from lajjzy.widgets.detail import DetailPanel

        panel = app.query_one(DetailPanel)
        await pilot.press("tab")  # focus detail
        await pilot.press("enter")  # open diff for first file
        await app.workers.wait_for_complete()
        assert panel.mode == "diff"
        await pilot.press("escape")
        assert panel.mode == "files"


@jj_required
async def test_press_n_creates_change(temp_repo: Path):
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test() as pilot:
        await app.workers.wait_for_complete()
        before = len(app.graph.details)
        await pilot.press("n")
        await app.workers.wait_for_complete()
        assert len(app.graph.details) == before + 1
