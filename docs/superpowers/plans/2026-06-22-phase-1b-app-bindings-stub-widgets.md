# Daily-driver essentials — Phase 1b: app.py bindings, workers, stub widgets Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the phase-1a core seams into `app.py` — new keybindings, `run_cmd` branches, workers for op-log/bookmarks/conflict-data, `_OPS` entries for the new mutation kinds, modal mounting, and six stub widget files that render empty. Ends green CI with all six features reachable via keybindings but showing empty/placeholder UI. Phase-2 agents then fill in each widget.

**Architecture:** `app.py` is the Textual `Backend` — it projects `Model` onto reactives and runs `Cmd`s on worker lanes. No new lanes; new data fetches (`LoadOpLog`, `LoadBookmarks`, `LoadConflictData`) run on `group="diff", exclusive=True` (ephemeral, like diff fetches). New mutations run on `group="mutation"` via the existing `_worker_mutation` + `_OPS` dict. Modal lifecycle via a `modal` reactive projected from `Model.modal`.

**Tech Stack:** Python 3.11+, Textual, `jj` 0.42.0, `pytest`, `ruff`, `mypy --strict`.

**Depends on:** Phase 1a merged (all core `Msg`/`Cmd`/`update` branches + facade functions exist).

**Reference:** Spec at `docs/superpowers/specs/2026-06-22-daily-driver-essentials-design.md`.

**Scope:** `src/lajjzy/app.py`, six new stub widget files, `src/lajjzy/widgets/__init__.py`, architecture-test extensions. NOT in this plan: real widget rendering (phase 2).

---

## File structure

**Create:**
- `src/lajjzy/widgets/omnibar.py` — stub `Omnibar` widget.
- `src/lajjzy/widgets/bookmark_input.py` — stub `BookmarkInput` widget.
- `src/lajjzy/widgets/bookmark_picker.py` — stub `BookmarkPicker` widget.
- `src/lajjzy/widgets/op_log.py` — stub `OpLog` widget.
- `src/lajjzy/widgets/conflict_view.py` — stub `ConflictView` widget.
- `src/lajjzy/widgets/hunk_picker.py` — stub `HunkPicker` widget.
- `tests/widgets/__init__.py` — test package.
- `tests/widgets/test_stub_widgets.py` — smoke tests that each stub mounts.

**Modify:**
- `src/lajjzy/app.py` — new bindings, `run_cmd` branches, 3 new workers, `_OPS` entries, `modal` reactive, `present` extension, `compose` modal mounting, `action_*` methods, `LoadGraph` revset threading.
- `src/lajjzy/widgets/__init__.py` — re-export new widgets.
- `tests/test_architecture.py` — extend with widget-purity check.

---

## Task 1: Extend `present` + add `modal` reactive

**Files:**
- Modify: `src/lajjzy/app.py:98-101` (reactives), `src/lajjzy/app.py:133-138` (`present`)
- Test: `tests/test_app.py` (extend)

- [ ] **Step 1: Write the failing test**

Append to `tests/test_app.py`:

```python
@jj_required
async def test_present_projects_modal_and_new_model_fields(temp_repo: Path):
    from lajjzy.core import Model, OpenOmnibar
    from dataclasses import replace

    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test():
        await app.workers.wait_for_complete()
        app.runtime.dispatch(OpenOmnibar())
        await app.workers.wait_for_complete()
        assert app.modal == "omnibar"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/test_app.py::test_present_projects_modal_and_new_model_fields -v`
Expected: FAIL with `AttributeError: 'LajjzyApp' object has no attribute 'modal'`.

- [ ] **Step 3: Add the `modal` reactive and extend `present`**

In `src/lajjzy/app.py`, add to the reactives block (after `rebase_source` at line 101):

```python
    modal: reactive[str | None] = reactive(None)
    op_log_entries: reactive[list | None] = reactive(None)
    bookmarks: reactive[list | None] = reactive(None)
    revset: reactive[str | None] = reactive(None)
    conflict_data: reactive[object | None] = reactive(None)
    conflict_path: reactive[str | None] = reactive(None)
```

Extend `present` (line 133) — add after `self.pending_mutation = model.pending_mutation`:

```python
        self.modal = model.modal
        self.op_log_entries = model.op_log_entries
        self.bookmarks = model.bookmarks
        self.revset = model.revset
        self.conflict_data = model.conflict_data
        self.conflict_path = model.conflict_path
```

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/test_app.py::test_present_projects_modal_and_new_model_fields -v`
Expected: PASS.

- [ ] **Step 5: Lint + typecheck + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/app.py tests/test_app.py
git commit -m "feat(app): project modal + new Model fields onto reactives"
```

---

## Task 2: Thread `LoadGraph.revset` through `_worker_load`

**Files:**
- Modify: `src/lajjzy/app.py:142-150` (`run_cmd`), `src/lajjzy/app.py:152-165` (`_worker_load`)
- Test: `tests/test_app.py` (extend)

- [ ] **Step 1: Write the failing test**

Append to `tests/test_app.py`:

```python
@jj_required
async def test_omnibar_submit_filters_graph_by_revset(temp_repo: Path):
    import subprocess

    subprocess.run(["jj", "new", "-m", "second"], cwd=temp_repo, check=True, capture_output=True)
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test():
        await app.workers.wait_for_complete()
        from lajjzy.core import OpenOmnibar, OmnibarSubmit

        app.runtime.dispatch(OpenOmnibar())
        await app.workers.wait_for_complete()
        app.runtime.dispatch(OmnibarSubmit('description("second")'))
        await app.workers.wait_for_complete()
        # Filtered graph should contain only the change matching the revset.
        change_ids = [l.change_id for l in app.graph.lines if l.change_id]
        assert all("second" in (app.graph.details[c].description if c else "") for c in change_ids)
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/test_app.py::test_omnibar_submit_filters_graph_by_revset -v`
Expected: FAIL — `_worker_load` ignores `revset`, calls `jj.load_graph(repo)` unfiltered.

- [ ] **Step 3: Thread `revset` through `run_cmd` and `_worker_load`**

In `run_cmd` (line 142), change the `LoadGraph` branch:

```python
        if isinstance(cmd, LoadGraph):
            self._worker_load(cmd.epoch, cmd.revset)
```

In `_worker_load` (line 152), add the `revset` parameter and pass it to `jj.load_graph`:

```python
    @work(group="load", exclusive=True)
    async def _worker_load(self, epoch: int, revset: str | None = None) -> None:
        # group="load", exclusive: a new reload cancels any in-flight reload.
        try:
            graph = await jj.load_graph(self.repo_path, revset)
        except JjError as exc:
            self.runtime.dispatch(GraphLoadFailed(str(exc)))
        except InvariantError:
            raise
        except Exception as exc:
            self.runtime.dispatch(GraphLoadFailed(f"Unexpected error: {exc}"))
        else:
            self.runtime.dispatch(GraphLoaded(epoch, graph))
```

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/test_app.py::test_omnibar_submit_filters_graph_by_revset -v`
Expected: PASS.

- [ ] **Step 5: Lint + typecheck + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/app.py tests/test_app.py
git commit -m "feat(app): thread LoadGraph.revset through _worker_load"
```

---

## Task 3: Add `_OPS` entries for new mutation kinds

**Files:**
- Modify: `src/lajjzy/app.py:52-59` (`_OPS` dict)
- Test: `tests/test_app.py` (extend)

- [ ] **Step 1: Write the failing test**

Append to `tests/test_app.py`:

```python
@jj_required
async def test_undo_key_runs_jj_undo(temp_repo: Path):
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test() as pilot:
        await app.workers.wait_for_complete()
        await pilot.press("u")
        await app.workers.wait_for_complete()
        # Status bar or error should reflect the undo ran (no crash).
        # More precisely: the mutation worker completed and reloaded.
        assert app.graph is not None


@jj_required
async def test_U_key_runs_jj_redo(temp_repo: Path):
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test() as pilot:
        await app.workers.wait_for_complete()
        await pilot.press("u")  # undo first so redo has something to redo
        await app.workers.wait_for_complete()
        await pilot.press("U")
        await app.workers.wait_for_complete()
        assert app.graph is not None
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/test_app.py::test_undo_key_runs_jj_undo -v`
Expected: FAIL — no `u` binding, no `"undo"` in `_OPS`.

- [ ] **Step 3: Add `_OPS` entries and bindings**

Extend the `_OPS` dict (line 52) — add after the existing entries:

```python
_OPS: dict[str, Callable[[Path, tuple[Any, ...]], Awaitable[str]]] = {
    "new": lambda cwd, a: jj.new_change(cwd, *a),
    "abandon": lambda cwd, a: jj.abandon(cwd, *a),
    "edit": lambda cwd, a: jj.edit_change(cwd, *a),
    "squash": lambda cwd, a: jj.squash(cwd, *a),
    "describe": lambda cwd, a: jj.describe(cwd, *a),
    "rebase": lambda cwd, a: jj.rebase_single(cwd, *a),
    "rebase_descendants": lambda cwd, a: jj.rebase_with_descendants(cwd, *a),
    # --- phase 1b additions ---
    "undo": lambda cwd, a: jj.undo(cwd),
    "redo": lambda cwd, a: jj.redo(cwd),
    "bookmark_set": lambda cwd, a: jj.bookmark_set(cwd, *a),
    "bookmark_delete": lambda cwd, a: jj.bookmark_delete(cwd, *a),
    "bookmark_move": lambda cwd, a: jj.bookmark_move(cwd, *a),
    "op_restore": lambda cwd, a: jj.op_restore(cwd, *a),
    "resolve": lambda cwd, a: jj.resolve(cwd, *a),
    "split": lambda cwd, a: jj.split(cwd, *a),
    "squash_partial": lambda cwd, a: jj.squash_partial(cwd, *a),
}
```

Add the keybindings to `BINDINGS` (after line 93, before the closing `]`):

```python
        ("u", "undo", "Undo"),
        ("U", "redo", "Redo"),
        ("/", "open_omnibar", "Omnibar"),
        ("B", "open_bookmark_set", "Set bookmark"),
        ("b", "open_bookmark_picker", "Bookmarks"),
        ("o", "open_op_log", "Op log"),
        ("s", "split", "Split"),
        ("ctrl+s", "squash_partial", "Squash partial"),
```

Add the `action_*` methods after the existing ones (after `action_rebase_cancel`):

```python
    def action_undo(self) -> None:
        self.runtime.dispatch(Undo())

    def action_redo(self) -> None:
        self.runtime.dispatch(Redo())

    def action_open_omnibar(self) -> None:
        self.runtime.dispatch(OpenOmnibar())

    def action_open_bookmark_set(self) -> None:
        self.runtime.dispatch(OpenBookmarkSet())

    def action_open_bookmark_picker(self) -> None:
        self.runtime.dispatch(OpenBookmarkPicker())

    def action_open_op_log(self) -> None:
        self.runtime.dispatch(OpenOpLog())

    def action_split(self) -> None:
        self.runtime.dispatch(Split())

    def action_squash_partial(self) -> None:
        self.runtime.dispatch(SquashPartial())
```

Add the new `Msg` imports to the `from lajjzy.core import (...)` block: `Undo`, `Redo`, `OpenOmnibar`, `OpenBookmarkSet`, `OpenBookmarkPicker`, `OpenOpLog`, `Split`, `SquashPartial`, `LoadOpLog`, `LoadBookmarks`, `LoadConflictData`, `OpLogLoaded`, `OpLogLoadFailed`, `BookmarksLoaded`, `BookmarksLoadFailed`, `ConflictDataLoaded`, `ConflictDataLoadFailed`.

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/test_app.py::test_undo_key_runs_jj_undo tests/test_app.py::test_U_key_runs_jj_redo -v`
Expected: PASS.

- [ ] **Step 5: Lint + typecheck + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/app.py tests/test_app.py
git commit -m "feat(app): _OPS entries + bindings for undo/redo/omnibar/bookmarks/op-log/split"
```

---

## Task 4: Add `run_cmd` branches + workers for op-log, bookmarks, conflict-data

**Files:**
- Modify: `src/lajjzy/app.py:142-150` (`run_cmd`), add 3 new worker methods
- Test: `tests/test_app.py` (extend)

- [ ] **Step 1: Write the failing tests**

Append to `tests/test_app.py`:

```python
@jj_required
async def test_open_op_log_loads_entries(temp_repo: Path):
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test():
        await app.workers.wait_for_complete()
        from lajjzy.core import OpenOpLog

        app.runtime.dispatch(OpenOpLog())
        await app.workers.wait_for_complete()
        assert app.op_log_entries is not None
        assert len(app.op_log_entries) >= 1


@jj_required
async def test_open_bookmark_picker_loads_bookmarks(temp_repo: Path):
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test():
        await app.workers.wait_for_complete()
        from lajjzy.core import OpenBookmarkPicker

        app.runtime.dispatch(OpenBookmarkPicker())
        await app.workers.wait_for_complete()
        # Empty repo has no bookmarks, but the load should succeed (None → []).
        assert app.bookmarks is not None
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/test_app.py::test_open_op_log_loads_entries -v`
Expected: FAIL — `run_cmd` has no branch for `LoadOpLog`.

- [ ] **Step 3: Add `run_cmd` branches and 3 workers**

Extend `run_cmd` (line 142) — add branches after `EditMessage`:

```python
        elif isinstance(cmd, LoadOpLog):
            self._worker_op_log()
        elif isinstance(cmd, LoadBookmarks):
            self._worker_bookmarks()
        elif isinstance(cmd, LoadConflictData):
            self._worker_conflict_data(cmd.path)
```

Add the three worker methods after `_run_editor` (before `compose`):

```python
    @work(group="diff", exclusive=True)
    async def _worker_op_log(self) -> None:
        try:
            entries = await jj.op_log(self.repo_path)
        except JjError as exc:
            self.runtime.dispatch(OpLogLoadFailed(str(exc)))
        except InvariantError:
            raise
        except Exception as exc:
            self.runtime.dispatch(OpLogLoadFailed(f"Unexpected error: {exc}"))
        else:
            self.runtime.dispatch(OpLogLoaded(entries))

    @work(group="diff", exclusive=True)
    async def _worker_bookmarks(self) -> None:
        try:
            bms = await jj.load_bookmarks(self.repo_path)
        except JjError as exc:
            self.runtime.dispatch(BookmarksLoadFailed(str(exc)))
        except InvariantError:
            raise
        except Exception as exc:
            self.runtime.dispatch(BookmarksLoadFailed(f"Unexpected error: {exc}"))
        else:
            self.runtime.dispatch(BookmarksLoaded(bms))

    @work(group="diff", exclusive=True)
    async def _worker_conflict_data(self, path: str) -> None:
        try:
            data = await jj.conflict_data(self.repo_path, path)
        except JjError as exc:
            self.runtime.dispatch(ConflictDataLoadFailed(str(exc)))
        except InvariantError:
            raise
        except Exception as exc:
            self.runtime.dispatch(ConflictDataLoadFailed(f"Unexpected error: {exc}"))
        else:
            self.runtime.dispatch(ConflictDataLoaded(data))
```

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/test_app.py::test_open_op_log_loads_entries tests/test_app.py::test_open_bookmark_picker_loads_bookmarks -v`
Expected: PASS.

- [ ] **Step 5: Lint + typecheck + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/app.py tests/test_app.py
git commit -m "feat(app): workers for op-log, bookmarks, conflict-data on group=diff"
```

---

## Task 5: Bookmark-mutation reload also fetches bookmarks

**Files:**
- Modify: `src/lajjzy/app.py:167-195` (`_worker_mutation`)
- Test: `tests/test_app.py` (extend)

After a bookmark set/delete/move mutation completes, the bookmarks list should refresh so the picker reflects the change. The mutation worker already reloads the graph; we add a bookmarks reload for bookmark-kind mutations.

- [ ] **Step 1: Write the failing test**

Append to `tests/test_app.py`:

```python
@jj_required
async def test_bookmark_set_mutation_refreshes_bookmarks(temp_repo: Path):
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test():
        await app.workers.wait_for_complete()
        from lajjzy.core import BookmarkInputConfirm, OpenBookmarkSet

        app.runtime.dispatch(OpenBookmarkSet())
        await app.workers.wait_for_complete()
        graph = app.graph
        target = graph.lines[graph.working_copy_index or 0].change_id
        app.runtime.dispatch(BookmarkInputConfirm("testbm"))
        await app.workers.wait_for_complete()
        assert app.bookmarks is not None
        assert any(b.name == "testbm" for b in app.bookmarks)
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/test_app.py::test_bookmark_set_mutation_refreshes_bookmarks -v`
Expected: FAIL — `app.bookmarks` is still `None` after the mutation (worker only reloaded graph).

- [ ] **Step 3: Extend `_worker_mutation` to reload bookmarks for bookmark kinds**

In `_worker_mutation` (line 167), after the successful graph load and before dispatching `MutationCompleted`, add a bookmarks reload for bookmark-kind mutations:

```python
        # For bookmark mutations, also refresh the bookmarks list so the
        # picker reflects the change in the same step as the graph reload.
        if kind in ("bookmark_set", "bookmark_delete", "bookmark_move"):
            try:
                bms = await jj.load_bookmarks(self.repo_path)
            except (JjError, Exception):
                bms = None  # non-fatal: graph reload is the primary result
            self.runtime.dispatch(MutationCompleted(epoch, message, graph, None, bookmarks=bms))
            return
        self.runtime.dispatch(MutationCompleted(epoch, message, graph, None))
```

This requires extending `MutationCompleted` with an optional `bookmarks` field. In `src/lajjzy/core/messages.py`, update the `MutationCompleted` class:

```python
@dataclass(frozen=True)
class MutationCompleted:
    epoch: int
    message: str
    graph: GraphData | None
    load_error: str | None
    bookmarks: list[Bookmark] | None = None  # optional, for bookmark mutations
```

And in `src/lajjzy/core/update.py`, extend `_mutation_completed` to apply `bookmarks` if present:

```python
def _mutation_completed(model: Model, msg: MutationCompleted) -> Model:
    if msg.load_error is not None:
        return replace(model, error=msg.load_error, pending_mutation=False)
    reported = replace(model, error=msg.message, pending_mutation=False)
    if msg.graph is None or msg.epoch != model.graph_epoch:
        # Even if the graph is stale, apply bookmarks if we fetched them.
        if msg.bookmarks is not None:
            reported = replace(reported, bookmarks=msg.bookmarks)
        return reported
    reported = replace(reported, graph=msg.graph, cursor=cursor_after_reload(msg.graph))
    if msg.bookmarks is not None:
        reported = replace(reported, bookmarks=msg.bookmarks)
    return reported
```

- [ ] **Step 4: Run test to verify it passes + re-run core tests**

```bash
uv run pytest tests/test_app.py::test_bookmark_set_mutation_refreshes_bookmarks tests/core/test_update.py -v
```
Expected: PASS (the new test + all existing core tests, since the `bookmarks` field defaults to `None` and existing `MutationCompleted` constructions are unaffected).

- [ ] **Step 5: Lint + typecheck + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/app.py src/lajjzy/core/messages.py src/lajjzy/core/update.py tests/test_app.py
git commit -m "feat: bookmark mutations refresh bookmarks list in same step"
```

---

## Task 6: Six stub widget files + re-exports

**Files:**
- Create: `src/lajjzy/widgets/omnibar.py`, `bookmark_input.py`, `bookmark_picker.py`, `op_log.py`, `conflict_view.py`, `hunk_picker.py`
- Modify: `src/lajjzy/widgets/__init__.py`
- Test: `tests/widgets/__init__.py`, `tests/widgets/test_stub_widgets.py` (create)

- [ ] **Step 1: Write the failing smoke tests**

Create `tests/widgets/__init__.py` (empty). Create `tests/widgets/test_stub_widgets.py`:

```python
from __future__ import annotations

import pytest

from lajjzy.widgets import (
    BookmarkInput,
    BookmarkPicker,
    ConflictView,
    HunkPicker,
    Omnibar,
    OpLog,
)


@pytest.mark.parametrize(
    "widget_cls",
    [Omnibar, BookmarkInput, BookmarkPicker, OpLog, ConflictView, HunkPicker],
)
def test_stub_widget_importable(widget_cls):
    # Each stub must at least be importable and constructible.
    w = widget_cls()
    assert w is not None
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/widgets/test_stub_widgets.py -v`
Expected: FAIL with `ImportError: cannot import name 'Omnibar'` etc.

- [ ] **Step 3: Create the six stub widget files**

Each stub is a minimal Textual widget that renders an empty placeholder. Create `src/lajjzy/widgets/omnibar.py`:

```python
from __future__ import annotations

from textual.widgets import Static


class Omnibar(Static):
    """Omnibar overlay — revset search + completion. STUB: phase 1b mounts it;
    phase 2 (feature 2) fills in render + view-local state + key handling."""

    def render(self) -> str:
        return "(omnibar — phase 2)"
```

Create `src/lajjzy/widgets/bookmark_input.py`:

```python
from __future__ import annotations

from textual.widgets import Static


class BookmarkInput(Static):
    """Bookmark naming input modal. STUB: phase 2 (feature 3) fills in."""

    def render(self) -> str:
        return "(bookmark input — phase 2)"
```

Create `src/lajjzy/widgets/bookmark_picker.py`:

```python
from __future__ import annotations

from textual.widgets import Static


class BookmarkPicker(Static):
    """Bookmark list picker modal. STUB: phase 2 (feature 3) fills in."""

    def render(self) -> str:
        return "(bookmark picker — phase 2)"
```

Create `src/lajjzy/widgets/op_log.py`:

```python
from __future__ import annotations

from textual.widgets import Static


class OpLog(Static):
    """Operation log modal. STUB: phase 2 (feature 4) fills in."""

    def render(self) -> str:
        return "(op log — phase 2)"
```

Create `src/lajjzy/widgets/conflict_view.py`:

```python
from __future__ import annotations

from textual.widgets import Static


class ConflictView(Static):
    """Conflict resolution view. STUB: phase 2 (feature 5) fills in."""

    def render(self) -> str:
        return "(conflict view — phase 2)"
```

Create `src/lajjzy/widgets/hunk_picker.py`:

```python
from __future__ import annotations

from textual.widgets import Static


class HunkPicker(Static):
    """Hunk picker modal for split / partial squash. STUB: phase 2 (feature 6)."""

    def render(self) -> str:
        return "(hunk picker — phase 2)"
```

Update `src/lajjzy/widgets/__init__.py`:

```python
from lajjzy.widgets.bookmark_input import BookmarkInput
from lajjzy.widgets.bookmark_picker import BookmarkPicker
from lajjzy.widgets.conflict_view import ConflictView
from lajjzy.widgets.detail import DetailPanel
from lajjzy.widgets.graph import GraphView
from lajjzy.widgets.hunk_picker import HunkPicker
from lajjzy.widgets.omnibar import Omnibar
from lajjzy.widgets.op_log import OpLog
from lajjzy.widgets.status_bar import StatusBar

__all__ = [
    "BookmarkInput",
    "BookmarkPicker",
    "ConflictView",
    "DetailPanel",
    "GraphView",
    "HunkPicker",
    "Omnibar",
    "OpLog",
    "StatusBar",
]
```

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/widgets/test_stub_widgets.py -v`
Expected: PASS (6 parametrized cases).

- [ ] **Step 5: Lint + typecheck + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/widgets/ tests/widgets/
git commit -m "feat(widgets): six stub widget files for phase-2 fill-in"
```

---

## Task 7: Mount modals in `compose` based on `modal` reactive

**Files:**
- Modify: `src/lajjzy/app.py:206-212` (`compose`)
- Test: `tests/test_app.py` (extend)

- [ ] **Step 1: Write the failing test**

Append to `tests/test_app.py`:

```python
@jj_required
async def test_omnibar_mounts_when_modal_reactive_set(temp_repo: Path):
    from lajjzy.widgets import Omnibar

    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test() as pilot:
        await app.workers.wait_for_complete()
        await pilot.press("/")
        await app.workers.wait_for_complete()
        # The Omnibar widget should now be mounted.
        app.query_one(Omnibar)
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/test_app.py::test_omnibar_mounts_when_modal_reactive_set -v`
Expected: FAIL — `Omnibar` is not mounted (compose doesn't yield it).

- [ ] **Step 3: Mount modals conditionally in `compose`**

Replace `compose` (line 206):

```python
    def compose(self) -> ComposeResult:
        from lajjzy.widgets import (
            BookmarkInput,
            BookmarkPicker,
            ConflictView,
            DetailPanel,
            GraphView,
            HunkPicker,
            Omnibar,
            OpLog,
            StatusBar,
        )

        with Horizontal(id="panes"):
            yield GraphView()
            yield DetailPanel()
        yield StatusBar()
        # Modals are always mounted but hidden; visibility follows self.modal.
        # (Mounting once avoids mount/unmount churn on every modal open/close.)
        yield Omnibar(id="omnibar")
        yield BookmarkInput(id="bookmark_input")
        yield BookmarkPicker(id="bookmark_picker")
        yield OpLog(id="op_log")
        yield ConflictView(id="conflict_view")
        yield HunkPicker(id="hunk_picker")
```

Add a `watch_modal` method to show/hide based on the reactive (Textual calls `watch_<name>` when a reactive changes):

```python
    def watch_modal(self, modal: str | None) -> None:
        from lajjzy.widgets import (
            BookmarkInput,
            BookmarkPicker,
            ConflictView,
            HunkPicker,
            Omnibar,
            OpLog,
        )

        mapping = {
            "omnibar": Omnibar,
            "bookmark_input": BookmarkInput,
            "bookmark_picker": BookmarkPicker,
            "op_log": OpLog,
            "conflict_view": ConflictView,
            "hunk_picker": HunkPicker,
        }
        for name, cls in mapping.items():
            try:
                w = self.query_one(cls)
            except Exception:
                continue
            w.display = modal == name
```

And initialize all modals hidden in `on_mount` (add after `self.query_one(GraphView).focus()`):

```python
        # Modals start hidden; watch_modal shows the active one.
        from lajjzy.widgets import (
            BookmarkInput,
            BookmarkPicker,
            ConflictView,
            HunkPicker,
            Omnibar,
            OpLog,
        )

        for cls in (Omnibar, BookmarkInput, BookmarkPicker, OpLog, ConflictView, HunkPicker):
            self.query_one(cls).display = False
```

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/test_app.py::test_omnibar_mounts_when_modal_reactive_set -v`
Expected: PASS.

- [ ] **Step 5: Lint + typecheck + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/app.py tests/test_app.py
git commit -m "feat(app): mount + show/hide modals based on Model.modal"
```

---

## Task 8: DetailPanel routes conflicted-file Enter to `OpenConflictView`

**Files:**
- Modify: `src/lajjzy/widgets/detail.py` (the `Enter` handler)
- Test: `tests/test_app.py` (extend)

- [ ] **Step 1: Write the failing test**

Append to `tests/test_app.py`:

```python
@jj_required
async def test_enter_on_conflicted_file_opens_conflict_view(temp_repo: Path):
    # Create a conflict: edit same file in two changes, then merge.
    import subprocess

    subprocess.run(["jj", "new", "-m", "base"], cwd=temp_repo, check=True, capture_output=True)
    (temp_repo / "c.txt").write_text("BASE\n")
    subprocess.run(["jj", "new", "-m", "left"], cwd=temp_repo, check=True, capture_output=True)
    (temp_repo / "c.txt").write_text("LEFT\n")
    subprocess.run(["jj", "new", "-m", "right", "--after", "@-"], cwd=temp_repo, check=True, capture_output=True)
    (temp_repo / "c.txt").write_text("RIGHT\n")
    subprocess.run(["jj", "new", "-m", "merge", "--after", "@-", "--allow-empty"], cwd=temp_repo, check=True, capture_output=True)
    # @ is now the merge with a conflict on c.txt.
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test() as pilot:
        await app.workers.wait_for_complete()
        from lajjzy.widgets import ConflictView, DetailPanel

        app.query_one(DetailPanel).focus()
        await pilot.press("enter")
        await app.workers.wait_for_complete()
        # Conflict view should be mounted and visible.
        cv = app.query_one(ConflictView)
        assert cv.display
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/test_app.py::test_enter_on_conflicted_file_opens_conflict_view -v`
Expected: FAIL — `Enter` on a conflicted file opens the diff view, not the conflict view.

- [ ] **Step 3: Route conflicted-file Enter to `OpenConflictView`**

Open `src/lajjzy/widgets/detail.py` and find the `Enter` key handler (the method that opens the diff view). Add a check: if the selected file has `has_conflict=True` (read from the app's graph details for the selected change), dispatch `OpenConflictView(path)` instead of opening the diff. The exact method name depends on the current `detail.py` structure — read it first:

```bash
grep -n "def.*enter\|key_enter\|on_key\|action_" src/lajjzy/widgets/detail.py
```

In the handler, before the existing diff-open logic:

```python
        # If the selected file is conflicted, open the conflict view instead
        # of the diff view.
        app = self.app  # type: ignore[attr-defined]
        from lajjzy.app import LajjzyApp

        if isinstance(app, LajjzyApp):
            graph = app.graph
            change_id = app.selected_change_id()
            if graph is not None and change_id is not None:
                detail = graph.details.get(change_id)
                if detail is not None:
                    selected_file = detail.files[self.cursor]  # adjust index name
                    if selected_file.status == FileStatus.CONFLICTED:
                        from lajjzy.core import OpenConflictView

                        app.runtime.dispatch(OpenConflictView(selected_file.path))
                        return
```

(Adjust `self.cursor` to the actual file-list cursor attribute name in `DetailPanel`.)

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/test_app.py::test_enter_on_conflicted_file_opens_conflict_view -v`
Expected: PASS. If the conflict-creation setup doesn't produce a conflict in 0.42.0, adjust the setup (use `jj rebase` to force a three-way merge).

- [ ] **Step 5: Lint + typecheck + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/widgets/detail.py tests/test_app.py
git commit -m "feat(detail): Enter on conflicted file opens conflict view"
```

---

## Task 9: Architecture test — widgets don't import jj facade or subprocess

**Files:**
- Modify: `tests/test_architecture.py`
- Test: self

- [ ] **Step 1: Write the failing test**

Append to `tests/test_architecture.py`:

```python
def test_widgets_do_not_import_jj_facade_or_subprocess():
    # Widgets must dispatch Msgs; they never call the jj facade or spawn
    # subprocesses directly (the two-facade-boundary rule).
    widgets_dir = SRC / "widgets"
    offenders = []
    for path in widgets_dir.rglob("*.py"):
        text = path.read_text(encoding="utf-8")
        for banned in ("lajjzy.backend.jj", "subprocess", "asyncio.create_subprocess"):
            if banned in text:
                offenders.append(f"{path.relative_to(SRC)}: {banned}")
    assert not offenders, f"widget imports facade/subprocess: {offenders}"
```

- [ ] **Step 2: Run test to verify it fails (it should pass already, but verify)**

Run: `uv run pytest tests/test_architecture.py::test_widgets_do_not_import_jj_facade_or_subprocess -v`
Expected: PASS (the stub widgets don't import these). This test is a guard for phase 2.

- [ ] **Step 3: Lint + typecheck + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add tests/test_architecture.py
git commit -m "test(arch): widgets must not import jj facade or subprocess"
```

---

## Task 10: Phase 1b verification + PR

**Files:** none (verification only)

- [ ] **Step 1: Full local CI run**

```bash
uv run ruff check .
uv run ruff format --check .
uv run mypy src/lajjzy
uv run pytest -q
```
Expected: all four green.

- [ ] **Step 2: Smoke-test the bindings live**

```bash
uv run lajjzy
```
Manually press `u`, `U`, `/`, `B`, `b`, `o`, `s`, `Ctrl-S`. Each should show its stub modal or run silently (undo/redo). `Esc` should close modals. Verify no crashes.

- [ ] **Step 3: Push and open PR**

```bash
git push -u origin HEAD
gh pr create --title "Phase 1b: app.py bindings, workers, stub widgets for daily-driver essentials" --body "Wires the phase-1a core seams into app.py: keybindings, run_cmd branches, workers for op-log/bookmarks/conflict-data, _OPS entries, modal mounting, DetailPanel conflicted-file routing. Six stub widgets mount and show/hide. Phase-2 agents fill in each widget. See docs/superpowers/specs/2026-06-22-daily-driver-essentials-design.md."
```

- [ ] **Step 4: Confirm CI green**

```bash
gh pr checks <PR-NUMBER> --watch
```
Expected: `test` job PASS.

---

## Self-review notes

- **Spec coverage:** all six features have bindings (T3), modal mounting (T7), and a stub widget (T6). Conflict view has the DetailPanel routing (T8). Bookmark mutations refresh bookmarks (T5). Data fetch workers (T4). Revset threading (T2).
- **Keybinding reassignments:** `U` (redo), `Ctrl-S` (partial squash) per spec; both added in T3.
- **Worker lanes:** new data fetches on `group="diff", exclusive=True` (T4) — no new lanes, matches AGENTS.md. New mutations on existing `group="mutation"` via `_OPS` (T3).
- **Modal lifecycle:** one `modal` reactive (T1) + `watch_modal` show/hide (T7) + always-mounted widgets (avoids churn). Only one modal visible at a time.
- **Crash policy:** every new worker has `except InvariantError: raise` before `except Exception` (T4), matching the PR #30 fix.
- **Type consistency:** `MutationCompleted.bookmarks` is optional (`list[Bookmark] | None = None`) so existing constructions are unaffected (T5). `LoadGraph.revset` is optional (`str | None = None`) so existing `LoadGraph(epoch)` calls still work (T2).
- **Phase-2 readiness:** after this plan lands, phase-2 agents can read `app.py` to see which reactives exist (`modal`, `op_log_entries`, `bookmarks`, `revset`, `conflict_data`, `conflict_path`) and which Msgs their widget should dispatch. They touch only their widget file + test file.
