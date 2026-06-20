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


@jj_required
async def test_ensure_working_copy_already_at_target_is_noop(temp_repo: Path):
    """Calling ensure_working_copy with the current @ change_id returns True
    without moving the working copy (the no-op fast-path)."""
    import subprocess

    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test():
        await app.workers.wait_for_complete()
        # The working-copy index and its change_id as known by the loaded graph.
        wc_index = app.graph.working_copy_index
        assert wc_index is not None
        target = app.graph.lines[wc_index].change_id
        assert target is not None

        result = await app.ensure_working_copy(target)

        assert result is True
        # Working copy must not have moved: ask jj directly.
        out = subprocess.run(
            ["jj", "log", "--no-graph", "-r", "@", "-T", "change_id.short()"],
            cwd=temp_repo,
            check=True,
            capture_output=True,
            text=True,
        )
        actual_wc = out.stdout.strip()
        assert actual_wc == target, (
            f"Working copy moved from {target!r} to {actual_wc!r} — "
            "ensure_working_copy called jj edit when it should have been a no-op"
        )


@jj_required
async def test_ensure_working_copy_switches_to_target(temp_repo: Path):
    """ensure_working_copy with a non-@ change_id runs jj edit and returns True."""
    import subprocess

    # Create a second change so there is a non-@ change to switch to.
    subprocess.run(["jj", "new", "-m", "second"], cwd=temp_repo, check=True, capture_output=True)
    # Now @ is the new empty change; the original "first change" is the parent.

    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test():
        await app.workers.wait_for_complete()

        # Identify a change that is NOT the working copy.
        wc_index = app.graph.working_copy_index
        assert wc_index is not None
        wc_change_id = app.graph.lines[wc_index].change_id

        non_wc = next(
            (
                app.graph.lines[i].change_id
                for i in app.graph.node_indices
                if i != wc_index and app.graph.lines[i].change_id is not None
            ),
            None,
        )
        assert non_wc is not None, "Expected at least one non-@ change in the graph"
        assert non_wc != wc_change_id

        result = await app.ensure_working_copy(non_wc)

        assert result is True
        # Verify via jj that @ actually moved.
        out = subprocess.run(
            ["jj", "log", "--no-graph", "-r", "@", "-T", "change_id.short()"],
            cwd=temp_repo,
            check=True,
            capture_output=True,
            text=True,
        )
        actual_wc = out.stdout.strip()
        assert actual_wc == non_wc, (
            f"Expected working copy to be {non_wc!r} after ensure_working_copy, "
            f"but jj reports {actual_wc!r}"
        )


@jj_required
async def test_ensure_working_copy_bad_change_returns_false(temp_repo: Path):
    """ensure_working_copy with an invalid change_id catches JjError, sets
    app.error, and returns False."""
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test():
        await app.workers.wait_for_complete()
        assert app.error is None  # clean slate after mount

        result = await app.ensure_working_copy("zzzzzzzzzzzz")

        assert result is False, "Expected False for an invalid change_id"
        assert app.error is not None, (
            "Expected app.error to be set after a failed ensure_working_copy"
        )


@jj_required
async def test_status_bar_shows_error(temp_repo: Path):
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test():
        await app.workers.wait_for_complete()
        from lajjzy.widgets.status_bar import StatusBar

        app.error = "boom"
        bar = app.query_one(StatusBar)
        assert "boom" in str(bar.render())


@jj_required
async def test_describe_without_editor_sets_error(temp_repo: Path, monkeypatch):
    monkeypatch.delenv("EDITOR", raising=False)
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test() as pilot:
        await app.workers.wait_for_complete()
        await pilot.press("e")
        assert app.error == "No $EDITOR set"


@jj_required
async def test_mutation_jjerror_surfaces_to_app_error(temp_repo: Path, monkeypatch):
    from lajjzy.backend.types import JjError
    import lajjzy.backend.jj as jj_mod

    async def boom(*args, **kwargs):
        raise JjError("boom")

    monkeypatch.setattr(jj_mod, "abandon", boom)

    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test() as pilot:
        await app.workers.wait_for_complete()
        graph_before = app.graph
        await pilot.press("d")
        await app.workers.wait_for_complete()
        assert app.error == "boom"
        # Graph must not be corrupted (still the same object or at least valid).
        assert app.graph is graph_before or app.graph is not None


@jj_required
async def test_rebase_cancel_via_escape_clears_mode(temp_repo: Path):
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test() as pilot:
        await app.workers.wait_for_complete()
        await pilot.press("r")
        assert app.rebase_source is not None
        await pilot.press("escape")
        assert app.rebase_source is None
        assert app.error == "Rebase cancelled"


@jj_required
async def test_rebase_confirm_same_dest_cancels(temp_repo: Path):
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test() as pilot:
        await app.workers.wait_for_complete()
        await pilot.press("r")
        assert app.rebase_source is not None
        await pilot.press("enter")
        assert app.rebase_source is None
        assert app.error is not None
        assert "cancelled" in app.error.lower()
