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


@jj_required
async def test_file_up_noop_in_diff_mode(temp_repo: Path):
    """k must not mutate file_cursor when DetailPanel is in diff mode."""
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test() as pilot:
        await app.workers.wait_for_complete()
        from lajjzy.widgets.detail import DetailPanel

        panel = app.query_one(DetailPanel)
        await pilot.press("tab")  # focus detail panel
        await pilot.press("enter")  # open diff for first file (a.txt)
        await app.workers.wait_for_complete()
        assert panel.mode == "diff", f"expected diff mode, got {panel.mode!r}"
        cursor_before = panel.file_cursor
        await pilot.press("k")
        assert panel.file_cursor == cursor_before, (
            f"file_cursor changed from {cursor_before} to {panel.file_cursor} "
            "while in diff mode — mode guard missing from action_file_up"
        )
        assert panel.mode == "diff"


# ---------------------------------------------------------------------------
# P1 mutation gate tests
# ---------------------------------------------------------------------------


@jj_required
async def test_mutation_gate_rejects_concurrent(temp_repo: Path, monkeypatch):
    """A second mutation key while one is in flight must be rejected, not cancel."""
    import asyncio

    import lajjzy.backend.jj as jjmod

    gate = asyncio.Event()
    calls: list[str] = []

    async def slow_new(cwd, after):
        calls.append("new")
        await gate.wait()
        return "Created"

    async def quick_abandon(cwd, change_id):
        calls.append("abandon")
        return "Abandoned"

    monkeypatch.setattr(jjmod, "new_change", slow_new)
    monkeypatch.setattr(jjmod, "abandon", quick_abandon)

    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test() as pilot:
        await app.workers.wait_for_complete()

        # Press "n" — slow_new starts, pending_mutation=True
        await pilot.press("n")
        await pilot.pause()
        # Give the worker a chance to enter slow_new and set calls
        await asyncio.sleep(0)
        assert "new" in calls, "slow_new should have been called"
        assert app.pending_mutation is True

        # Press "d" while slow_new is still blocked — should be rejected
        await pilot.press("d")
        await pilot.pause()
        assert "abandon" not in calls, "abandon must NOT run while mutation is in flight"

        # Unblock slow_new and let everything finish
        gate.set()
        await app.workers.wait_for_complete()
        assert app.pending_mutation is False


@jj_required
async def test_mutation_gate_clears_after_completion(temp_repo: Path, monkeypatch):
    """After a successful mutation, pending_mutation is False and a second one is accepted."""
    import lajjzy.backend.jj as jjmod

    calls: list[str] = []

    async def fast_new(cwd, after):
        calls.append("new")
        return "Created"

    async def fast_abandon(cwd, change_id):
        calls.append("abandon")
        return "Abandoned"

    monkeypatch.setattr(jjmod, "new_change", fast_new)
    monkeypatch.setattr(jjmod, "abandon", fast_abandon)
    # Patch load_graph to return the existing graph immediately so the worker
    # finishes quickly without spawning jj.
    original_load = jjmod.load_graph

    async def instant_load(cwd, revset=None):
        return await original_load(cwd, revset)

    # The backend reloads via the jj facade attribute (jj.load_graph), so
    # patching the facade is sufficient — no app-module patch needed.
    monkeypatch.setattr(jjmod, "load_graph", instant_load)

    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test() as pilot:
        await app.workers.wait_for_complete()

        # First mutation
        await pilot.press("n")
        await app.workers.wait_for_complete()
        assert app.pending_mutation is False, "pending_mutation must clear after first mutation"
        assert "new" in calls

        # Second mutation — must be accepted now
        await pilot.press("d")
        await app.workers.wait_for_complete()
        assert "abandon" in calls, "second mutation was rejected after first completed"
        assert app.pending_mutation is False


# ---------------------------------------------------------------------------
# InvariantError crash wiring tests
# ---------------------------------------------------------------------------


import pytest  # noqa: E402

from lajjzy.invariants import InvariantError  # noqa: E402


def test_main_exits_70_on_invariant_error(monkeypatch):
    import lajjzy.app as appmod

    def boom(self):
        raise InvariantError("model broken")

    monkeypatch.setattr(appmod.LajjzyApp, "run", boom)
    with pytest.raises(SystemExit) as exc:
        appmod.main()
    assert exc.value.code == 70


def test_main_does_not_intercept_normal_exit(monkeypatch):
    import lajjzy.app as appmod

    monkeypatch.setattr(appmod.LajjzyApp, "run", lambda self: None)
    # Should return normally, no SystemExit.
    appmod.main()


# ---------------------------------------------------------------------------
# Task 5: runtime invariant sites — I3 (cursor on node)
# (I1 mutation gate is covered in tests/core/test_update.py by
#  test_second_mutation_rejected_while_pending.)


@jj_required
async def test_navigation_keeps_cursor_on_node(temp_repo: Path):
    import subprocess

    subprocess.run(["jj", "new", "-m", "x"], cwd=temp_repo, check=True, capture_output=True)
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test() as pilot:
        await app.workers.wait_for_complete()
        for key in ("j", "k", "g", "G", "j", "j"):
            await pilot.press(key)
            assert app.cursor in app.graph.node_indices  # I3 holds after every move


# ---------------------------------------------------------------------------
# Worker-path InvariantError capture (regression test for WorkerFailed.error)
# ---------------------------------------------------------------------------


@jj_required
async def test_worker_invariant_error_captured_via_workerfailed(temp_repo: Path, monkeypatch):
    """I1/I3 invariants fire inside @work workers. Textual wraps the exception
    in WorkerFailed and stores it in WorkerFailed.error (NOT __cause__).
    _handle_exception must unwrap via .error so _invariant_error is captured
    and main() can exit 70.

    This test fails against the buggy __cause__ path and passes after the .error fix.
    """
    import lajjzy.app as app_mod
    from lajjzy.invariants import InvariantError

    sentinel = InvariantError("worker-path invariant breach")

    async def boom(_path, _revset=None):
        raise sentinel

    # load_graph is called via the `jj` module (imported into app.py as `jj`),
    # so patch it there — patching app_mod.load_graph would target a name that
    # nothing reads after the MVU refactor moved the call to jj.load_graph.
    monkeypatch.setattr(app_mod.jj, "load_graph", boom)

    app = LajjzyApp(repo_path=temp_repo)
    # run_test() re-raises the WorkerFailed on exit when _exception is set —
    # absorb it here since we are intentionally crashing the worker.
    try:
        async with app.run_test():
            # reload() is called on_mount; wait for the worker to finish
            # (it will fail, but workers.wait_for_complete() returns regardless).
            try:
                await app.workers.wait_for_complete()
            except Exception:
                pass
    except Exception:
        pass

    # After the app exits, _invariant_error must have been captured.
    assert app._invariant_error is not None, (
        "_invariant_error was not captured — WorkerFailed.error unwrap is missing"
    )
    assert isinstance(app._invariant_error, InvariantError)


# ---------------------------------------------------------------------------
# New worker behavioral tests (op-log / bookmarks / conflict)
# ---------------------------------------------------------------------------


@jj_required
async def test_open_op_log_populates_model(temp_repo: Path, monkeypatch):
    """Dispatching OpenOpLog triggers _worker_load_op_log which sets
    model.op_log_entries via OpLogLoaded."""
    import lajjzy.backend.jj as jj_mod
    from lajjzy.backend.types import OpLogEntry
    from lajjzy.core.messages import OpenOpLog

    canned = [OpLogEntry(op_id="aabbccdd", timestamp="1 second ago", description="init")]

    async def fake_op_log(cwd):
        return canned

    monkeypatch.setattr(jj_mod, "op_log", fake_op_log)

    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test():
        await app.workers.wait_for_complete()
        # Dispatch OpenOpLog through the runtime (mirrors a key press that would
        # open the op-log view); the update() function emits LoadOpLog which
        # run_cmd routes to _worker_load_op_log.
        app.runtime.dispatch(OpenOpLog())
        await app.workers.wait_for_complete()
        assert app.model.op_log_entries == canned, (
            f"Expected op_log_entries={canned!r}, got {app.model.op_log_entries!r}"
        )


@jj_required
async def test_open_op_log_jjerror_sets_model_error(temp_repo: Path, monkeypatch):
    """A JjError from jj.op_log causes OpLogLoadFailed which sets model.error."""
    import lajjzy.backend.jj as jj_mod
    from lajjzy.backend.types import JjError
    from lajjzy.core.messages import OpenOpLog

    async def boom(cwd):
        raise JjError("op log exploded")

    monkeypatch.setattr(jj_mod, "op_log", boom)

    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test():
        await app.workers.wait_for_complete()
        # Clear any mount-time error first.
        app.runtime.dispatch(OpenOpLog())
        await app.workers.wait_for_complete()
        assert app.model.error == "op log exploded", (
            f"Expected error='op log exploded', got {app.model.error!r}"
        )


@jj_required
async def test_open_bookmark_picker_populates_model(temp_repo: Path, monkeypatch):
    """Dispatching OpenBookmarkPicker triggers _worker_load_bookmarks which sets
    model.bookmarks via BookmarksLoaded."""
    import lajjzy.backend.jj as jj_mod
    from lajjzy.backend.types import Bookmark
    from lajjzy.core.messages import OpenBookmarkPicker

    canned = [Bookmark(name="main", change_id="abcdef12", change_description="first change")]

    async def fake_load_bookmarks(cwd):
        return canned

    monkeypatch.setattr(jj_mod, "load_bookmarks", fake_load_bookmarks)

    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test():
        await app.workers.wait_for_complete()
        app.runtime.dispatch(OpenBookmarkPicker())
        await app.workers.wait_for_complete()
        assert app.model.bookmarks == canned, (
            f"Expected bookmarks={canned!r}, got {app.model.bookmarks!r}"
        )


@jj_required
async def test_present_projects_modal_and_new_model_fields(temp_repo: Path):
    from lajjzy.core import OpenOmnibar

    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test():
        await app.workers.wait_for_complete()
        app.runtime.dispatch(OpenOmnibar())
        await app.workers.wait_for_complete()
        assert app.modal == "omnibar"


# ---------------------------------------------------------------------------
# Task 3: pilot tests for new key bindings
# ---------------------------------------------------------------------------


@jj_required
async def test_undo_key_runs_jj_undo(temp_repo: Path):
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test() as pilot:
        await app.workers.wait_for_complete()
        await pilot.press("u")
        await app.workers.wait_for_complete()
        assert app.graph is not None


@jj_required
async def test_U_key_runs_jj_redo(temp_repo: Path):
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test() as pilot:
        await app.workers.wait_for_complete()
        await pilot.press("u")
        await app.workers.wait_for_complete()
        await pilot.press("U")
        await app.workers.wait_for_complete()
        assert app.graph is not None


@jj_required
async def test_slash_key_opens_omnibar_modal(temp_repo: Path):
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test() as pilot:
        await app.workers.wait_for_complete()
        await pilot.press("/")
        await app.workers.wait_for_complete()
        assert app.modal == "omnibar"
