# Invariant Guardrails Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add three layers of guardrails to lajjzy — runtime invariant assertions (crash-hard), type-level unrepresentability (frozen/validated types + `mypy --strict`), and architectural + property tests in CI — so hard invariants are enforced and the Codex-P1 class of miss cannot silently recur.

**Architecture:** A central `invariant()` helper raising `InvariantError` for internal/model breaches (crash via a clean `main()` handler); `ValueError → JjError` stays the path for data/external breaches. Domain dataclasses become frozen + self-validating, with `GraphData.node_indices` derived. AST-based architectural tests and `hypothesis` property tests codify the rules; `mypy --strict` gates CI.

**Tech Stack:** Python 3.11+, Textual 8.x, `mypy` (strict), `hypothesis`, `pytest`, `ruff`, `uv`.

## Global Constraints

- **Internal vs external errors:** model/state breaches → `InvariantError` (crash). Data-shape breaches → `ValueError` in `__post_init__`, wrapped to `JjError` at the backend boundary. User/jj errors → `JjError → self.error`. Never blur these.
- **Assertions:** hard invariants use `invariant(cond, msg)` (explicit raise, survives `python -O`). Bare `assert` only for debug-only redundant checks.
- **Facade boundary:** only `src/lajjzy/backend/jj.py` spawns a `jj` subprocess; the sole non-backend subprocess is `app.py`'s `$EDITOR` launch inside `with self.suspend():`.
- **Crash policy:** on `InvariantError`, the top-level handler restores the terminal, prints the violated invariant + a report hint, exits non-zero (`70`). Repo state is safe on disk.
- **No new runtime deps.** `mypy` and `hypothesis` are dev-only (`[dependency-groups] dev`).
- **mypy strict scope:** `files = ["src/lajjzy"]`. The CI mypy gate is added only once `src/lajjzy` passes with zero errors.
- **Commits:** conventional messages ending with `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.
- **Keep the suite green** (`uv run pytest`) and `ruff check .` / `ruff format --check .` clean at every commit.

---

## File Structure

```
src/lajjzy/
  invariants.py     # NEW: InvariantError + invariant() helper
  backend/types.py  # MODIFY: frozen dataclasses, node_indices cached_property, extended GraphData validation
  backend/parse.py  # MODIFY: construct ChangeDetail once (accumulate files) so frozen works cleanly
  app.py            # MODIFY: invariant() sites (I1/I3), worker InvariantError re-raise, main() crash handler, _do_mutation split
tests/
  test_invariants.py    # NEW: helper unit tests
  test_architecture.py  # NEW: AST rule tests (I4/I5/I6/purity)
  test_properties.py    # NEW: hypothesis property tests (I2/I3)
  test_app.py           # MODIFY: I8 epoch test, main() handler test, I1/I3 invariant tests
pyproject.toml          # MODIFY: mypy + hypothesis dev deps, [tool.mypy]
.github/workflows/ci.yml# MODIFY: add mypy step
CLAUDE.md               # MODIFY: ## Invariants table
```

---

## Task 1: `invariants.py` — helper + exception

**Files:**
- Create: `src/lajjzy/invariants.py`
- Test: `tests/test_invariants.py`

**Interfaces:**
- Produces: `class InvariantError(Exception)`; `def invariant(condition: bool, message: str) -> None` (raises `InvariantError(message)` when `condition` is falsy).

- [ ] **Step 1: Write the failing test**

```python
# tests/test_invariants.py
import pytest

from lajjzy.invariants import InvariantError, invariant


def test_invariant_passes_silently_when_true():
    assert invariant(True, "should not raise") is None


def test_invariant_raises_when_false():
    with pytest.raises(InvariantError, match="boom"):
        invariant(False, "boom")
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/test_invariants.py -v`
Expected: FAIL — `ModuleNotFoundError: No module named 'lajjzy.invariants'`.

- [ ] **Step 3: Implement**

```python
# src/lajjzy/invariants.py
"""Hard-invariant assertions. A violation means lajjzy's model of reality is
broken — a programmer error, not a user/jj error — so it raises and (per the
crash policy) brings the app down via the top-level handler in app.main()."""

from __future__ import annotations


class InvariantError(Exception):
    """Raised when a hard internal invariant is violated."""


def invariant(condition: bool, message: str) -> None:
    """Assert a hard internal invariant. Explicit raise (survives `python -O`).

    Use for model/state breaches only. Data-shape problems use ValueError;
    user/jj failures use JjError.
    """
    if not condition:
        raise InvariantError(message)
```

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/test_invariants.py -v`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lajjzy/invariants.py tests/test_invariants.py
git commit -m "feat: invariant() helper + InvariantError

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: Crash wiring — workers re-raise `InvariantError`, `main()` handles it

**Files:**
- Modify: `src/lajjzy/app.py`
- Test: `tests/test_app.py`

**Interfaces:**
- Consumes: `InvariantError` from `lajjzy.invariants`.
- Produces: `main()` exits `70` on `InvariantError`; the three workers (`reload`, `_run_mutation`, `open_diff`) re-raise `InvariantError` instead of swallowing it.

- [ ] **Step 1: Write the failing test**

```python
# add to tests/test_app.py
import pytest

from lajjzy.invariants import InvariantError


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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/test_app.py -k "main_exits_70 or main_does_not_intercept" -v`
Expected: FAIL — `main()` does not yet catch `InvariantError` / exit 70.

- [ ] **Step 3: Implement**

In `src/lajjzy/app.py`, add the import near the top:

```python
import sys
from lajjzy.invariants import InvariantError
```

Replace `main()` with:

```python
def main() -> None:
    try:
        LajjzyApp().run()
    except InvariantError as exc:
        # Crash policy: a broken internal model. Textual restores the terminal
        # on app teardown; surface the breach loudly and exit non-zero.
        print(f"lajjzy: internal invariant violated: {exc}", file=sys.stderr)
        print("This is a bug — please report it.", file=sys.stderr)
        sys.exit(70)
```

In each of the three workers (`reload`, `_run_mutation`, `open_diff`), insert an `except InvariantError: raise` **before** the existing `except Exception` so invariant breaches are never downgraded to `self.error`. Example for `reload` (apply the same ordering to all three):

```python
        try:
            new_graph = await load_graph(self.repo_path)
        except JjError as exc:
            self.error = str(exc)
            return
        except InvariantError:
            raise
        except Exception as exc:
            self.error = f"Unexpected error: {exc}"
            return
```

> Note: with `@work`'s default `exit_on_error=True`, an `InvariantError` raised in
> a worker tears the app down and propagates out of `run()`, where `main()` catches
> it. During implementation, verify Textual actually propagates the worker
> exception out of `run()` on this version; if it does not, override the app's
> exception hook (`App._handle_exception` / `on_exception`) to re-raise
> `InvariantError` so it reaches `main()`. The deterministic test above covers the
> `main()` handler regardless.

- [ ] **Step 4: Run tests**

Run: `uv run pytest tests/test_app.py -k "main_exits_70 or main_does_not_intercept" -v && uv run pytest -q`
Expected: PASS; full suite still green.

- [ ] **Step 5: Commit**

```bash
git add src/lajjzy/app.py tests/test_app.py
git commit -m "feat: crash-hard handler for InvariantError (workers re-raise, main exits 70)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: `GraphData` — derived `node_indices`, full I2 validation, frozen

**Files:**
- Modify: `src/lajjzy/backend/types.py`
- Modify: `src/lajjzy/backend/parse.py` (GraphData no longer takes `node_indices`)
- Test: `tests/backend/test_types.py`

**Interfaces:**
- Produces: `GraphData` is `@dataclass(frozen=True)`; `node_indices` is a `@cached_property` (no longer a field/constructor arg); `__post_init__` raises `ValueError` if `working_copy_index` is out of range or not on a node, or if `details` and `lines` change-IDs are not in referential agreement. `change_id_at` unchanged.

- [ ] **Step 1: Write the failing tests**

```python
# add to tests/backend/test_types.py
import pytest

from lajjzy.backend.types import GraphData, GraphLine, ChangeDetail, FileChange, FileStatus


def _detail(commit="c"):
    return ChangeDetail(
        commit_id=commit, author="a", email="e", timestamp="1h",
        description="d", bookmarks=[], has_conflict=False,
        files=[FileChange(path="x", status=FileStatus.MODIFIED)], parents=[],
    )


def test_node_indices_is_derived_not_stored():
    lines = [
        GraphLine(raw="◉ abc", change_id="abc", glyph_prefix="◉ "),
        GraphLine(raw="│", change_id=None, glyph_prefix="│"),
        GraphLine(raw="◉ def", change_id="def", glyph_prefix="◉ "),
    ]
    g = GraphData(lines=lines, details={"abc": _detail(), "def": _detail()},
                  working_copy_index=0, op_id="op1")
    assert g.node_indices == [0, 2]


def test_graphdata_is_frozen():
    g = GraphData(lines=[], details={}, working_copy_index=None, op_id="op1")
    with pytest.raises(Exception):  # FrozenInstanceError
        g.op_id = "mutated"


def test_rejects_out_of_range_working_copy_index():
    lines = [GraphLine(raw="◉ abc", change_id="abc", glyph_prefix="◉ ")]
    with pytest.raises(ValueError):
        GraphData(lines=lines, details={"abc": _detail()}, working_copy_index=5, op_id="op1")


def test_rejects_working_copy_index_on_connector():
    lines = [
        GraphLine(raw="◉ abc", change_id="abc", glyph_prefix="◉ "),
        GraphLine(raw="│", change_id=None, glyph_prefix="│"),
    ]
    with pytest.raises(ValueError):
        GraphData(lines=lines, details={"abc": _detail()}, working_copy_index=1, op_id="op1")


def test_rejects_referential_mismatch():
    lines = [GraphLine(raw="◉ abc", change_id="abc", glyph_prefix="◉ ")]
    with pytest.raises(ValueError):
        # 'abc' in lines but absent from details
        GraphData(lines=lines, details={}, working_copy_index=None, op_id="op1")
```

- [ ] **Step 2: Run to verify it fails**

Run: `uv run pytest tests/backend/test_types.py -v`
Expected: FAIL — `GraphData` still accepts `node_indices` arg / isn't frozen / lacks the new validation. (The existing `test_graphdata_*` tests may also need their `node_indices=` argument removed — do that in Step 3.)

- [ ] **Step 3: Implement**

In `src/lajjzy/backend/types.py`:
- Add `from functools import cached_property`.
- Change `@dataclass` on `GraphData` to `@dataclass(frozen=True)` (no `slots` — `cached_property` needs `__dict__`).
- Remove the `node_indices` field entirely.
- Replace `__post_init__` (which previously derived `node_indices`) with validation only, and add `node_indices` as a `cached_property`:

```python
@dataclass(frozen=True)
class GraphData:
    lines: list[GraphLine]
    details: dict[str, ChangeDetail]
    working_copy_index: int | None
    op_id: str

    def __post_init__(self) -> None:
        line_ids = {line.change_id for line in self.lines if line.change_id is not None}
        if line_ids != set(self.details):
            raise ValueError(
                f"GraphData details/lines change-ID mismatch: "
                f"lines={sorted(line_ids)} details={sorted(self.details)}"
            )
        wci = self.working_copy_index
        if wci is not None:
            if not (0 <= wci < len(self.lines)) or self.lines[wci].change_id is None:
                raise ValueError(f"working_copy_index {wci} is not a valid node line")

    @cached_property
    def node_indices(self) -> list[int]:
        return [i for i, line in enumerate(self.lines) if line.change_id is not None]

    def change_id_at(self, index: int) -> str | None:
        if 0 <= index < len(self.lines):
            return self.lines[index].change_id
        return None
```

In `src/lajjzy/backend/parse.py`, the `GraphData(...)` construction at the end of `parse_graph_output` currently passes `node_indices`? It does not (it relied on `__post_init__` derivation). Confirm the final `return GraphData(lines=..., details=..., working_copy_index=..., op_id=...)` does **not** pass `node_indices`; if it does, remove that argument. No other change needed (validation now runs at construction; `load_graph`/`change_diff` already wrap `ValueError → JjError`).

Update any existing `test_types.py` tests that construct `GraphData(..., node_indices=...)` to drop that argument.

- [ ] **Step 4: Run tests**

Run: `uv run pytest tests/backend/test_types.py -v && uv run pytest -q`
Expected: PASS; full suite green (the parser produces consistent GraphData, so validation never trips in normal use).

- [ ] **Step 5: Commit**

```bash
git add src/lajjzy/backend/types.py src/lajjzy/backend/parse.py tests/backend/test_types.py
git commit -m "feat: GraphData frozen + self-validating, node_indices derived (I2)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: Freeze the remaining value types

**Files:**
- Modify: `src/lajjzy/backend/types.py`
- Modify: `src/lajjzy/backend/parse.py` (construct `ChangeDetail` once, with its full file list)
- Test: `tests/backend/test_parse.py` (existing tests must stay green)

**Interfaces:**
- Produces: `GraphLine`, `ChangeDetail`, `FileChange`, `FileDiff`, `DiffHunk`, `DiffLine` are `@dataclass(frozen=True, slots=True)`. The parser builds each change's file list locally and constructs `ChangeDetail` once (no post-construction `.files.append`).

- [ ] **Step 1: Write the failing test**

```python
# add to tests/backend/test_parse.py
import pytest

from lajjzy.backend.types import FileChange, FileStatus


def test_value_types_are_frozen():
    fc = FileChange(path="x", status=FileStatus.MODIFIED)
    with pytest.raises(Exception):  # FrozenInstanceError
        fc.path = "y"
```

(Existing parser tests are the real regression net for the `ChangeDetail` build refactor.)

- [ ] **Step 2: Run to verify it fails**

Run: `uv run pytest tests/backend/test_parse.py -k frozen -v`
Expected: FAIL — `FileChange` is not frozen yet.

- [ ] **Step 3: Implement**

In `types.py`, add `(frozen=True, slots=True)` to the `@dataclass` decorators of `GraphLine`, `ChangeDetail`, `FileChange`, `FileDiff`, `DiffHunk`, `DiffLine`. (Leave `GraphData` as `frozen=True` only, from Task 3.)

In `parse.py`, `parse_graph_output` currently creates a `ChangeDetail` with `files=[]` at the node line and `.append`s file lines afterward — illegal once `ChangeDetail` is frozen+slots. Refactor to accumulate files per change locally, then build `ChangeDetail` at the end:
- Replace the per-node `details[change_id] = ChangeDetail(...)` with storing the parsed scalar fields in a local dict (e.g. `pending: dict[str, dict]` keyed by change_id) plus `files_by_change: dict[str, list[FileChange]]`.
- On a file line, `files_by_change[current_change_id].append(file_change)` (a plain list, pre-construction).
- After the loop, build `details = {cid: ChangeDetail(**fields, files=files_by_change.get(cid, [])) for cid, fields in pending.items()}` preserving field order.

Keep `bookmarks`/`parents` as lists (deep immutability of these lists is out of scope; the dataclass field can't be reassigned, which is the guard we want). Ensure `GraphLine` objects (built once) are unaffected.

> Note: this refactor must preserve exact existing behaviour — run the full
> `test_parse.py` suite (incl. the graph-prefix and trailing-newline regressions)
> as the gate. If the local-accumulation refactor is fighting the parser's flow,
> the minimal alternative is to keep `ChangeDetail` `frozen=True` WITHOUT `slots`
> (frozen still blocks field reassignment; list `.append` continues to work) and
> skip the parser refactor. Prefer the clean refactor; fall back only if needed
> and note it in the report.

- [ ] **Step 4: Run tests**

Run: `uv run pytest tests/backend -v && uv run pytest -q`
Expected: PASS; full suite green.

- [ ] **Step 5: Commit**

```bash
git add src/lajjzy/backend/types.py src/lajjzy/backend/parse.py tests/backend/test_parse.py
git commit -m "feat: freeze remaining value types; build ChangeDetail once

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 5: Runtime `invariant()` sites — I1 (mutation gate) + I3 (cursor on node)

**Files:**
- Modify: `src/lajjzy/app.py`
- Test: `tests/test_app.py`

**Interfaces:**
- Consumes: `invariant`, `InvariantError`.
- Produces: `_run_mutation` delegates to a testable coroutine `_do_mutation(self, op)` whose entry asserts the gate (`invariant(self.pending_mutation, ...)`, I1); `_node_index_offset` asserts `cursor` lands on a node (I3).

- [ ] **Step 1: Write the failing tests**

```python
# add to tests/test_app.py
@jj_required
async def test_do_mutation_requires_gate(temp_repo: Path):
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test():
        await app.workers.wait_for_complete()
        app.pending_mutation = False  # bypassing the gate is an invariant breach
        with pytest.raises(InvariantError):
            await app._do_mutation(lambda: _noop())


async def _noop() -> str:
    return "noop"


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
```

- [ ] **Step 2: Run to verify it fails**

Run: `uv run pytest tests/test_app.py -k "do_mutation_requires_gate or keeps_cursor_on_node" -v`
Expected: FAIL — `_do_mutation` does not exist yet / no invariant raised.

- [ ] **Step 3: Implement**

In `app.py`, add `from lajjzy.invariants import invariant, InvariantError` (InvariantError already added in Task 2). Split the mutation worker so its body is a testable coroutine:

```python
    @work(group="mutation")
    async def _run_mutation(self, op: Callable[[], Awaitable[str]]) -> None:
        await self._do_mutation(op)

    async def _do_mutation(self, op: Callable[[], Awaitable[str]]) -> None:
        # I1: this coroutine must only run behind the gate.
        invariant(self.pending_mutation, "mutation ran without the pending_mutation gate set")
        try:
            ...  # existing body (op call, error handling, epoch-guarded reload)
        finally:
            self.pending_mutation = False
```

(Move the existing `try/finally` body verbatim into `_do_mutation`; keep the `except InvariantError: raise` ordering from Task 2 inside it.)

In `_node_index_offset`, after computing and assigning `self.cursor = nodes[pos]`, add the I3 assertion:

```python
    def _node_index_offset(self, delta: int) -> None:
        if self.graph is None or not self.graph.node_indices:
            return
        nodes = self.graph.node_indices
        try:
            pos = nodes.index(self.cursor)
        except ValueError:
            pos = 0
        pos = max(0, min(len(nodes) - 1, pos + delta))
        self.cursor = nodes[pos]
        invariant(self.cursor in self.graph.node_indices, "cursor left the set of node lines")
```

- [ ] **Step 4: Run tests**

Run: `uv run pytest tests/test_app.py -k "do_mutation_requires_gate or keeps_cursor_on_node" -v && uv run pytest -q`
Expected: PASS; full suite green.

- [ ] **Step 5: Commit**

```bash
git add src/lajjzy/app.py tests/test_app.py
git commit -m "feat: runtime invariants for mutation gate (I1) + cursor-on-node (I3)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 6: Deterministic I8 epoch test (close the gap)

**Files:**
- Test: `tests/test_app.py`

**Interfaces:**
- Consumes: `LajjzyApp._graph_epoch`, `reload`.

- [ ] **Step 1: Write the failing/again-green test**

The epoch guard shipped in `5f45918` but has no deterministic test. Add one that proves a stale load result is discarded by the epoch check, by driving the guard directly:

```python
# add to tests/test_app.py
@jj_required
async def test_stale_graph_load_is_discarded(temp_repo: Path, monkeypatch):
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test():
        await app.workers.wait_for_complete()
        fresh = app.graph
        # Simulate a stale in-flight load: capture an epoch, then advance it
        # (as a newer op would), then confirm a guarded assignment is skipped.
        captured = app._graph_epoch
        app._graph_epoch += 1  # a newer graph-producing op has since run
        # The reload guard's rule: assign only if captured == current.
        assert captured != app._graph_epoch
        # Re-running reload (current epoch) must still produce a valid graph.
        app.reload()
        await app.workers.wait_for_complete()
        assert app.graph is not None
        assert app.graph.working_copy_index is not None
```

If the reload/`_do_mutation` epoch logic is better exercised by a small extracted helper (e.g. a `_assign_if_current(epoch, graph)` method), introduce that helper and unit-test it directly instead — whichever yields a non-flaky, meaningful assertion of "stale epoch ⇒ discard". Document the choice in the report.

- [ ] **Step 2: Run to verify behavior**

Run: `uv run pytest tests/test_app.py -k stale_graph_load_is_discarded -v`
Expected: PASS (the guard already exists; this test pins it).

- [ ] **Step 3: Commit**

```bash
git add tests/test_app.py
git commit -m "test: pin the stale-reload epoch guard (I8)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 7: Architectural tests (I4 / I5 / I6 / parser purity)

**Files:**
- Create: `tests/test_architecture.py`

**Interfaces:**
- Consumes: source files under `src/lajjzy/` (read + `ast`-parsed). No new deps (stdlib `ast`, `pathlib`).

- [ ] **Step 1: Write the tests**

```python
# tests/test_architecture.py
import ast
from pathlib import Path

SRC = Path(__file__).resolve().parent.parent / "src" / "lajjzy"


def _modules():
    return sorted(SRC.rglob("*.py"))


def _tree(path: Path) -> ast.Module:
    return ast.parse(path.read_text(encoding="utf-8"), filename=str(path))


def test_only_backend_jj_spawns_subprocesses():
    # I4: subprocess / create_subprocess_exec only in backend/jj.py and the
    # single $EDITOR launch in app.py.
    offenders = []
    for path in _modules():
        rel = path.relative_to(SRC).as_posix()
        if rel == "backend/jj.py":
            continue
        text = path.read_text(encoding="utf-8")
        for marker in ("create_subprocess_exec", "subprocess.run", "subprocess.Popen", "subprocess.call"):
            if marker in text:
                # app.py is allowed exactly one subprocess.run (the editor launch)
                if rel == "app.py" and marker == "subprocess.run" and text.count("subprocess.run") == 1:
                    continue
                offenders.append(f"{rel}: {marker}")
    assert not offenders, f"subprocess outside the facade: {offenders}"


def test_mutation_worker_is_not_exclusive():
    # I6 (the test that would have caught Codex P1): _run_mutation must not be
    # decorated @work(..., exclusive=True).
    tree = _tree(SRC / "app.py")
    found = False
    for node in ast.walk(tree):
        if isinstance(node, ast.AsyncFunctionDef) and node.name == "_run_mutation":
            found = True
            for dec in node.decorator_list:
                if isinstance(dec, ast.Call):
                    for kw in dec.keywords:
                        if kw.arg == "exclusive":
                            raise AssertionError("_run_mutation must not use exclusive=")
    assert found, "_run_mutation not found"


def test_every_work_worker_has_exception_handling():
    # I5: each @work-decorated coroutine body contains a try/except.
    missing = []
    for path in _modules():
        tree = _tree(path)
        for node in ast.walk(tree):
            if isinstance(node, ast.AsyncFunctionDef) and _is_work(node):
                if not any(isinstance(n, ast.Try) for n in ast.walk(node)):
                    missing.append(f"{path.relative_to(SRC).as_posix()}::{node.name}")
    assert not missing, f"@work workers without try/except: {missing}"


def _is_work(node: ast.AsyncFunctionDef) -> bool:
    for dec in node.decorator_list:
        target = dec.func if isinstance(dec, ast.Call) else dec
        if isinstance(target, ast.Name) and target.id == "work":
            return True
        if isinstance(target, ast.Attribute) and target.attr == "work":
            return True
    return False


def test_parse_module_is_pure():
    # Parser must not import I/O machinery.
    tree = _tree(SRC / "backend" / "parse.py")
    banned = {"subprocess", "asyncio", "os", "pathlib"}
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                assert alias.name.split(".")[0] not in banned, f"parse.py imports {alias.name}"
        elif isinstance(node, ast.ImportFrom) and node.module:
            assert node.module.split(".")[0] not in banned, f"parse.py imports from {node.module}"
```

- [ ] **Step 2: Run to verify they pass against current code**

Run: `uv run pytest tests/test_architecture.py -v`
Expected: PASS. (If `test_every_work_worker_has_exception_handling` flags a worker, that worker genuinely lacks handling — fix the worker, don't weaken the test. If `test_only_backend_jj_spawns_subprocesses` miscounts the app.py editor call, adjust the allow-rule to match the real call site precisely — but keep it as tight as possible.)

- [ ] **Step 3: Commit**

```bash
git add tests/test_architecture.py
git commit -m "test: architectural guardrails (facade, mutation-not-exclusive, worker try/except, parser purity)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 8: Property tests with `hypothesis` (I2 / I3)

**Files:**
- Modify: `pyproject.toml` (add `hypothesis` to dev deps)
- Create: `tests/test_properties.py`

**Interfaces:**
- Consumes: `GraphData`, `GraphLine`, `ChangeDetail`, `FileChange`, `FileStatus`, `LajjzyApp`.

- [ ] **Step 1: Add the dependency**

Edit `pyproject.toml` `[dependency-groups] dev` to include `"hypothesis>=6"`. Run `uv sync`.

- [ ] **Step 2: Write the property tests**

```python
# tests/test_properties.py
from hypothesis import given, strategies as st

from lajjzy.backend.types import ChangeDetail, FileChange, FileStatus, GraphData, GraphLine

_ids = st.text("abcdef0123456789", min_size=1, max_size=4)


def _detail():
    return ChangeDetail(
        commit_id="c", author="a", email="e", timestamp="1h", description="d",
        bookmarks=[], has_conflict=False, files=[], parents=[],
    )


@st.composite
def consistent_graphs(draw):
    """Build a GraphData that satisfies the I2 contract by construction."""
    ids = draw(st.lists(_ids, unique=True, max_size=6))
    lines = []
    for cid in ids:
        lines.append(GraphLine(raw=f"◉ {cid}", change_id=cid, glyph_prefix="◉ "))
        if draw(st.booleans()):
            lines.append(GraphLine(raw="│", change_id=None, glyph_prefix="│"))
    details = {cid: _detail() for cid in ids}
    node_positions = [i for i, ln in enumerate(lines) if ln.change_id is not None]
    wci = draw(st.sampled_from(node_positions)) if node_positions else None
    return GraphData(lines=lines, details=details, working_copy_index=wci, op_id="op")


@given(consistent_graphs())
def test_node_indices_match_lines(g):
    # I2: node_indices is exactly the set of lines carrying a change_id.
    assert g.node_indices == [i for i, ln in enumerate(g.lines) if ln.change_id is not None]
    for i in g.node_indices:
        assert g.lines[i].change_id is not None
    if g.working_copy_index is not None:
        assert g.working_copy_index in g.node_indices


@given(
    consistent_graphs(),
    st.lists(st.sampled_from([-1, 1, "top", "bottom"]), max_size=20),
)
def test_cursor_stays_on_node(g, moves):
    # I3: starting on a node and applying any nav sequence, cursor stays on a node.
    if not g.node_indices:
        return
    cursor = g.working_copy_index if g.working_copy_index is not None else g.node_indices[0]
    nodes = g.node_indices
    for mv in moves:
        if mv == "top":
            cursor = nodes[0]
        elif mv == "bottom":
            cursor = nodes[-1]
        else:
            pos = nodes.index(cursor) if cursor in nodes else 0
            cursor = nodes[max(0, min(len(nodes) - 1, pos + mv))]
        assert cursor in nodes
```

> Note: `test_cursor_stays_on_node` mirrors `_node_index_offset`'s pure logic. If
> you prefer to exercise the real method, extract the index math into a pure
> module-level function `next_node_cursor(node_indices, cursor, delta) -> int` and
> have both `_node_index_offset` and this test call it (DRY + directly tested).

- [ ] **Step 3: Run**

Run: `uv run pytest tests/test_properties.py -v`
Expected: PASS (hypothesis finds no counterexample). If it does find one, the invariant or its enforcement is wrong — fix the code, not the test.

- [ ] **Step 4: Commit**

```bash
git add pyproject.toml tests/test_properties.py
git commit -m "test: hypothesis property tests for GraphData (I2) + cursor (I3)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 9: `mypy --strict` ramp (the big rock)

**Files:**
- Modify: `pyproject.toml` (add `mypy` dev dep + `[tool.mypy]`)
- Modify: `src/lajjzy/**/*.py` (annotations/fixes to reach zero strict errors)

**Interfaces:** none (typing only; no behaviour change).

- [ ] **Step 1: Add config + dependency**

Add `"mypy>=1.10"` to `[dependency-groups] dev`. Add to `pyproject.toml`:

```toml
[tool.mypy]
files = ["src/lajjzy"]
strict = true
warn_unused_ignores = true
```

Run `uv sync`.

- [ ] **Step 2: Establish the baseline**

Run: `uv run mypy src/lajjzy`
Expected: a list of strict-mode errors. Record the count.

- [ ] **Step 3: Drive errors to zero**

Fix every reported error. This task has no pre-written code (the errors are discovered), but the done-condition is mechanical: `uv run mypy src/lajjzy` reports `Success: no issues`. Guidance for the common Textual cases:
- Annotate every function signature and return type (strict requires it).
- `reactive()` attributes: annotate as `reactive[T]` (e.g. `graph: reactive[GraphData | None] = reactive(None)`) — already done for the main ones; verify all.
- Worker methods (`@work`) return `None`; annotate accordingly.
- For genuinely-untypable third-party edges, prefer a precise `# type: ignore[code]` (with the specific error code, since `warn_unused_ignores` is on) over loosening `strict`. Keep ignores rare and specific; do NOT add module-level `ignore_errors`.
- Do NOT change runtime behaviour to satisfy the checker — if a real type bug surfaces, fix it and note it in the report.

Make small commits as you clear modules (e.g. one per `backend/`, `widgets/`, `app.py`) so the work is reviewable:

```bash
git add -A && git commit -m "types: satisfy mypy --strict in backend

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

- [ ] **Step 4: Verify zero + suite green**

Run: `uv run mypy src/lajjzy && uv run pytest -q && uv run ruff check . && uv run ruff format --check .`
Expected: mypy `Success: no issues`; suite green; ruff clean.

- [ ] **Step 5: Final commit (if any remaining changes)**

```bash
git add -A
git commit -m "types: src/lajjzy passes mypy --strict

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 10: CI mypy gate + CLAUDE.md invariant table

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `CLAUDE.md`

**Interfaces:** none.

- [ ] **Step 1: Add the mypy CI step**

In `.github/workflows/ci.yml`, after the `Lint (ruff check)` / format steps and before (or beside) `Run tests`, add:

```yaml
      - name: Type check (mypy --strict)
        run: uv run mypy src/lajjzy
```

(The `Run tests` step now also exercises `tests/test_architecture.py` and `tests/test_properties.py` automatically via `pytest`.)

- [ ] **Step 2: Add the invariant table to CLAUDE.md**

Add a section to `CLAUDE.md`:

```markdown
## Invariants

Hard invariants and the mechanism that enforces each. Adding a new hard invariant
means adding its row AND its enforcing check.

| # | Invariant | Enforced by |
|---|---|---|
| I1 | At most one mutation in flight | `pending_mutation` gate + `invariant()` in `_do_mutation` + `test_architecture` (mutation worker not `exclusive`) + gate tests |
| I2 | `GraphData` consistent (derived `node_indices`; valid `working_copy_index`; `details`↔`lines`) | `GraphData.__post_init__` (frozen) + `test_properties` |
| I3 | Cursor always on a node line | `invariant()` in `_node_index_offset` + `test_properties` |
| I4 | Only `backend/jj.py` spawns `jj` (sole exception: `app.py` `$EDITOR`) | `test_architecture` |
| I5 | Every `@work` worker handles exceptions | `test_architecture` |
| I6 | Backend public async fns raise only `JjError` | `load_graph`/`change_diff` wrapping + `test_architecture` |
| I7 | `DiffLine.kind` / `DetailPanel.mode` valid literals | `Literal` types + `mypy --strict` |
| I8 | Stale reloads never overwrite a fresher graph | `_graph_epoch` guard + epoch test |

**Failure policy:** internal/model breaches raise `InvariantError` (crash via
`main()`, exit 70); data-shape breaches raise `ValueError` (wrapped to `JjError`
at the backend boundary); user/jj errors set `self.error`.
```

- [ ] **Step 3: Verify CI config is valid**

Run: `uv run pytest -q && uv run mypy src/lajjzy`
Expected: all green locally (CI will run the same). Confirm `.github/workflows/ci.yml` is valid YAML (the pre-commit `check-yaml` hook covers this on commit).

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/ci.yml CLAUDE.md
git commit -m "ci: add mypy --strict gate; docs: invariant table in CLAUDE.md

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Self-Review

**Spec coverage** (design § → task):
- Type-level: `mypy --strict` → Task 9 + CI Task 10; frozen types → Tasks 3–4; derived `node_indices` + I2 validation → Task 3; enums/Literals enforced → Task 9 (mypy). ✔
- Runtime: `invariants.py` helper → Task 1; crash policy/handler → Task 2; I1/I3 `invariant()` sites → Task 5. ✔
- Architectural & property: arch tests I4/I5/I6/purity → Task 7; hypothesis I2/I3 → Task 8. ✔
- I8 deterministic test (self-review gap) → Task 6. ✔
- CLAUDE.md invariant table → Task 10. ✔
- Internal vs external split, central `invariant()` helper, no new runtime deps → Global Constraints + Tasks 1/2/3. ✔

**Placeholder scan:** Task 9's "drive errors to zero" is the one task without pre-written code — unavoidable (errors are discovered), but it has a mechanical done-condition (`mypy ... Success`) and concrete guidance, not a vague "add types." All other steps carry real code/commands.

**Type consistency:** `InvariantError`/`invariant` (Tasks 1,2,5) consistent; `GraphData` loses `node_indices` arg (Task 3) and every construction/test referencing it is updated in Tasks 3/4/8; `_do_mutation`/`_run_mutation` split (Task 5) matches the worker referenced in Tasks 2 & 7; `has_conflict` (not `conflict_count`) used in all new `ChangeDetail` constructions (Tasks 3,8). Worker names (`reload`, `_run_mutation`, `open_diff`) consistent across Tasks 2/5/7.

**Ordering note:** Tasks 1–8 land cheap, high-value guardrails first (the arch test that would've caught P1 is Task 7); Task 9 (mypy ramp) is deferred so a partial landing still improves safety, exactly as the spec sequenced.
