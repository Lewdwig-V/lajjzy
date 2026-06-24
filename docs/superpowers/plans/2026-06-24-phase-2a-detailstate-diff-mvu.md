# Phase 2a — DetailState + diff-through-MVU foundation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the detail pane's logic-bearing state (selected file, files/diff mode, fetched diff) out of the `DetailPanel` widget and into the pure `Model`, with diff data flowing through the MVU loop via a `LoadChangeDiff` Cmd, so `DetailPanel` becomes a pure projection and the view-local `open_diff` worker is removed.

**Architecture:** Pure MVU. New `DetailState` frozen dataclass on `Model`; cursor-nav resets it on selection change; `DetailFileUp/Down/Back/OpenFile` Msgs drive it; `DetailOpenFile` on a normal file emits `LoadChangeDiff(change_id)` whose worker runs `jj.change_diff` and dispatches `ChangeDiffLoaded(change_id, diff)` back into `update`. `DetailPanel` reads `app.detail` and dispatches Msgs; it holds no logic state.

**Tech Stack:** Python 3.11+, Textual, `jj` 0.42.0, `pytest`, `ruff`, `mypy --strict`.

**Spec:** `docs/superpowers/specs/2026-06-24-phase2-exemplary-mvu-design.md` (this is sub-phase 2a of three).

## Global Constraints

- `src/lajjzy/core/` stays pure: no `textual`, `asyncio`, `subprocess`, `os`, or `lajjzy.backend.jj` imports (enforced by `tests/test_architecture.py::test_core_modules_are_pure`). Importing `lajjzy.backend.types` is allowed.
- `src/lajjzy/backend/jj.py` is the ONLY module that runs jj subprocesses, via `run_jj`.
- Widgets never import the jj facade or `subprocess` (enforced by `test_widgets_do_not_import_jj_facade_or_subprocess`); they only render reactives and dispatch `Msg`s.
- Every mutation/effect worker re-raises `InvariantError` before any broad `except Exception` (crash policy).
- `Model` is frozen; all transitions go through `update` and use `dataclasses.replace`.
- mypy `--strict` clean, ruff clean, full suite green before each commit.
- The `run_cmd` `Cmd` dispatch must remain exhaustive (`else: assert_never(cmd)`).

---

## File structure

**Modify:**
- `src/lajjzy/core/model.py` — add `DetailState`; add `Model.detail`; add `select_change` helper; update docstring.
- `src/lajjzy/core/messages.py` — add `DetailFileUp`, `DetailFileDown`, `DetailOpenFile`, `DetailBack`, `ChangeDiffLoaded`, `ChangeDiffLoadFailed`; extend `Msg` union; import `FileDiff`.
- `src/lajjzy/core/commands.py` — add `LoadChangeDiff`; extend `Cmd` union.
- `src/lajjzy/core/update.py` — reset `detail` on cursor change; add detail-nav + diff branches.
- `src/lajjzy/core/__init__.py` — re-export new symbols.
- `src/lajjzy/app.py` — `detail` reactive + `present` projection; `LoadChangeDiff` worker + `run_cmd` branch; remove `open_diff` worker.
- `src/lajjzy/widgets/detail.py` — refactor to a pure projection.
- `tests/core/test_update.py`, `tests/test_app.py` — tests.

---

## Task 1: `DetailState` + `Model.detail` + reset-on-select

**Files:**
- Modify: `src/lajjzy/core/model.py`
- Modify: `src/lajjzy/core/__init__.py`
- Test: `tests/core/test_update.py`

**Interfaces:**
- Produces: `DetailState(file_cursor: int = 0, mode: Literal["files","diff"] = "files", diff: list[FileDiff] | None = None)` (frozen, slots); `Model.detail: DetailState`; `select_change(model: Model, cursor: int) -> Model` (sets cursor, resets `detail` to a fresh `DetailState` only when the cursor actually changes).

- [ ] **Step 1: Write the failing test**

Append to `tests/core/test_update.py` (the file already imports `Model`, `replace`, `update`, and the cursor Msgs; add `DetailState` and `select_change` to the `from lajjzy.core.model import ...` line, and `CursorDown`/`CursorUp` are already imported):

```python
def test_model_has_detail_defaulting_to_fresh_detailstate():
    m = Model()
    assert m.detail == DetailState()
    assert m.detail.file_cursor == 0
    assert m.detail.mode == "files"
    assert m.detail.diff is None


def test_select_change_resets_detail_only_on_actual_change():
    g = _loaded("aaa", "bbb", working=0).graph  # two nodes
    m = replace(Model(), graph=g, cursor=g.node_indices[0],
                detail=DetailState(file_cursor=3, mode="diff", diff=[]))
    # moving to a different node resets detail
    moved = select_change(m, g.node_indices[1])
    assert moved.cursor == g.node_indices[1]
    assert moved.detail == DetailState()
    # selecting the same cursor leaves detail untouched
    same = select_change(m, g.node_indices[0])
    assert same.detail == m.detail
```

(`_loaded(*change_ids, working=...)` is the existing helper at the top of `test_update.py` that builds a `Model` with a graph from change ids.)

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/core/test_update.py -k "detail_defaulting or select_change_resets" -v`
Expected: FAIL — `cannot import name 'DetailState'` / `'select_change'`.

- [ ] **Step 3: Implement in `src/lajjzy/core/model.py`**

Add `FileDiff` to the types import and `field` to the dataclasses import:

```python
from dataclasses import dataclass, field
from lajjzy.backend.types import Bookmark, ConflictData, FileDiff, GraphData, OpLogEntry
```

Add the `DetailState` dataclass (above `Model`):

```python
@dataclass(frozen=True, slots=True)
class DetailState:
    """The detail pane's logic-bearing state, owned by the Model (not the
    widget). ``diff`` is loaded through the MVU loop (LoadChangeDiff →
    ChangeDiffLoaded); it is None until the load lands and while in files mode."""

    file_cursor: int = 0
    mode: Literal["files", "diff"] = "files"
    diff: list[FileDiff] | None = None
```

Add the field to `Model` (after `modal: Modal | None = None`), and DELETE the
docstring paragraph that begins "Detail-pane browsing state ... is deliberately
*not* here" (it is now false):

```python
    detail: DetailState = field(default_factory=DetailState)
```

Add the helper (below `selected_change_id`):

```python
def select_change(model: Model, cursor: int) -> Model:
    """Move the change-graph cursor. Resets the detail pane to a fresh
    DetailState whenever the selected line actually changes, so a stale diff or
    file cursor never carries across to a different change."""
    if cursor == model.cursor:
        return replace(model, cursor=cursor)
    return replace(model, cursor=cursor, detail=DetailState())
```

Add `from dataclasses import replace` at the top of `model.py` if not present
(it currently is not — add it next to the other `dataclasses` import:
`from dataclasses import dataclass, field, replace`).

- [ ] **Step 4: Re-export from `src/lajjzy/core/__init__.py`**

Add `DetailState` and `select_change` to the `from lajjzy.core.model import (...)` block and to `__all__`.

- [ ] **Step 5: Run test to verify it passes**

Run: `uv run pytest tests/core/test_update.py -k "detail_defaulting or select_change_resets" -v`
Expected: PASS (2 tests).

- [ ] **Step 6: Wire cursor-nav branches to reset detail, run full core suite**

In `src/lajjzy/core/update.py`, import `select_change` (add to the
`from lajjzy.core.model import ...` line) and replace the four navigation
branches so they go through it:

```python
    if isinstance(msg, CursorDown):
        return select_change(model, step_cursor(model, 1)), []
    if isinstance(msg, CursorUp):
        return select_change(model, step_cursor(model, -1)), []
    if isinstance(msg, CursorTop):
        if model.graph and model.graph.node_indices:
            return select_change(model, model.graph.node_indices[0]), []
        return model, []
    if isinstance(msg, CursorBottom):
        if model.graph and model.graph.node_indices:
            return select_change(model, model.graph.node_indices[-1]), []
        return model, []
```

Also reset detail on a fresh graph load — in the `GraphLoaded` branch, add
`detail=DetailState()` to the `replace(...)`:

```python
        return replace(
            model, error=None, graph=msg.graph,
            cursor=cursor_after_reload(msg.graph), detail=DetailState(),
        ), []
```

(Add `DetailState` to update.py's `from lajjzy.core.model import ...` import.)

Run: `uv run pytest tests/core/ -q`
Expected: all PASS (existing nav tests still green — they assert `cursor`, which is unchanged; `detail` defaults equal).

- [ ] **Step 7: Gate + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/core/model.py src/lajjzy/core/update.py src/lajjzy/core/__init__.py tests/core/test_update.py
git commit -m "feat(core): DetailState on Model + select_change resets detail on selection change"
```

---

## Task 2: detail-pane navigation Msgs (`DetailFileUp/Down/Back`)

**Files:**
- Modify: `src/lajjzy/core/messages.py`, `src/lajjzy/core/update.py`, `src/lajjzy/core/__init__.py`
- Test: `tests/core/test_update.py`

**Interfaces:**
- Produces: `DetailFileDown`, `DetailFileUp`, `DetailBack` (empty frozen dataclasses); `update` branches that clamp `detail.file_cursor` to the selected change's file count (only in `mode == "files"`) and that exit diff mode.
- Consumes: `Model.detail`, `selected_change_id`, `GraphData.details`.

- [ ] **Step 1: Write the failing test**

Append to `tests/core/test_update.py` (add the three Msg names to the
`from lajjzy.core.messages import (...)` block):

```python
def _two_file_change():
    # A model whose selected change "aaa" has two files.
    from lajjzy.backend.types import ChangeDetail, FileChange, FileStatus, GraphData, GraphLine

    line = GraphLine(change_id="aaa", node=True, text="@  aaa")
    detail = ChangeDetail(
        change_id="aaa", description="d", author="a", timestamp="now",
        bookmarks=[], parents=[], is_empty=False, has_conflict=False,
        files=[FileChange(path="a.txt", status=FileStatus.MODIFIED),
               FileChange(path="b.txt", status=FileStatus.MODIFIED)],
    )
    g = GraphData(lines=[line], details={"aaa": detail}, working_copy_index=0)
    return replace(Model(), graph=g, cursor=0)


def test_detail_file_down_clamps_to_file_count():
    m = _two_file_change()
    m1, cmds = update(m, DetailFileDown())
    assert m1.detail.file_cursor == 1
    assert cmds == []
    m2, _ = update(m1, DetailFileDown())  # already at last file
    assert m2.detail.file_cursor == 1


def test_detail_file_up_clamps_at_zero():
    m = replace(_two_file_change(), detail=DetailState(file_cursor=1))
    m1, _ = update(m, DetailFileUp())
    assert m1.detail.file_cursor == 0
    m2, _ = update(m1, DetailFileUp())
    assert m2.detail.file_cursor == 0


def test_detail_file_nav_ignored_in_diff_mode():
    m = replace(_two_file_change(), detail=DetailState(file_cursor=0, mode="diff"))
    m1, _ = update(m, DetailFileDown())
    assert m1.detail.file_cursor == 0  # unchanged in diff mode


def test_detail_back_returns_to_files_and_clears_diff():
    m = replace(_two_file_change(), detail=DetailState(mode="diff", diff=[]))
    m1, _ = update(m, DetailBack())
    assert m1.detail.mode == "files"
    assert m1.detail.diff is None
```

(Adjust the `ChangeDetail`/`GraphLine`/`GraphData` constructor kwargs to the
actual field names in `src/lajjzy/backend/types.py` — read them first; the names
above are the expected ones but verify and fix any mismatch.)

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/core/test_update.py -k "detail_file or detail_back" -v`
Expected: FAIL — import error / no branches.

- [ ] **Step 3: Add the Msgs**

In `src/lajjzy/core/messages.py`, add (before the `Msg = ...` union) and then
add each new name to the union:

```python
@dataclass(frozen=True)
class DetailFileDown:
    pass


@dataclass(frozen=True)
class DetailFileUp:
    pass


@dataclass(frozen=True)
class DetailBack:
    pass
```

- [ ] **Step 4: Add the `update` branches**

In `src/lajjzy/core/update.py`, add a small helper near the bottom (next to
`_start_mutation`) and the branches (place them after the navigation block):

```python
def _current_files(model: Model) -> list[FileChange]:
    cid = selected_change_id(model)
    if cid is None or model.graph is None:
        return []
    detail = model.graph.details.get(cid)
    return detail.files if detail else []
```

Branches (after `CursorBottom`):

```python
    if isinstance(msg, DetailFileDown):
        if model.detail.mode != "files":
            return model, []
        n = len(_current_files(model))
        if n == 0:
            return model, []
        new = min(n - 1, model.detail.file_cursor + 1)
        return replace(model, detail=replace(model.detail, file_cursor=new)), []
    if isinstance(msg, DetailFileUp):
        if model.detail.mode != "files":
            return model, []
        new = max(0, model.detail.file_cursor - 1)
        return replace(model, detail=replace(model.detail, file_cursor=new)), []
    if isinstance(msg, DetailBack):
        if model.detail.mode == "diff":
            return replace(model, detail=replace(model.detail, mode="files", diff=None)), []
        return model, []
```

Add `FileChange` to update.py's `from lajjzy.backend.types import ...` (add the
import line if none exists) and `DetailFileDown`, `DetailFileUp`, `DetailBack`
to the messages import.

- [ ] **Step 5: Re-export + run tests**

Add the three Msgs to `core/__init__.py` (imports + `__all__`).
Run: `uv run pytest tests/core/test_update.py -k "detail_file or detail_back" -v`
Expected: PASS (4 tests).

- [ ] **Step 6: Gate + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/core/messages.py src/lajjzy/core/update.py src/lajjzy/core/__init__.py tests/core/test_update.py
git commit -m "feat(core): DetailFileUp/Down/Back update branches"
```

---

## Task 3: `LoadChangeDiff` Cmd + `ChangeDiffLoaded`/`ChangeDiffLoadFailed` + `DetailOpenFile`

**Files:**
- Modify: `src/lajjzy/core/commands.py`, `src/lajjzy/core/messages.py`, `src/lajjzy/core/update.py`, `src/lajjzy/core/__init__.py`
- Test: `tests/core/test_update.py`

**Interfaces:**
- Produces: `LoadChangeDiff(change_id: str)` (Cmd); `ChangeDiffLoaded(change_id: str, diff: list[FileDiff])`, `ChangeDiffLoadFailed(error: str)` (Msgs); `DetailOpenFile` (Msg). `DetailOpenFile` on a non-conflicted selected file → `detail.mode="diff"` + `[LoadChangeDiff(change_id)]`; on a `CONFLICTED` file → delegates to the existing `OpenConflictView` behavior. `ChangeDiffLoaded` stores the diff in `detail.diff` only if `detail.mode == "diff"` and `change_id` still matches the selection (else dropped). `ChangeDiffLoadFailed` → `Model.error`.
- Consumes: `OpenConflictView` handling (existing, 1a), `selected_change_id`, `_current_files`, `FileStatus`.

- [ ] **Step 1: Write the failing test**

Append to `tests/core/test_update.py` (import the new names; `OpenConflictView`,
`FileStatus` may need importing):

```python
def test_detail_open_file_normal_enters_diff_mode_and_loads():
    m = _two_file_change()  # selected change "aaa", file_cursor 0 = a.txt (MODIFIED)
    m1, cmds = update(m, DetailOpenFile())
    assert m1.detail.mode == "diff"
    assert cmds == [LoadChangeDiff("aaa")]


def test_detail_open_file_conflicted_opens_conflict_view():
    from lajjzy.backend.types import ChangeDetail, FileChange, FileStatus, GraphData, GraphLine

    line = GraphLine(change_id="aaa", node=True, text="@  aaa")
    detail = ChangeDetail(
        change_id="aaa", description="d", author="a", timestamp="now",
        bookmarks=[], parents=[], is_empty=False, has_conflict=True,
        files=[FileChange(path="c.txt", status=FileStatus.CONFLICTED)],
    )
    g = GraphData(lines=[line], details={"aaa": detail}, working_copy_index=0)
    m = replace(Model(), graph=g, cursor=0)
    m1, cmds = update(m, DetailOpenFile())
    assert m1.modal == "conflict_view"
    assert m1.conflict_path == "c.txt"
    assert cmds == [LoadConflictData("c.txt")]


def test_change_diff_loaded_stores_when_relevant():
    m = replace(_two_file_change(), detail=DetailState(mode="diff"))
    m1, _ = update(m, ChangeDiffLoaded("aaa", []))
    assert m1.detail.diff == []


def test_change_diff_loaded_dropped_when_not_in_diff_mode():
    m = _two_file_change()  # mode == "files"
    m1, _ = update(m, ChangeDiffLoaded("aaa", [SOME_DIFF := []]))
    assert m1.detail.diff is None  # dropped


def test_change_diff_loaded_dropped_when_change_id_stale():
    m = replace(_two_file_change(), detail=DetailState(mode="diff"))
    m1, _ = update(m, ChangeDiffLoaded("different", []))
    assert m1.detail.diff is None


def test_change_diff_load_failed_sets_error():
    m = _two_file_change()
    m1, _ = update(m, ChangeDiffLoadFailed("boom"))
    assert m1.error == "boom"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/core/test_update.py -k "detail_open_file or change_diff" -v`
Expected: FAIL.

- [ ] **Step 3: Add the Cmd and Msgs**

`src/lajjzy/core/commands.py` — add (before the `Cmd = ...` union) and extend the union:

```python
@dataclass(frozen=True)
class LoadChangeDiff:
    """Fetch the diff for one change. On completion dispatch
    ChangeDiffLoaded(change_id, diff) or ChangeDiffLoadFailed(error)."""

    change_id: str
```

`src/lajjzy/core/messages.py` — add `FileDiff` to the types import, add the Msgs
(before the union) and extend the union:

```python
@dataclass(frozen=True)
class DetailOpenFile:
    pass


@dataclass(frozen=True)
class ChangeDiffLoaded:
    change_id: str
    diff: list[FileDiff]


@dataclass(frozen=True)
class ChangeDiffLoadFailed:
    error: str
```

- [ ] **Step 4: Add the `update` branches**

In `src/lajjzy/core/update.py` (after the `DetailBack` branch). `DetailOpenFile`
delegates the conflicted case to `OpenConflictView` by recursively calling
`update` so the conflict-open logic stays defined in exactly one place:

```python
    if isinstance(msg, DetailOpenFile):
        if model.detail.mode != "files":
            return model, []
        files = _current_files(model)
        fc = model.detail.file_cursor
        if not (0 <= fc < len(files)):
            return model, []
        selected = files[fc]
        if selected.status == FileStatus.CONFLICTED:
            return update(model, OpenConflictView(selected.path))
        cid = selected_change_id(model)
        if cid is None:
            return model, []
        return replace(model, detail=replace(model.detail, mode="diff")), [LoadChangeDiff(cid)]
    if isinstance(msg, ChangeDiffLoaded):
        if model.detail.mode != "diff" or selected_change_id(model) != msg.change_id:
            return model, []  # superseded: user navigated away or left diff mode
        return replace(model, detail=replace(model.detail, diff=msg.diff)), []
    if isinstance(msg, ChangeDiffLoadFailed):
        return replace(model, error=msg.error), []
```

Add `FileStatus` to update.py's backend-types import, `LoadChangeDiff` to the
commands import, and `DetailOpenFile`/`ChangeDiffLoaded`/`ChangeDiffLoadFailed`
(and confirm `OpenConflictView`, `LoadConflictData` are imported — they are, from 1a).

- [ ] **Step 5: Re-export + run tests**

Add `LoadChangeDiff`, `DetailOpenFile`, `ChangeDiffLoaded`, `ChangeDiffLoadFailed`
to `core/__init__.py` (imports + `__all__`).
Run: `uv run pytest tests/core/test_update.py -k "detail_open_file or change_diff" -v`
Expected: PASS (6 tests).

- [ ] **Step 6: Gate + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/core/ tests/core/test_update.py
git commit -m "feat(core): LoadChangeDiff + DetailOpenFile (diff/conflict) + ChangeDiffLoaded guard"
```

---

## Task 4: `app.py` — `detail` reactive, `LoadChangeDiff` worker, remove `open_diff`

**Files:**
- Modify: `src/lajjzy/app.py`
- Test: `tests/test_app.py`

**Interfaces:**
- Consumes: `LoadChangeDiff`, `ChangeDiffLoaded`, `ChangeDiffLoadFailed`, `DetailState`, `jj.change_diff(cwd, change_id) -> list[FileDiff]`.
- Produces: `app.detail: reactive[DetailState]` projected by `present`; a `_worker_change_diff` on `group="diff", exclusive=True`; `run_cmd` handles `LoadChangeDiff`; the old `open_diff` worker is gone.

- [ ] **Step 1: Write the failing test**

Append to `tests/test_app.py` (match the existing `@jj_required` pilot pattern):

```python
@jj_required
async def test_detail_open_file_loads_diff_through_mvu(temp_repo: Path):
    import subprocess

    (temp_repo / "x.txt").write_text("one\n")
    subprocess.run(["jj", "describe", "-m", "add x"], cwd=temp_repo, check=True, capture_output=True)
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test() as pilot:
        await app.workers.wait_for_complete()
        from lajjzy.core import DetailOpenFile

        app.runtime.dispatch(DetailOpenFile())
        await app.workers.wait_for_complete()
        assert app.detail.mode == "diff"
        assert app.detail.diff is not None
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/test_app.py::test_detail_open_file_loads_diff_through_mvu -v`
Expected: FAIL — `app` has no `detail` reactive / `LoadChangeDiff` unhandled.

- [ ] **Step 3: Add the `detail` reactive + projection**

In `src/lajjzy/app.py`: add `DetailState` to the `from lajjzy.core import (...)`
block; add `LoadChangeDiff` to the commands part of that import and
`ChangeDiffLoaded`, `ChangeDiffLoadFailed` to the messages import block.

Add the reactive (after the other reactives, e.g. after `conflict_path`):

```python
    detail: reactive[DetailState] = reactive(DetailState())
```

Extend `present` (after `self.conflict_path = model.conflict_path`):

```python
        self.detail = model.detail
```

- [ ] **Step 4: Handle `LoadChangeDiff` in `run_cmd` + add the worker, remove `open_diff`**

In `run_cmd`, add a branch before the final `assert_never`:

```python
        elif isinstance(cmd, LoadChangeDiff):
            self._worker_change_diff(cmd.change_id)
```

Add the worker (place it where `open_diff` was):

```python
    @work(group="diff", exclusive=True)
    async def _worker_change_diff(self, change_id: str) -> None:
        try:
            diff = await jj.change_diff(self.repo_path, change_id)
        except JjError as exc:
            self.runtime.dispatch(ChangeDiffLoadFailed(str(exc)))
        except InvariantError:
            raise
        except Exception as exc:
            self.runtime.dispatch(ChangeDiffLoadFailed(f"Unexpected error: {exc}"))
        else:
            self.runtime.dispatch(ChangeDiffLoaded(change_id, diff))
```

DELETE the entire `open_diff` method (the `@work(group="diff", exclusive=True)`
coroutine that filtered `all_files` and set `panel.diff`/`panel.mode`).

- [ ] **Step 5: Run test + check for stale references**

Run: `uv run pytest tests/test_app.py::test_detail_open_file_loads_diff_through_mvu -v`
Expected: PASS.
Then: `grep -rn "open_diff" src tests` — if any test or code still calls
`app.open_diff(...)`, update it to dispatch `DetailOpenFile()` (the diff now
flows through MVU). Re-run the full app suite: `uv run pytest tests/test_app.py -q`.

- [ ] **Step 6: Gate + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/app.py tests/test_app.py
git commit -m "feat(app): detail reactive + LoadChangeDiff worker; remove view-local open_diff"
```

---

## Task 5: `DetailPanel` → pure projection

**Files:**
- Modify: `src/lajjzy/widgets/detail.py`
- Test: `tests/test_app.py`

**Interfaces:**
- Consumes: `app.detail` (`DetailState`), `app.graph`, `app.cursor`, `app.selected_change_id()`; dispatches `DetailFileDown/Up/OpenFile/Back`.
- Produces: a `DetailPanel` that holds NO `file_cursor`/`mode`/`diff` state — it reads them from `app.detail`.

- [ ] **Step 1: Write the failing test**

Append to `tests/test_app.py`:

```python
@jj_required
async def test_detail_panel_holds_no_logic_state(temp_repo: Path):
    from lajjzy.widgets import DetailPanel

    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test():
        await app.workers.wait_for_complete()
        panel = app.query_one(DetailPanel)
        # The widget must not own these any more; they live on Model.detail.
        assert not hasattr(panel, "file_cursor") or "file_cursor" not in type(panel).__dict__
        assert "mode" not in type(panel).__dict__
```

(If asserting reactive-absence is awkward, instead assert behavior: after
`DetailFileDown` the rendered selection moves — but the no-state assertion is the
contract this task enforces. Keep whichever cleanly proves the widget is stateless.)

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/test_app.py::test_detail_panel_holds_no_logic_state -v`
Expected: FAIL — `DetailPanel` still declares `file_cursor`/`mode` reactives.

- [ ] **Step 3: Rewrite `src/lajjzy/widgets/detail.py`**

Replace the file with a pure projection:

```python
from __future__ import annotations

from typing import TYPE_CHECKING, cast

from rich.text import Text
from textual.widget import Widget

from lajjzy.backend.types import FileChange

if TYPE_CHECKING:
    from lajjzy.app import LajjzyApp


class DetailPanel(Widget):
    can_focus = True

    # Focus-scoped: these fire ONLY when the DetailPanel has focus. Each
    # dispatches a Msg; all state lives on Model.detail.
    BINDINGS = [
        ("j", "file_down", "Next file"),
        ("down", "file_down", "Next file"),
        ("k", "file_up", "Prev file"),
        ("up", "file_up", "Prev file"),
        ("enter", "open_file", "Open diff"),
        ("escape", "back", "Back"),
    ]

    def on_mount(self) -> None:
        # Re-render whenever the projected detail state or selection changes.
        self.watch(self.app, "detail", lambda _: self.refresh())
        self.watch(self.app, "graph", lambda _: self.refresh())
        self.watch(self.app, "cursor", lambda _: self.refresh())

    def _app(self) -> LajjzyApp:
        return cast("LajjzyApp", self.app)

    def current_files(self) -> list[FileChange]:
        app = self._app()
        change_id = app.selected_change_id()
        graph = app.graph
        if change_id is None or graph is None:
            return []
        detail = graph.details.get(change_id)
        return detail.files if detail else []

    def action_file_down(self) -> None:
        from lajjzy.core import DetailFileDown

        self._app().runtime.dispatch(DetailFileDown())

    def action_file_up(self) -> None:
        from lajjzy.core import DetailFileUp

        self._app().runtime.dispatch(DetailFileUp())

    def action_open_file(self) -> None:
        from lajjzy.core import DetailOpenFile

        self._app().runtime.dispatch(DetailOpenFile())

    def action_back(self) -> None:
        from lajjzy.core import DetailBack

        self._app().runtime.dispatch(DetailBack())

    def render(self) -> Text:
        detail = self._app().detail
        if detail.mode == "diff":
            return self._render_diff()
        return self._render_files()

    def _render_files(self) -> Text:
        files = self.current_files()
        if not files:
            return Text("(no file changes)", style="dim")
        cursor = self._app().detail.file_cursor
        text = Text()
        for i, fc in enumerate(files):
            style = "reverse" if i == cursor else ""
            text.append(f"{fc.status.value} {fc.path}\n", style=style)
        return text

    def _render_diff(self) -> Text:
        diff = self._app().detail.diff
        if not diff:
            return Text("(no diff)", style="dim")
        files = self.current_files()
        cursor = self._app().detail.file_cursor
        # Show the opened file's diff (preserves the prior single-file view).
        path = files[cursor].path if 0 <= cursor < len(files) else None
        shown = [fd for fd in diff if fd.path == path] or diff
        text = Text()
        for fd in shown:
            text.append(f"{fd.path}\n", style="bold")
            for hunk in fd.hunks:
                text.append(hunk.header + "\n", style="cyan")
                for ln in hunk.lines:
                    style = {"add": "green", "remove": "red"}.get(ln.kind, "")
                    sign = {"add": "+", "remove": "-"}.get(ln.kind, " ")
                    text.append(f"{sign}{ln.text}\n", style=style)
        return text
```

- [ ] **Step 4: Run test + the existing detail behavior tests**

Run: `uv run pytest tests/test_app.py -q`
Expected: PASS. If any existing test referenced `panel.file_cursor`/`panel.mode`/
`panel.diff` directly, update it to drive via `DetailFileDown`/`DetailOpenFile`
Msgs and assert on `app.detail`.

- [ ] **Step 5: Gate + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/widgets/detail.py tests/test_app.py
git commit -m "refactor(detail): DetailPanel is a pure projection of Model.detail"
```

---

## Task 6: Phase 2a verification + PR

**Files:** none (verification only)

- [ ] **Step 1: Full local CI run**

```bash
uv run ruff check .
uv run ruff format --check .
uv run mypy src/lajjzy
uv run pytest -q
```
Expected: all four green.

- [ ] **Step 2: Confirm the split-brain seed is gone**

```bash
grep -rn "open_diff" src tests        # expect: no matches
grep -n "file_cursor\|self.diff\|mode: reactive" src/lajjzy/widgets/detail.py  # expect: none
```
The detail pane's state must live only on `Model.detail`.

- [ ] **Step 3: Push and open PR**

```bash
git push -u origin HEAD
gh pr create --title "Phase 2a: DetailState + diff-through-MVU foundation" --body "Moves the detail pane's logic state (file cursor, files/diff mode, fetched diff) out of the DetailPanel widget into Model.detail, with diff data flowing through the MVU loop (LoadChangeDiff Cmd -> ChangeDiffLoaded Msg). Removes the view-local open_diff worker — the first split-brain seed. DetailPanel is now a pure projection. Foundation for phase 2b (modal tagged union) and 2c (widgets). See docs/superpowers/specs/2026-06-24-phase2-exemplary-mvu-design.md."
```

- [ ] **Step 4: Confirm CI green** — `gh pr checks <PR> --watch`.

---

## Self-review notes

- **Spec coverage (2a slice):** `DetailState` in Model (T1), diff-through-MVU `LoadChangeDiff`/`ChangeDiffLoaded` (T3/T4), DetailPanel pure projection + `open_diff` removed (T4/T5), reset-on-selection (T1), conflicted-file routing preserved via `DetailOpenFile`→`OpenConflictView` (T3). Modal union + widgets are 2b/2c, out of scope here.
- **Stale-load guard:** `ChangeDiffLoaded` is dropped unless `detail.mode=="diff"` AND `change_id` matches the current selection (T3) — mirrors the graph-load epoch guard.
- **Constructor-kwargs caveat:** Tasks 2–3 build `GraphData`/`ChangeDetail`/`GraphLine`/`FileChange` directly in tests; the exact field names must be read from `src/lajjzy/backend/types.py` and matched (the helper `_loaded` in `test_update.py` shows the real construction and can be reused instead of hand-building).
- **mypy union exhaustiveness:** new Msgs are added to the `Msg` union and new Cmd to the `Cmd` union, so `update` and `run_cmd` (with `assert_never`) typecheck.
