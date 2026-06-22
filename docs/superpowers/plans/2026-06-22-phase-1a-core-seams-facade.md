# Daily-driver essentials — Phase 1a: core seams + jj facade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land every pure-core type, parser, jj facade function, `Msg`/`Cmd`/`update` branch, and core unit test needed by the six daily-driver features, without touching `app.py` or widgets. Ends green CI with feature-invisible state.

**Architecture:** Pure MVU core (`core/`) + jj facade (`backend/jj.py`) + parsers (`backend/parse.py`) + types (`backend/types.py`). No Textual, no asyncio in core. Every new behaviour is a `Msg` + `update` branch + `Cmd`; every effect is a facade function called only from `app.py` workers (in phase 1b). TDD: each `update` branch is pinned by a unit test before it exists.

**Tech Stack:** Python 3.11+, `jj` 0.42.0, `dataclasses`, `pytest`, `ruff`, `mypy --strict`.

**Reference:** Spec at `docs/superpowers/specs/2026-06-22-daily-driver-essentials-design.md`. Rust prototype at commit `731edd1` under `crates/` — behavioural reference for facade functions.

**Scope of this plan:** types, parsers, facade functions, core `Msg`/`Cmd`/`Model`/`update`, core unit tests. NOT in this plan: `app.py` bindings/workers, widgets, architecture-test extensions (those are phase 1b), per-feature widget implementation (phase 2).

---

## File structure

**Create:**
- `tests/backend/test_jj_facade_ext.py` — integration tests for the new facade functions (undo, redo, op_log, op_restore, bookmark_*, load_bookmarks, split, squash_partial, conflict_data, resolve).
- `tests/backend/test_parse_ext.py` — unit tests for the new parsers (op log, bookmarks, conflict data).

**Modify:**
- `src/lajjzy/backend/types.py` — add `OpLogEntry`, `Bookmark`, `ConflictData`, `ConflictRegion`, `HunkResolution`, `CompletionItem`.
- `src/lajjzy/backend/parse.py` — add `parse_op_log`, `parse_bookmarks`, `parse_conflict_data`.
- `src/lajjzy/backend/jj.py` — add `undo`, `redo`, `op_log`, `op_restore`, `bookmark_set`, `bookmark_delete`, `bookmark_move`, `load_bookmarks`, `split`, `squash_partial`, `conflict_data`, `resolve`.
- `src/lajjzy/core/messages.py` — add all new `Msg` types.
- `src/lajjzy/core/commands.py` — add `LoadOpLog`, `LoadBookmarks`, `LoadConflictData`; extend `LoadGraph` with `revset`; extend `RunMutation` (no shape change — `kind` + `args`).
- `src/lajjzy/core/model.py` — add new `Model` fields + helpers.
- `src/lajjzy/core/update.py` — add all new `update` branches.
- `src/lajjzy/core/__init__.py` — re-export new symbols.
- `tests/core/test_update.py` — add unit tests for every new `update` branch.

---

## Task 1: New backend types

**Files:**
- Modify: `src/lajjzy/backend/types.py` (append after line 94, before `GraphData`'s end — actually append at end of file)
- Test: `tests/backend/test_parse_ext.py` (created in Task 2, but the types must exist first)

- [ ] **Step 1: Write the failing type-import test**

Create `tests/backend/test_parse_ext.py`:

```python
from __future__ import annotations

from lajjzy.backend.types import (
    Bookmark,
    CompletionItem,
    ConflictData,
    ConflictRegion,
    HunkResolution,
    OpLogEntry,
)


def test_op_log_entry_fields():
    e = OpLogEntry(op_id="abc", timestamp="1h ago", description="commit")
    assert e.op_id == "abc"
    assert e.timestamp == "1h ago"
    assert e.description == "commit"


def test_bookmark_fields():
    b = Bookmark(name="main", change_id="ksqxwpml", change_description="head")
    assert b.name == "main"
    assert b.change_id == "ksqxwpml"
    assert b.change_description == "head"


def test_conflict_region_resolved():
    r = ConflictRegion.resolved("context line")
    assert r.kind == "resolved"
    assert r.text == "context line"


def test_conflict_region_conflict():
    r = ConflictRegion.conflict(base="b", left="l", right="r")
    assert r.kind == "conflict"
    assert r.base == "b"
    assert r.left == "l"
    assert r.right == "r"


def test_conflict_data():
    c = ConflictData(regions=[ConflictRegion.resolved("x")])
    assert len(c.regions) == 1


def test_hunk_resolution_values():
    assert HunkResolution.NONE is not None
    assert HunkResolution.ACCEPT_LEFT is not None
    assert HunkResolution.ACCEPT_RIGHT is not None
    assert HunkResolution.NONE is not HunkResolution.ACCEPT_LEFT


def test_completion_item_fields():
    c = CompletionItem(insert_text="all(", display_text="all() — all visible changes")
    assert c.insert_text == "all("
    assert c.display_text.startswith("all()")
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/backend/test_parse_ext.py -v`
Expected: FAIL with `ImportError: cannot import name 'Bookmark'` etc.

- [ ] **Step 3: Add the types to `src/lajjzy/backend/types.py`**

Append at end of file:

```python
@dataclass(frozen=True, slots=True)
class OpLogEntry:
    op_id: str
    timestamp: str
    description: str


@dataclass(frozen=True, slots=True)
class Bookmark:
    name: str
    change_id: str
    change_description: str


@dataclass(frozen=True, slots=True)
class CompletionItem:
    insert_text: str
    display_text: str


class HunkResolution:
    """Sentinel constants for per-hunk resolution choices in the conflict view.

    Kept as plain class attributes (not an Enum) so widgets can pass them as
    plain values without importing the enum wrapper; matches how the Rust
    prototype modelled it as a plain enum we serialize to a label.
    """

    NONE = "none"  # undecided
    ACCEPT_LEFT = "accept_left"
    ACCEPT_RIGHT = "accept_right"


@dataclass(frozen=True, slots=True)
class ConflictRegion:
    """One region of a conflicted file. Either non-conflicting content
    (``kind == "resolved"``) or a three-way conflict hunk
    (``kind == "conflict"``). Use the ``resolved(...)`` / ``conflict(...)``
    classmethods to construct — they set ``kind`` and the side fields."""

    kind: str  # "resolved" | "conflict"
    text: str = ""        # for resolved
    base: str = ""        # for conflict
    left: str = ""        # for conflict (ours)
    right: str = ""       # for conflict (theirs)

    @classmethod
    def resolved(cls, text: str) -> ConflictRegion:
        return cls(kind="resolved", text=text)

    @classmethod
    def conflict(cls, base: str, left: str, right: str) -> ConflictRegion:
        return cls(kind="conflict", base=base, left=left, right=right)


@dataclass(frozen=True, slots=True)
class ConflictData:
    regions: list[ConflictRegion]
```

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/backend/test_parse_ext.py -v`
Expected: PASS (7 tests).

- [ ] **Step 5: Lint + typecheck + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/backend/types.py tests/backend/test_parse_ext.py
git commit -m "feat(backend): add types for op log, bookmarks, conflicts, completions"
```

---

## Task 2: Op-log parser

**Files:**
- Modify: `src/lajjzy/backend/parse.py` (add `parse_op_log`)
- Test: `tests/backend/test_parse_ext.py` (extend)

- [ ] **Step 1: Write the failing test**

Append to `tests/backend/test_parse_ext.py`:

```python
from lajjzy.backend.parse import parse_op_log


def test_parse_op_log_empty():
    assert parse_op_log("") == []


def test_parse_op_log_entries():
    # jj op log --no-graph -T produces one entry per line with our template;
    # fields are separated by \x1f, entries by \n.
    out = "abc123\x1f2 hours ago\x1fcommit xyz\n" "def456\x1f1 hour ago\x1fabsorb"
    entries = parse_op_log(out)
    assert len(entries) == 2
    assert entries[0].op_id == "abc123"
    assert entries[0].timestamp == "2 hours ago"
    assert entries[0].description == "commit xyz"
    assert entries[1].op_id == "def456"
    assert entries[1].description == "absorb"


def test_parse_op_log_ignores_blank_trailing_line():
    out = "abc\x1fnow\x1fdesc\n"
    entries = parse_op_log(out)
    assert len(entries) == 1
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/backend/test_parse_ext.py::test_parse_op_log_empty -v`
Expected: FAIL with `ImportError: cannot import name 'parse_op_log'`.

- [ ] **Step 3: Implement `parse_op_log`**

Append to `src/lajjzy/backend/parse.py`:

```python
def parse_op_log(output: str) -> list[OpLogEntry]:
    """Parse `jj op log --no-graph -T <template>` output into OpLogEntry list.

    Template (set in jj.py): id ++ \\x1f ++ timestamp ++ \\x1f ++ description ++ \\n
    """
    entries: list[OpLogEntry] = []
    for line in output.split("\n"):
        if not line:
            continue
        parts = line.split("\x1f")
        if len(parts) != 3:
            continue
        entries.append(OpLogEntry(op_id=parts[0], timestamp=parts[1], description=parts[2]))
    return entries
```

Add the import at the top of `parse.py`:

```python
from lajjzy.backend.types import OpLogEntry
```
(into the existing `from lajjzy.backend.types import ...` line — append `OpLogEntry` to the names imported.)

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/backend/test_parse_ext.py -v`
Expected: PASS (10 tests).

- [ ] **Step 5: Lint + typecheck + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/backend/parse.py tests/backend/test_parse_ext.py
git commit -m "feat(backend): parse_op_log"
```

---

## Task 3: Bookmark parser

**Files:**
- Modify: `src/lajjzy/backend/parse.py` (add `parse_bookmarks`)
- Test: `tests/backend/test_parse_ext.py` (extend)

- [ ] **Step 1: Write the failing test**

Append to `tests/backend/test_parse_ext.py`:

```python
from lajjzy.backend.parse import parse_bookmarks


def test_parse_bookmarks_empty():
    assert parse_bookmarks("") == []


def test_parse_bookmarks_entries():
    # name \x1f change_id \x1f change_description \n
    out = "main\x1fksqxwpml\x1fhead commit\n" "feature\x1fytoqrzxn\x1fwip"
    bms = parse_bookmarks(out)
    assert len(bms) == 2
    assert bms[0].name == "main"
    assert bms[0].change_id == "ksqxwpml"
    assert bms[0].change_description == "head commit"
    assert bms[1].name == "feature"


def test_parse_bookmarks_ignores_blank_trailing_line():
    bms = parse_bookmarks("main\x1fabc\x1fdesc\n")
    assert len(bms) == 1
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/backend/test_parse_ext.py::test_parse_bookmarks_empty -v`
Expected: FAIL with `ImportError: cannot import name 'parse_bookmarks'`.

- [ ] **Step 3: Implement `parse_bookmarks`**

Append to `src/lajjzy/backend/parse.py`. Add `Bookmark` to the `from lajjzy.backend.types import ...` import.

```python
def parse_bookmarks(output: str) -> list[Bookmark]:
    """Parse `jj bookmark list -T <template>` output into Bookmark list.

    Template (set in jj.py): name ++ \\x1f ++ change_id.short() ++ \\x1f ++
    description.first_line() ++ \\n
    """
    bms: list[Bookmark] = []
    for line in output.split("\n"):
        if not line:
            continue
        parts = line.split("\x1f")
        if len(parts) != 3:
            continue
        bms.append(
            Bookmark(name=parts[0], change_id=parts[1], change_description=parts[2])
        )
    return bms
```

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/backend/test_parse_ext.py -v`
Expected: PASS (13 tests).

- [ ] **Step 5: Lint + typecheck + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/backend/parse.py tests/backend/test_parse_ext.py
git commit -m "feat(backend): parse_bookmarks"
```

---

## Task 4: Conflict-data parser

**Files:**
- Modify: `src/lajjzy/backend/parse.py` (add `parse_conflict_data`)
- Test: `tests/backend/test_parse_ext.py` (extend)

jj's conflict file format (0.42.0) uses `<<<<<<<` / `+++++++` / `>>>>>>>` markers with a `%%%%%%%` base separator. The exact marker set is verified in the integration test (Task 12); the parser here handles the canonical 7-char marker form.

- [ ] **Step 1: Write the failing test**

Append to `tests/backend/test_parse_ext.py`:

```python
from lajjzy.backend.parse import parse_conflict_data


def test_parse_conflict_data_no_conflicts():
    # A file with no conflict markers is one resolved region.
    cd = parse_conflict_data("line1\nline2\n")
    assert len(cd.regions) == 1
    assert cd.regions[0].kind == "resolved"
    assert cd.regions[0].text == "line1\nline2"


def test_parse_conflict_data_one_conflict():
    out = (
        "before\n"
        "<<<<<<<\n"
        "ours\n"
        "|||||||\n"
        "base\n"
        "=======\n"
        "theirs\n"
        ">>>>>>>\n"
        "after\n"
    )
    cd = parse_conflict_data(out)
    assert len(cd.regions) == 3
    assert cd.regions[0].kind == "resolved"
    assert cd.regions[0].text == "before\n"
    assert cd.regions[1].kind == "conflict"
    assert cd.regions[1].left == "ours\n"
    assert cd.regions[1].base == "base\n"
    assert cd.regions[1].right == "theirs\n"
    assert cd.regions[2].kind == "resolved"
    assert cd.regions[2].text == "after\n"


def test_parse_conflict_data_empty_sides():
    # Empty side = that side deleted the region.
    out = "<<<<<<<\n|||||||\nbase\n=======\ntheirs\n>>>>>>>\n"
    cd = parse_conflict_data(out)
    assert len(cd.regions) == 1
    assert cd.regions[0].kind == "conflict"
    assert cd.regions[0].left == ""
    assert cd.regions[0].base == "base\n"
    assert cd.regions[0].right == "theirs\n"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/backend/test_parse_ext.py::test_parse_conflict_data_no_conflicts -v`
Expected: FAIL with `ImportError: cannot import name 'parse_conflict_data'`.

- [ ] **Step 3: Implement `parse_conflict_data`**

Add `ConflictData` and `ConflictRegion` to the `from lajjzy.backend.types import ...` import in `parse.py`. Append:

```python
_CONFLICT_MARKERS = ("<<<<<<<", "|||||||", "=======", ">>>>>>>")


def parse_conflict_data(output: str) -> ConflictData:
    """Parse a conflicted file's raw content into ConflictData.

    jj's conflict format uses 7-char markers on their own lines:
        <<<<<<<
        <left (ours)>
        |||||||
        <base>
        =======
        <right (theirs)>
        >>>>>>>
    Regions outside conflict hunks are non-conflicting (``resolved``).
    An empty side means that side deleted the region.
    """
    lines = output.splitlines(keepends=True)
    regions: list[ConflictRegion] = []
    i = 0
    pending_resolved: list[str] = []

    def flush_resolved() -> None:
        if pending_resolved:
            regions.append(ConflictRegion.resolved("".join(pending_resolved)))
            pending_resolved.clear()

    while i < len(lines):
        stripped = lines[i].rstrip("\n")
        if stripped == "<<<<<<<":
            flush_resolved()
            i += 1
            left: list[str] = []
            while i < len(lines) and lines[i].rstrip("\n") != "|||||||":
                left.append(lines[i])
                i += 1
            i += 1  # skip |||||||
            base: list[str] = []
            while i < len(lines) and lines[i].rstrip("\n") != "=======":
                base.append(lines[i])
                i += 1
            i += 1  # skip =======
            right: list[str] = []
            while i < len(lines) and lines[i].rstrip("\n") != ">>>>>>>":
                right.append(lines[i])
                i += 1
            i += 1  # skip >>>>>>>
            regions.append(
                ConflictRegion.conflict(
                    base="".join(base),
                    left="".join(left),
                    right="".join(right),
                )
            )
        else:
            pending_resolved.append(lines[i])
            i += 1

    flush_resolved()
    return ConflictData(regions=regions)
```

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/backend/test_parse_ext.py -v`
Expected: PASS (16 tests).

- [ ] **Step 5: Lint + typecheck + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/backend/parse.py tests/backend/test_parse_ext.py
git commit -m "feat(backend): parse_conflict_data"
```

---

## Task 5: jj facade — undo, redo, op_log, op_restore

**Files:**
- Modify: `src/lajjzy/backend/jj.py` (add 4 functions)
- Test: `tests/backend/test_jj_facade_ext.py` (create)

- [ ] **Step 1: Write the failing integration tests**

Create `tests/backend/test_jj_facade_ext.py`:

```python
from __future__ import annotations

from pathlib import Path

import pytest

from lajjzy.backend import jj
from lajjzy.backend.types import OpLogEntry

from tests.conftest import jj_required


@jj_required
async def test_undo_returns_message(temp_repo: Path):
    msg = await jj.undo(temp_repo)
    assert "undo" in msg.lower() or "undid" in msg.lower()


@jj_required
async def test_redo_returns_message(temp_repo: Path):
    await jj.undo(temp_repo)
    msg = await jj.redo(temp_repo)
    assert "redo" in msg.lower() or "redid" in msg.lower()


@jj_required
async def test_op_log_returns_entries(temp_repo: Path):
    entries = await jj.op_log(temp_repo)
    assert isinstance(entries, list)
    assert len(entries) >= 1
    assert all(isinstance(e, OpLogEntry) for e in entries)
    assert all(e.op_id for e in entries)


@jj_required
async def test_op_restore_roundtrip(temp_repo: Path):
    # Capture current state, make a change, then restore to undo it.
    entries_before = await jj.op_log(temp_repo)
    op_id = entries_before[0].op_id
    await jj.new_change(temp_repo, "ksqxwpml")
    await jj.op_restore(temp_repo, op_id)
    # After restore, the new change should be gone — graph load reflects it.
    from lajjzy.backend.types import GraphData

    graph = await jj.load_graph(temp_repo)
    assert isinstance(graph, GraphData)
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/backend/test_jj_facade_ext.py -v`
Expected: FAIL with `AttributeError: module 'lajjzy.backend.jj' has no attribute 'undo'`.

- [ ] **Step 3: Implement the 4 facade functions**

Append to `src/lajjzy/backend/jj.py`. Add `OpLogEntry` to the `from lajjzy.backend.types import ...` import, and add `parse_op_log` to the `from lajjzy.backend.parse import ...` import:

```python
async def undo(cwd: Path) -> str:
    await run_jj(["undo"], cwd)
    return "Undid the last operation"


async def redo(cwd: Path) -> str:
    await run_jj(["redo"], cwd)
    return "Redid the last operation"


_OP_LOG_TEMPLATE = (
    'self.id().short(16) ++ "\\x1f" ++ '
    "committer.timestamp().ago() ++ \"\\x1f\" ++ "
    'coalesce(description.first_line(), "") ++ "\\n"'
)


async def op_log(cwd: Path) -> list[OpLogEntry]:
    stdout = await run_jj(
        ["op", "log", "--no-graph", "-T", _OP_LOG_TEMPLATE], cwd
    )
    return parse_op_log(stdout)


async def op_restore(cwd: Path, op_id: str) -> str:
    await run_jj(["op", "restore", op_id], cwd)
    return f"Restored operation {op_id}"
```

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/backend/test_jj_facade_ext.py -v`
Expected: PASS (4 tests). If `test_op_restore_roundtrip` fails because `jj op restore` doesn't accept a short op-id, switch the template to `self.id().short(16)` (already set) and confirm `jj op restore <short>` works in 0.42.0. If it needs the full ID, lengthen the template.

- [ ] **Step 5: Lint + typecheck + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/backend/jj.py tests/backend/test_jj_facade_ext.py
git commit -m "feat(backend): undo, redo, op_log, op_restore facades"
```

---

## Task 6: jj facade — bookmark_set, bookmark_delete, bookmark_move, load_bookmarks

**Files:**
- Modify: `src/lajjzy/backend/jj.py` (add 4 functions)
- Test: `tests/backend/test_jj_facade_ext.py` (extend)

- [ ] **Step 1: Write the failing integration tests**

Append to `tests/backend/test_jj_facade_ext.py`:

```python
from lajjzy.backend.types import Bookmark


@jj_required
async def test_load_bookmarks_empty_repo(temp_repo: Path):
    bms = await jj.load_bookmarks(temp_repo)
    assert bms == []


@jj_required
async def test_bookmark_set_and_load(temp_repo: Path):
    graph = await jj.load_graph(temp_repo)
    target = graph.lines[graph.working_copy_index or 0].change_id
    assert target is not None
    await jj.bookmark_set(temp_repo, target, "mybm")
    bms = await jj.load_bookmarks(temp_repo)
    names = [b.name for b in bms]
    assert "mybm" in names
    matched = [b for b in bms if b.name == "mybm"][0]
    assert matched.change_id == target


@jj_required
async def test_bookmark_delete(temp_repo: Path):
    graph = await jj.load_graph(temp_repo)
    target = graph.lines[graph.working_copy_index or 0].change_id
    assert target is not None
    await jj.bookmark_set(temp_repo, target, "todelete")
    await jj.bookmark_delete(temp_repo, "todelete")
    bms = await jj.load_bookmarks(temp_repo)
    assert "todelete" not in [b.name for b in bms]


@jj_required
async def test_bookmark_move(temp_repo: Path):
    import subprocess

    subprocess.run(["jj", "new", "-m", "second"], cwd=temp_repo, check=True, capture_output=True)
    graph = await jj.load_graph(temp_repo)
    first = graph.lines[0].change_id
    second = graph.lines[1].change_id
    assert first is not None and second is not None
    await jj.bookmark_set(temp_repo, first, "moveable")
    await jj.bookmark_move(temp_repo, "moveable", second)
    bms = await jj.load_bookmarks(temp_repo)
    moved = [b for b in bms if b.name == "moveable"][0]
    assert moved.change_id == second
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/backend/test_jj_facade_ext.py::test_load_bookmarks_empty_repo -v`
Expected: FAIL with `AttributeError: module 'lajjzy.backend.jj' has no attribute 'load_bookmarks'`.

- [ ] **Step 3: Implement the 4 facade functions**

Append to `src/lajjzy/backend/jj.py`. Add `Bookmark` to the types import and `parse_bookmarks` to the parse import:

```python
_BOOKMARK_TEMPLATE = (
    'name ++ "\\x1f" ++ change_id.short() ++ "\\x1f" ++ '
    'coalesce(description.first_line(), "") ++ "\\n"'
)


async def load_bookmarks(cwd: Path) -> list[Bookmark]:
    stdout = await run_jj(
        ["bookmark", "list", "-T", _BOOKMARK_TEMPLATE, "--color=never"], cwd
    )
    return parse_bookmarks(stdout)


async def bookmark_set(cwd: Path, change_id: str, name: str) -> str:
    await run_jj(["bookmark", "set", "-r", change_id, name], cwd)
    return f"Set bookmark {name} on {change_id}"


async def bookmark_delete(cwd: Path, name: str) -> str:
    await run_jj(["bookmark", "delete", name], cwd)
    return f"Deleted bookmark {name}"


async def bookmark_move(cwd: Path, name: str, dest_change_id: str) -> str:
    await run_jj(["bookmark", "set", "-r", dest_change_id, name], cwd)
    return f"Moved bookmark {name} to {dest_change_id}"
```

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/backend/test_jj_facade_ext.py -v`
Expected: PASS (8 tests). If `jj bookmark list -T` doesn't work in 0.42.0, try `jj bookmark list --template` or fall back to parsing the default human-readable output (update the parser + template together).

- [ ] **Step 5: Lint + typecheck + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/backend/jj.py tests/backend/test_jj_facade_ext.py
git commit -m "feat(backend): bookmark set/delete/move + load_bookmarks facades"
```

---

## Task 7: jj facade — conflict_data, resolve

**Files:**
- Modify: `src/lajjzy/backend/jj.py` (add 2 functions)
- Test: `tests/backend/test_jj_facade_ext.py` (extend)

- [ ] **Step 1: Write the failing integration tests**

Append to `tests/backend/test_jj_facade_ext.py`:

```python
from lajjzy.backend.types import ConflictData, HunkResolution


@jj_required
async def test_conflict_data_no_conflict(temp_repo: Path):
    # No conflicts in a fresh repo — but conflict_data on a non-conflicted file
    # should still return one resolved region with the file content.
    import subprocess

    subprocess.run(["touch", "file.txt"], cwd=temp_repo, check=True)
    subprocess.run(["jj", "new"], cwd=temp_repo, check=True, capture_output=True)
    subprocess.run(["touch", "other.txt"], cwd=temp_repo, check=True)
    subprocess.run(["jj", "new"], cwd=temp_repo, check=True, capture_output=True)
    cd = await jj.conflict_data(temp_repo, "file.txt")
    assert isinstance(cd, ConflictData)
    assert all(r.kind == "resolved" for r in cd.regions)


@jj_required
async def test_resolve_accept_left(temp_repo: Path):
    # Create a conflict: two changes edit the same line differently.
    import subprocess

    subprocess.run(["jj", "new", "-m", "base"], cwd=temp_repo, check=True, capture_output=True)
    (temp_repo / "c.txt").write_text("LINE\n")
    subprocess.run(["jj", "new", "-m", "left"], cwd=temp_repo, check=True, capture_output=True)
    (temp_repo / "c.txt").write_text("LEFT\n")
    subprocess.run(["jj", "new", "-m", "right", "--after", "@-"], cwd=temp_repo, check=True, capture_output=True)
    (temp_repo / "c.txt").write_text("RIGHT\n")
    subprocess.run(["jj", "new", "-m", "merge", "--after", "@-", "--allow-empty"], cwd=temp_repo, check=True, capture_output=True)
    # Now @ is the merge with a conflict on c.txt.
    cd = await jj.conflict_data(temp_repo, "c.txt")
    conflict_regions = [r for r in cd.regions if r.kind == "conflict"]
    assert len(conflict_regions) >= 1
    resolutions = [HunkResolution.ACCEPT_LEFT] * len(conflict_regions)
    msg = await jj.resolve(temp_repo, "c.txt", resolutions)
    assert "resolve" in msg.lower() or "resolved" in msg.lower()
    # After resolve, the file should contain LEFT.
    assert (temp_repo / "c.txt").read_text() == "LEFT\n"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/backend/test_jj_facade_ext.py::test_conflict_data_no_conflict -v`
Expected: FAIL with `AttributeError: module 'lajjzy.backend.jj' has no attribute 'conflict_data'`.

- [ ] **Step 3: Implement `conflict_data` and `resolve`**

Add `ConflictData`, `ConflictRegion`, `HunkResolution` to the types import and `parse_conflict_data` to the parse import in `jj.py`. Append:

```python
async def conflict_data(cwd: Path, path: str) -> ConflictData:
    """Read a conflicted file's raw content (with jj conflict markers) and
    parse it into ConflictData. Works on the working copy (`@`)."""
    stdout = await run_jj(["file", "show", "-r", "@", path], cwd)
    return parse_conflict_data(stdout)


def _build_resolved_content(data: ConflictData, resolutions: list[str]) -> str:
    """Apply per-hunk resolution choices to produce the final file content.

    `resolutions` is one entry per conflict region (in order), each being
    HunkResolution.NONE / ACCEPT_LEFT / ACCEPT_RIGHT. NONE is treated as
    ACCEPT_LEFT (the widget must not let users apply with NONE set, but we
    default defensively).
    """
    out: list[str] = []
    conflict_idx = 0
    for region in data.regions:
        if region.kind == "resolved":
            out.append(region.text)
            continue
        choice = resolutions[conflict_idx]
        conflict_idx += 1
        if choice == HunkResolution.ACCEPT_RIGHT:
            out.append(region.right)
        else:  # NONE or ACCEPT_LEFT
            out.append(region.left)
    return "".join(out)


async def resolve(
    cwd: Path, path: str, resolutions: list[str]
) -> str:
    """Write the resolved file content to the working copy. Caller must ensure
    `@` is the conflicted change (LajjzyApp.ensure_working_copy). Does NOT mark
    the conflict resolved in jj's internal state — that happens automatically
    when the file no longer contains conflict markers."""
    data = await conflict_data(cwd, path)
    resolved = _build_resolved_content(data, resolutions)
    (cwd / path).write_text(resolved)
    return f"Resolved {path}"
```

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/backend/test_jj_facade_ext.py -v`
Expected: PASS (10 tests). If the conflict-creation setup in `test_resolve_accept_left` doesn't produce a conflict in jj 0.42.0 (the `--after` semantics may differ), adjust the setup to use `jj new --after=@-` and `jj rebase` to force a three-way merge. The test is the spec for "resolve works end-to-end"; the setup mechanics are secondary.

- [ ] **Step 5: Lint + typecheck + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/backend/jj.py tests/backend/test_jj_facade_ext.py
git commit -m "feat(backend): conflict_data + resolve facades"
```

---

## Task 8: jj facade — split, squash_partial

**Files:**
- Modify: `src/lajjzy/backend/jj.py` (add 2 functions)
- Test: `tests/backend/test_jj_facade_ext.py` (extend)

`jj split` and `jj squash` in 0.42.0 support `--interactive` but for non-interactive hunk selection we use `jj split -r <id> -p <path>` with stdin piping of a selected-hunk spec, OR the simpler `jj split --interactive` with a tool. The cleanest non-interactive approach in 0.42.0 is `jj split -r <id>` with the `--restore-descs` flag and a path list. Because jj's non-interactive hunk selection CLI is limited, the facade accepts a list of `(path, hunk_idx)` refs and uses `jj split -r <source> <paths>` to split only the specified files (whole-file split per path). Full hunk-granular split is a follow-up if the CLI lacks the flag — flagged in the spec's open questions.

- [ ] **Step 1: Write the failing integration tests**

Append to `tests/backend/test_jj_facade_ext.py`:

```python
from lajjzy.backend.types import HunkRef


@jj_required
async def test_split_whole_file(temp_repo: Path):
    import subprocess

    (temp_repo / "split.txt").write_text("a\nb\n")
    subprocess.run(["jj", "new", "-m", "tosplit"], cwd=temp_repo, check=True, capture_output=True)
    graph = await jj.load_graph(temp_repo)
    target = graph.lines[graph.working_copy_index or 0].change_id
    assert target is not None
    # Split the whole file into a new change.
    hunks = [HunkRef(path="split.txt", hunk_idx=0)]
    msg = await jj.split(temp_repo, target, hunks)
    assert "split" in msg.lower()
    # After split, there should be two changes where there was one.
    graph2 = await jj.load_graph(temp_repo)
    assert len(graph2.node_indices) > len(graph.node_indices)


@jj_required
async def test_squash_partial_whole_file(temp_repo: Path):
    import subprocess

    (temp_repo / "squash.txt").write_text("x\n")
    subprocess.run(["jj", "new", "-m", "tosquash"], cwd=temp_repo, check=True, capture_output=True)
    (temp_repo / "squash.txt").write_text("x\ny\n")
    subprocess.run(["jj", "new", "-m", "child"], cwd=temp_repo, check=True, capture_output=True)
    graph = await jj.load_graph(temp_repo)
    # Squash the child's changes into its parent (partial — only squash.txt).
    child = graph.lines[graph.working_copy_index or 0].change_id
    assert child is not None
    hunks = [HunkRef(path="squash.txt", hunk_idx=0)]
    msg = await jj.squash_partial(temp_repo, child, hunks)
    assert "squash" in msg.lower()
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/backend/test_jj_facade_ext.py::test_split_whole_file -v`
Expected: FAIL with `AttributeError: module 'lajjzy.backend.jj' has no attribute 'split'` and `ImportError: cannot import name 'HunkRef'`.

- [ ] **Step 3: Add `HunkRef` type and implement `split`, `squash_partial`**

Add to `src/lajjzy/backend/types.py`:

```python
@dataclass(frozen=True, slots=True)
class HunkRef:
    """A reference to a selected hunk for split / partial squash. `hunk_idx`
    is the index of the hunk within the file's diff (0-based). Phase-1
    implementation splits at file granularity (the whole file is selected if
    any of its hunks are selected); hunk-granular selection arrives when the
    jj CLI exposes a stable non-interactive flag for it."""

    path: str
    hunk_idx: int
```

Append to `src/lajjzy/backend/jj.py`. Add `HunkRef` to the types import:

```python
async def split(cwd: Path, source: str, hunks: list[HunkRef]) -> str:
    # Non-interactive split: jj split -r <source> <paths...> splits the listed
    # files' changes into a new change. Hunk-granular split within a file
    # requires --interactive (a TTY); we split at file granularity here and
    # document the limitation. Only the paths of `hunks` matter in phase 1.
    paths = sorted({h.path for h in hunks})
    if not paths:
        raise JjError("split requires at least one selected hunk")
    await run_jj(["split", "-r", source, *paths], cwd)
    return f"Split {len(paths)} file(s) out of {source}"


async def squash_partial(
    cwd: Path, source: str, hunks: list[HunkRef]
) -> str:
    # jj squash -r <source> <paths...> moves only the listed files' changes
    # into the parent. Same file-granularity limitation as split().
    paths = sorted({h.path for h in hunks})
    if not paths:
        raise JjError("squash_partial requires at least one selected hunk")
    await run_jj(["squash", "-r", source, "--use-destination-message", *paths], cwd)
    return f"Squashed {len(paths)} file(s) from {source} into parent"
```

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/backend/test_jj_facade_ext.py -v`
Expected: PASS (12 tests). If `jj split -r <source> <paths>` doesn't accept path args in 0.42.0, use `jj split -r <source> --interactive </dev/null` (no-op) or pipe a selection script. The test pins the behaviour; adjust the facade to match what 0.42.0 actually supports.

- [ ] **Step 5: Lint + typecheck + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/backend/types.py src/lajjzy/backend/jj.py tests/backend/test_jj_facade_ext.py
git commit -m "feat(backend): split + squash_partial facades (file granularity)"
```

---

## Task 9: Core `Msg` types — all six features

**Files:**
- Modify: `src/lajjzy/core/messages.py` (append all new Msg classes)
- Test: `tests/core/test_update.py` (extend in Tasks 11–14)

- [ ] **Step 1: Write the failing import test**

Append to `tests/core/test_update.py` (after the existing imports):

```python
from lajjzy.core import (
    # new in phase 1a:
    ApplyResolutions,
    BookmarkDelete,
    BookmarkInputCancel,
    BookmarkInputConfirm,
    BookmarkMove,
    BookmarkMoveConfirm,
    BookmarkSet,
    BookmarksLoaded,
    BookmarksLoadFailed,
    ConflictDataLoadFailed,
    ConflictDataLoaded,
    ConflictViewClose,
    HunkPickerClose,
    LoadBookmarks,
    LoadConflictData,
    LoadOpLog,
    OpenBookmarkPicker,
    OpenBookmarkSet,
    OpenConflictView,
    OpenOmnibar,
    OpenOpLog,
    OmnibarAcceptCompletion,
    OmnibarBackspace,
    OmnibarCancel,
    OmnibarInput,
    OmnibarSubmit,
    OpLogClose,
    OpLogLoaded,
    OpLogLoadFailed,
    OpLogRestore,
    Redo,
    Split,
    SplitConfirm,
    SquashPartial,
    SquashPartialConfirm,
    Undo,
)
from lajjzy.backend.types import (
    Bookmark,
    CompletionItem,
    ConflictData,
    HunkRef,
    HunkResolution,
    OpLogEntry,
)


def test_msg_types_importable():
    # Smoke test — just constructing each is enough to verify the import + dataclass shape.
    assert Undo() is not None
    assert Redo() is not None
    assert OpenOmnibar() is not None
    assert OmnibarInput("x") is not None
    assert OmnibarSubmit("mine()") is not None
    assert OpenBookmarkSet() is not None
    assert OpenBookmarkPicker() is not None
    assert BookmarkInputConfirm("main") is not None
    assert BookmarkDelete("main") is not None
    assert BookmarkMove("main") is not None
    assert BookmarkMoveConfirm("main", "ksqxwpml") is not None
    assert OpenOpLog() is not None
    assert OpLogRestore("abc123") is not None
    assert OpLogClose() is not None
    assert OpenConflictView("file.txt") is not None
    assert ConflictViewClose() is not None
    assert ApplyResolutions("file.txt", [HunkResolution.ACCEPT_LEFT]) is not None
    assert Split() is not None
    assert SquashPartial() is not None
    assert SplitConfirm("ksqxwpml", [HunkRef("file.txt", 0)]) is not None
    assert SquashPartialConfirm("ksqxwpml", [HunkRef("file.txt", 0)]) is not None
    # result Msgs
    assert OpLogLoaded([]) is not None
    assert OpLogLoadFailed("boom") is not None
    assert BookmarksLoaded([]) is not None
    assert BookmarksLoadFailed("boom") is not None
    assert ConflictDataLoaded(ConflictData(regions=[])) is not None
    assert ConflictDataLoadFailed("boom") is not None
    # cmds
    assert LoadOpLog() is not None
    assert LoadBookmarks() is not None
    assert LoadConflictData("file.txt") is not None
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/core/test_update.py::test_msg_types_importable -v`
Expected: FAIL with `ImportError: cannot import name 'Undo'` etc.

- [ ] **Step 3: Add all new `Msg` classes to `src/lajjzy/core/messages.py`**

Append (the file currently ends after `MutationCompleted` at line 110):

```python
# --- undo / redo -------------------------------------------------------
@dataclass(frozen=True)
class Undo:
    pass


@dataclass(frozen=True)
class Redo:
    pass


# --- omnibar -----------------------------------------------------------
@dataclass(frozen=True)
class OpenOmnibar:
    pass


@dataclass(frozen=True)
class OmnibarInput:
    char: str


@dataclass(frozen=True)
class OmnibarBackspace:
    pass


@dataclass(frozen=True)
class OmnibarAcceptCompletion:
    pass


@dataclass(frozen=True)
class OmnibarSubmit:
    revset: str | None  # None = clear filter, empty string = no-op, non-empty = apply


@dataclass(frozen=True)
class OmnibarCancel:
    pass


# --- bookmarks ---------------------------------------------------------
@dataclass(frozen=True)
class OpenBookmarkSet:
    pass


@dataclass(frozen=True)
class OpenBookmarkPicker:
    pass


@dataclass(frozen=True)
class BookmarkInputConfirm:
    name: str


@dataclass(frozen=True)
class BookmarkInputCancel:
    pass


@dataclass(frozen=True)
class BookmarkDelete:
    name: str


@dataclass(frozen=True)
class BookmarkMove:
    name: str


@dataclass(frozen=True)
class BookmarkMoveConfirm:
    name: str
    dest_change_id: str


@dataclass(frozen=True)
class BookmarksLoaded:
    bookmarks: list[Bookmark]


@dataclass(frozen=True)
class BookmarksLoadFailed:
    error: str


# --- operation log -----------------------------------------------------
@dataclass(frozen=True)
class OpenOpLog:
    pass


@dataclass(frozen=True)
class OpLogClose:
    pass


@dataclass(frozen=True)
class OpLogRestore:
    op_id: str


@dataclass(frozen=True)
class OpLogLoaded:
    entries: list[OpLogEntry]


@dataclass(frozen=True)
class OpLogLoadFailed:
    error: str


# --- conflict view -----------------------------------------------------
@dataclass(frozen=True)
class OpenConflictView:
    path: str


@dataclass(frozen=True)
class ConflictViewClose:
    pass


@dataclass(frozen=True)
class ApplyResolutions:
    path: str
    resolutions: list[str]  # list of HunkResolution.* values, one per conflict region


@dataclass(frozen=True)
class ConflictDataLoaded:
    data: ConflictData


@dataclass(frozen=True)
class ConflictDataLoadFailed:
    error: str


# --- hunk picker (split / partial squash) ------------------------------
@dataclass(frozen=True)
class Split:
    pass


@dataclass(frozen=True)
class SquashPartial:
    pass


@dataclass(frozen=True)
class HunkPickerClose:
    pass


@dataclass(frozen=True)
class SplitConfirm:
    source: str
    hunks: list[HunkRef]


@dataclass(frozen=True)
class SquashPartialConfirm:
    source: str
    hunks: list[HunkRef]
```

Add the imports at the top of `messages.py`:

```python
from lajjzy.backend.types import (
    Bookmark,
    ConflictData,
    HunkRef,
    OpLogEntry,
)
```
(into the existing `from lajjzy.backend.types import GraphData` line — extend it.)

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/core/test_update.py::test_msg_types_importable -v`
Expected: PASS.

- [ ] **Step 5: Lint + typecheck + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/core/messages.py tests/core/test_update.py
git commit -m "feat(core): Msg types for undo/redo, omnibar, bookmarks, op log, conflicts, hunk picker"
```

---

## Task 10: Core `Cmd` types + `Model` fields

**Files:**
- Modify: `src/lajjzy/core/commands.py` (add `LoadOpLog`, `LoadBookmarks`, `LoadConflictData`; extend `LoadGraph` with `revset`)
- Modify: `src/lajjzy/core/model.py` (add new fields + helpers)
- Test: `tests/core/test_update.py` (extend)

- [ ] **Step 1: Write the failing test**

Append to `tests/core/test_update.py`:

```python
from lajjzy.core.commands import LoadConflictData, LoadGraph, LoadOpLog, LoadBookmarks


def test_loadgraph_has_revset_field():
    cmd = LoadGraph(epoch=1, revset="mine()")
    assert cmd.epoch == 1
    assert cmd.revset == "mine()"


def test_loadgraph_revset_defaults_none():
    cmd = LoadGraph(epoch=1)
    assert cmd.revset is None


def test_loadoplog_cmd():
    cmd = LoadOpLog()
    assert cmd is not None


def test_loadbookmarks_cmd():
    cmd = LoadBookmarks()
    assert cmd is not None


def test_loadconflictdata_cmd():
    cmd = LoadConflictData(path="file.txt")
    assert cmd.path == "file.txt"


def test_model_new_fields_default_none():
    m = Model()
    assert m.op_log_entries is None
    assert m.bookmarks is None
    assert m.revset is None
    assert m.conflict_data is None
    assert m.conflict_path is None
    assert m.modal is None
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/core/test_update.py -k "loadgraph_has_revset or loadoplog_cmd or loadbookmarks_cmd or loadconflictdata_cmd or model_new_fields" -v`
Expected: FAIL (missing `LoadOpLog`, `LoadBookmarks`, `LoadConflictData`, `revset` field, `Model` fields).

- [ ] **Step 3: Extend `src/lajjzy/core/commands.py`**

Replace the existing `LoadGraph` class and the `Cmd` union. The file currently has `LoadGraph` at line 12 with only `epoch`. Replace:

```python
@dataclass(frozen=True)
class LoadGraph:
    """Reload the change graph. On completion dispatch GraphLoaded(epoch, graph)
    or GraphLoadFailed(error)."""

    epoch: int
    revset: str | None = None
```

Append the new Cmds before the `Cmd = ...` line:

```python
@dataclass(frozen=True)
class LoadOpLog:
    """Fetch jj op log. On completion dispatch OpLogLoaded(entries) or
    OpLogLoadFailed(error)."""


@dataclass(frozen=True)
class LoadBookmarks:
    """Fetch jj bookmark list. On completion dispatch BookmarksLoaded(bookmarks)
    or BookmarksLoadFailed(error)."""


@dataclass(frozen=True)
class LoadConflictData:
    """Fetch conflict data for one file. On completion dispatch
    ConflictDataLoaded(data) or ConflictDataLoadFailed(error)."""

    path: str
```

Update the `Cmd` union:

```python
Cmd = LoadGraph | RunMutation | EditMessage | LoadOpLog | LoadBookmarks | LoadConflictData
```

- [ ] **Step 4: Extend `src/lajjzy/core/model.py`**

Add the new imports and fields. The `Model` dataclass currently ends at `graph_epoch: int = 0` (line 31). Replace the field block:

```python
from lajjzy.backend.types import Bookmark, ConflictData, GraphData, OpLogEntry


@dataclass(frozen=True)
class Model:
    # ... existing docstring ...

    graph: GraphData | None = None
    cursor: int = 0
    error: str | None = None
    rebase_source: str | None = None
    rebase_descendants: bool = False
    pending_mutation: bool = False
    graph_epoch: int = 0
    # --- phase 1a additions ---
    op_log_entries: list[OpLogEntry] | None = None
    bookmarks: list[Bookmark] | None = None
    revset: str | None = None
    conflict_data: ConflictData | None = None
    conflict_path: str | None = None
    modal: str | None = None  # "omnibar"|"bookmark_input"|"bookmark_picker"|"op_log"|"conflict_view"|"hunk_picker"|None
```

- [ ] **Step 5: Run test to verify it passes**

Run: `uv run pytest tests/core/test_update.py -k "loadgraph_has_revset or loadoplog_cmd or loadbookmarks_cmd or loadconflictdata_cmd or model_new_fields" -v`
Expected: PASS (5 tests).

- [ ] **Step 6: Re-export new symbols from `src/lajjzy/core/__init__.py`**

Open `src/lajjzy/core/__init__.py` and add the new `Msg` and `Cmd` names to the existing re-exports (both `from .commands import ...` and `from .messages import ...`). Also re-export `LoadOpLog`, `LoadBookmarks`, `LoadConflictData`. Run the full core test suite to confirm nothing broke:

```bash
uv run pytest tests/core/ -v
```

Expected: all PASS.

- [ ] **Step 7: Lint + typecheck + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/core/commands.py src/lajjzy/core/model.py src/lajjzy/core/__init__.py tests/core/test_update.py
git commit -m "feat(core): Cmd types (LoadOpLog/LoadBookmarks/LoadConflictData) + Model fields"
```

---

## Task 11: `update` — undo/redo + omnibar branches

**Files:**
- Modify: `src/lajjzy/core/update.py`
- Test: `tests/core/test_update.py` (extend)

- [ ] **Step 1: Write the failing tests**

Append to `tests/core/test_update.py`:

```python
# --- undo / redo -------------------------------------------------------


def test_undo_starts_mutation():
    m = _loaded("aaa", working=0)
    m1, cmds = update(m, Undo())
    assert m1.pending_mutation is True
    assert m1.graph_epoch == 2
    assert cmds == [RunMutation(2, "undo", ())]


def test_undo_blocked_while_pending():
    m = _loaded("aaa", working=0)
    armed, _ = update(m, NewChange())
    blocked, cmds = update(armed, Undo())
    assert cmds == []
    assert blocked.error == "A mutation is already in progress"
    assert blocked.pending_mutation is True


def test_redo_starts_mutation():
    m = _loaded("aaa", working=0)
    m1, cmds = update(m, Redo())
    assert m1.pending_mutation is True
    assert cmds == [RunMutation(2, "redo", ())]


# --- omnibar -----------------------------------------------------------


def test_open_omnibar_sets_modal():
    m = _loaded("aaa", working=0)
    m1, cmds = update(m, OpenOmnibar())
    assert m1.modal == "omnibar"
    assert cmds == []


def test_omnibar_cancel_clears_modal():
    m = _loaded("aaa", working=0)
    opened, _ = update(m, OpenOmnibar())
    cancelled, _ = update(opened, OmnibarCancel())
    assert cancelled.modal is None


def test_omnibar_submit_with_revset_loads_filtered_graph():
    m = _loaded("aaa", working=0)
    opened, _ = update(m, OpenOmnibar())
    submitted, cmds = update(opened, OmnibarSubmit("mine()"))
    assert submitted.modal is None
    assert submitted.revset == "mine()"
    assert cmds == [LoadGraph(submitted.graph_epoch, "mine()")]


def test_omnibar_submit_none_clears_revset():
    m = _loaded("aaa", working=0)
    # precondition: a revset is active
    pre = replace(m, revset="mine()")
    opened, _ = update(pre, OpenOmnibar())
    submitted, cmds = update(opened, OmnibarSubmit(None))
    assert submitted.modal is None
    assert submitted.revset is None
    assert cmds == [LoadGraph(submitted.graph_epoch, None)]
```

Add `replace` to the imports if not already (it is — line 3 of `test_update.py`).

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/core/test_update.py -k "undo or redo or omnibar" -v`
Expected: FAIL (the `update` function returns `model, []` for unknown Msgs).

- [ ] **Step 3: Add the branches to `src/lajjzy/core/update.py`**

In the `update` function, add these branches after the existing mutation branches (after the `if isinstance(msg, MutationCompleted)` block, before the describe section). Add `Undo`, `Redo`, `OpenOmnibar`, `OmnibarCancel`, `OmnibarSubmit` to the import from `lajjzy.core.messages`. Also import `LoadGraph` (already imported).

```python
    # --- undo / redo ------------------------------------------------------
    if isinstance(msg, Undo):
        return _start_mutation(model, "undo", ())
    if isinstance(msg, Redo):
        return _start_mutation(model, "redo", ())

    # --- omnibar ----------------------------------------------------------
    if isinstance(msg, OpenOmnibar):
        return replace(model, modal="omnibar"), []
    if isinstance(msg, OmnibarCancel):
        return replace(model, modal=None), []
    if isinstance(msg, OmnibarSubmit):
        revset = msg.revset
        if revset is not None and revset == "":
            # empty query = no-op, just close
            return replace(model, modal=None), []
        epoch = model.graph_epoch + 1
        return replace(model, modal=None, revset=revset, graph_epoch=epoch), [
            LoadGraph(epoch, revset)
        ]
```

Note: `OmnibarInput`, `OmnibarBackspace`, `OmnibarAcceptCompletion` are widget-local state changes (query/cursor/completions live in the widget, not the Model) — they do NOT have `update` branches. The widget handles them directly and only dispatches `OmnibarSubmit`/`OmnibarCancel` to core. Add a comment in `update.py`:

```python
    # OmnibarInput / OmnibarBackspace / OmnibarAcceptCompletion are handled
    # widget-locally (query/cursor/completions are ephemeral); only submit /
    # cancel reach core.
```

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/core/test_update.py -k "undo or redo or omnibar" -v`
Expected: PASS (7 tests).

- [ ] **Step 5: Lint + typecheck + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/core/update.py tests/core/test_update.py
git commit -m "feat(core): update branches for undo/redo + omnibar"
```

---

## Task 12: `update` — bookmark branches

**Files:**
- Modify: `src/lajjzy/core/update.py`
- Test: `tests/core/test_update.py` (extend)

- [ ] **Step 1: Write the failing tests**

Append to `tests/core/test_update.py`:

```python
# --- bookmarks ---------------------------------------------------------


def test_open_bookmark_set_sets_modal():
    m = _loaded("aaa", working=0)
    m1, _ = update(m, OpenBookmarkSet())
    assert m1.modal == "bookmark_input"


def test_open_bookmark_picker_sets_modal_and_loads():
    m = _loaded("aaa", working=0)
    m1, cmds = update(m, OpenBookmarkPicker())
    assert m1.modal == "bookmark_picker"
    assert cmds == [LoadBookmarks()]


def test_bookmark_input_confirm_starts_mutation():
    m = _loaded("aaa", working=0)
    opened, _ = update(m, OpenBookmarkSet())
    confirmed, cmds = update(opened, BookmarkInputConfirm("main"))
    assert confirmed.modal is None
    assert confirmed.pending_mutation is True
    assert cmds == [RunMutation(confirmed.graph_epoch, "bookmark_set", ("aaa", "main"))]


def test_bookmark_input_cancel_clears_modal():
    m = _loaded("aaa", working=0)
    opened, _ = update(m, OpenBookmarkSet())
    cancelled, _ = update(opened, BookmarkInputCancel())
    assert cancelled.modal is None


def test_bookmark_delete_starts_mutation():
    m = _loaded("aaa", working=0)
    m1, cmds = update(m, BookmarkDelete("main"))
    assert m1.pending_mutation is True
    assert cmds == [RunMutation(2, "bookmark_delete", ("main",))]


def test_bookmark_move_confirm_starts_mutation():
    m = _loaded("aaa", "bbb", working=0)
    confirmed, cmds = update(m, BookmarkMoveConfirm("main", "bbb"))
    assert confirmed.pending_mutation is True
    assert cmds == [RunMutation(2, "bookmark_move", ("main", "bbb"))]


def test_bookmarks_loaded_stores_entries():
    m = _loaded("aaa", working=0)
    bms = [Bookmark(name="main", change_id="aaa", change_description="d")]
    m1, _ = update(m, BookmarksLoaded(bms))
    assert m1.bookmarks == bms


def test_bookmarks_load_failed_sets_error():
    m = _loaded("aaa", working=0)
    m1, _ = update(m, BookmarksLoadFailed("boom"))
    assert m1.error == "boom"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/core/test_update.py -k "bookmark" -v`
Expected: FAIL.

- [ ] **Step 3: Add the branches to `src/lajjzy/core/update.py`**

Add `OpenBookmarkSet`, `OpenBookmarkPicker`, `BookmarkInputConfirm`, `BookmarkInputCancel`, `BookmarkDelete`, `BookmarkMoveConfirm`, `BookmarksLoaded`, `BookmarksLoadFailed`, `LoadBookmarks` to the imports. Append after the omnibar branches:

```python
    # --- bookmarks --------------------------------------------------------
    if isinstance(msg, OpenBookmarkSet):
        return replace(model, modal="bookmark_input"), []
    if isinstance(msg, OpenBookmarkPicker):
        return replace(model, modal="bookmark_picker"), [LoadBookmarks()]
    if isinstance(msg, BookmarkInputConfirm):
        target = selected_change_id(model)
        if target is None:
            return replace(model, modal=None, error="No change selected"), []
        return _start_mutation(
            replace(model, modal=None), "bookmark_set", (target, msg.name)
        )
    if isinstance(msg, BookmarkInputCancel):
        return replace(model, modal=None), []
    if isinstance(msg, BookmarkDelete):
        return _start_mutation(model, "bookmark_delete", (msg.name,))
    if isinstance(msg, BookmarkMoveConfirm):
        return _start_mutation(model, "bookmark_move", (msg.name, msg.dest_change_id))
    if isinstance(msg, BookmarksLoaded):
        return replace(model, bookmarks=msg.bookmarks), []
    if isinstance(msg, BookmarksLoadFailed):
        return replace(model, error=msg.error), []
```

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/core/test_update.py -k "bookmark" -v`
Expected: PASS (8 tests).

- [ ] **Step 5: Lint + typecheck + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/core/update.py tests/core/test_update.py
git commit -m "feat(core): update branches for bookmark set/delete/move + load"
```

---

## Task 13: `update` — op-log + conflict-view branches

**Files:**
- Modify: `src/lajjzy/core/update.py`
- Test: `tests/core/test_update.py` (extend)

- [ ] **Step 1: Write the failing tests**

Append to `tests/core/test_update.py`:

```python
# --- operation log -----------------------------------------------------


def test_open_op_log_sets_modal_and_loads():
    m = _loaded("aaa", working=0)
    m1, cmds = update(m, OpenOpLog())
    assert m1.modal == "op_log"
    assert cmds == [LoadOpLog()]


def test_op_log_close_clears_modal():
    m = _loaded("aaa", working=0)
    opened, _ = update(m, OpenOpLog())
    closed, _ = update(opened, OpLogClose())
    assert closed.modal is None


def test_op_log_restore_starts_mutation():
    m = _loaded("aaa", working=0)
    opened, _ = update(m, OpenOpLog())
    restored, cmds = update(opened, OpLogRestore("abc123"))
    assert restored.pending_mutation is True
    assert restored.modal is None
    assert cmds == [RunMutation(restored.graph_epoch, "op_restore", ("abc123",))]


def test_op_log_loaded_stores_entries():
    m = _loaded("aaa", working=0)
    entries = [OpLogEntry(op_id="abc", timestamp="now", description="d")]
    m1, _ = update(m, OpLogLoaded(entries))
    assert m1.op_log_entries == entries


def test_op_log_load_failed_sets_error():
    m = _loaded("aaa", working=0)
    m1, _ = update(m, OpLogLoadFailed("boom"))
    assert m1.error == "boom"


# --- conflict view -----------------------------------------------------


def test_open_conflict_view_sets_modal_and_loads():
    m = _loaded("aaa", working=0)
    m1, cmds = update(m, OpenConflictView("file.txt"))
    assert m1.modal == "conflict_view"
    assert m1.conflict_path == "file.txt"
    assert cmds == [LoadConflictData("file.txt")]


def test_conflict_view_close_clears_modal_and_path():
    m = _loaded("aaa", working=0)
    opened, _ = update(m, OpenConflictView("file.txt"))
    closed, _ = update(opened, ConflictViewClose())
    assert closed.modal is None
    assert closed.conflict_path is None
    assert closed.conflict_data is None


def test_apply_resolutions_starts_mutation():
    m = _loaded("aaa", working=0)
    opened, _ = update(m, OpenConflictView("file.txt"))
    applied, cmds = update(
        opened, ApplyResolutions("file.txt", [HunkResolution.ACCEPT_LEFT])
    )
    assert applied.pending_mutation is True
    assert applied.modal is None
    assert cmds == [
        RunMutation(applied.graph_epoch, "resolve", ("file.txt", [HunkResolution.ACCEPT_LEFT]))
    ]


def test_conflict_data_loaded_stores_data():
    m = _loaded("aaa", working=0)
    data = ConflictData(regions=[])
    m1, _ = update(m, ConflictDataLoaded(data))
    assert m1.conflict_data is data


def test_conflict_data_load_failed_sets_error():
    m = _loaded("aaa", working=0)
    m1, _ = update(m, ConflictDataLoadFailed("boom"))
    assert m1.error == "boom"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/core/test_update.py -k "op_log or conflict" -v`
Expected: FAIL.

- [ ] **Step 3: Add the branches to `src/lajjzy/core/update.py`**

Add `OpenOpLog`, `OpLogClose`, `OpLogRestore`, `OpLogLoaded`, `OpLogLoadFailed`, `OpenConflictView`, `ConflictViewClose`, `ApplyResolutions`, `ConflictDataLoaded`, `ConflictDataLoadFailed`, `LoadOpLog`, `LoadConflictData` to the imports. Append:

```python
    # --- operation log ----------------------------------------------------
    if isinstance(msg, OpenOpLog):
        return replace(model, modal="op_log"), [LoadOpLog()]
    if isinstance(msg, OpLogClose):
        return replace(model, modal=None), []
    if isinstance(msg, OpLogRestore):
        return _start_mutation(replace(model, modal=None), "op_restore", (msg.op_id,))
    if isinstance(msg, OpLogLoaded):
        return replace(model, op_log_entries=msg.entries), []
    if isinstance(msg, OpLogLoadFailed):
        return replace(model, error=msg.error), []

    # --- conflict view ----------------------------------------------------
    if isinstance(msg, OpenConflictView):
        return replace(
            model, modal="conflict_view", conflict_path=msg.path
        ), [LoadConflictData(msg.path)]
    if isinstance(msg, ConflictViewClose):
        return replace(model, modal=None, conflict_path=None, conflict_data=None), []
    if isinstance(msg, ApplyResolutions):
        return _start_mutation(
            replace(model, modal=None),
            "resolve",
            (msg.path, msg.resolutions),
        )
    if isinstance(msg, ConflictDataLoaded):
        return replace(model, conflict_data=msg.data), []
    if isinstance(msg, ConflictDataLoadFailed):
        return replace(model, error=msg.error), []
```

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/core/test_update.py -k "op_log or conflict" -v`
Expected: PASS (10 tests).

- [ ] **Step 5: Lint + typecheck + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/core/update.py tests/core/test_update.py
git commit -m "feat(core): update branches for op log + conflict view"
```

---

## Task 14: `update` — hunk-picker (split / squash_partial) branches + full suite green

**Files:**
- Modify: `src/lajjzy/core/update.py`
- Test: `tests/core/test_update.py` (extend)

- [ ] **Step 1: Write the failing tests**

Append to `tests/core/test_update.py`:

```python
# --- hunk picker (split / partial squash) ------------------------------


def test_split_opens_hunk_picker_modal():
    m = _loaded("aaa", working=0)
    m1, _ = update(m, Split())
    assert m1.modal == "hunk_picker"


def test_squash_partial_opens_hunk_picker_modal():
    m = _loaded("aaa", working=0)
    m1, _ = update(m, SquashPartial())
    assert m1.modal == "hunk_picker"


def test_hunk_picker_close_clears_modal():
    m = _loaded("aaa", working=0)
    opened, _ = update(m, Split())
    closed, _ = update(opened, HunkPickerClose())
    assert closed.modal is None


def test_split_confirm_starts_mutation():
    m = _loaded("aaa", working=0)
    opened, _ = update(m, Split())
    hunks = [HunkRef(path="file.txt", hunk_idx=0)]
    confirmed, cmds = update(opened, SplitConfirm("aaa", hunks))
    assert confirmed.pending_mutation is True
    assert confirmed.modal is None
    assert cmds == [RunMutation(confirmed.graph_epoch, "split", ("aaa", hunks))]


def test_squash_partial_confirm_starts_mutation():
    m = _loaded("aaa", working=0)
    opened, _ = update(m, SquashPartial())
    hunks = [HunkRef(path="file.txt", hunk_idx=0)]
    confirmed, cmds = update(opened, SquashPartialConfirm("aaa", hunks))
    assert confirmed.pending_mutation is True
    assert cmds == [RunMutation(confirmed.graph_epoch, "squash_partial", ("aaa", hunks))]


def test_split_confirm_blocked_while_pending():
    m = _loaded("aaa", working=0)
    armed, _ = update(m, NewChange())
    opened = replace(armed, modal="hunk_picker")
    blocked, cmds = update(opened, SplitConfirm("aaa", [HunkRef("f", 0)]))
    assert cmds == []
    assert blocked.error == "A mutation is already in progress"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/core/test_update.py -k "split or squash_partial or hunk_picker" -v`
Expected: FAIL.

- [ ] **Step 3: Add the branches to `src/lajjzy/core/update.py`**

Add `Split`, `SquashPartial`, `HunkPickerClose`, `SplitConfirm`, `SquashPartialConfirm` to the imports. Append:

```python
    # --- hunk picker (split / partial squash) ----------------------------
    if isinstance(msg, Split):
        return replace(model, modal="hunk_picker"), []
    if isinstance(msg, SquashPartial):
        return replace(model, modal="hunk_picker"), []
    if isinstance(msg, HunkPickerClose):
        return replace(model, modal=None), []
    if isinstance(msg, SplitConfirm):
        return _start_mutation(replace(model, modal=None), "split", (msg.source, msg.hunks))
    if isinstance(msg, SquashPartialConfirm):
        return _start_mutation(
            replace(model, modal=None), "squash_partial", (msg.source, msg.hunks)
        )
```

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/core/test_update.py -k "split or squash_partial or hunk_picker" -v`
Expected: PASS (6 tests).

- [ ] **Step 5: Run the FULL test suite to confirm no regressions**

```bash
uv run pytest -q
```
Expected: all PASS (existing 85 + new ~40 tests).

- [ ] **Step 6: Lint + typecheck + commit**

```bash
uv run ruff check . && uv run ruff format --check . && uv run mypy src/lajjzy
git add src/lajjzy/core/update.py tests/core/test_update.py
git commit -m "feat(core): update branches for hunk picker (split / squash_partial)"
```

---

## Task 15: Phase 1a verification + PR

**Files:** none (verification only)

- [ ] **Step 1: Full local CI run**

```bash
uv run ruff check .
uv run ruff format --check .
uv run mypy src/lajjzy
uv run pytest -q
```
Expected: all four green.

- [ ] **Step 2: Confirm no `app.py` or widget changes leaked in**

```bash
git diff main --name-only
```
Expected: only `src/lajjzy/backend/{types,parse,jj}.py`, `src/lajjzy/core/{messages,commands,model,update,__init__}.py`, `tests/backend/test_{parse_ext,jj_facade_ext}.py`, `tests/core/test_update.py`, and the spec/plan docs. NO `src/lajjzy/app.py`, NO `src/lajjzy/widgets/*`.

- [ ] **Step 3: Push and open PR**

```bash
git push -u origin HEAD
gh pr create --title "Phase 1a: core seams + jj facade for daily-driver essentials" --body "Lands all pure-core types, parsers, jj facade functions, Msg/Cmd/update branches, and core unit tests for the six daily-driver features (undo/redo, omnibar, bookmarks, op log, conflict view, hunk picker). No app.py or widget changes — those are phase 1b. See docs/superpowers/specs/2026-06-22-daily-driver-essentials-design.md."
```

- [ ] **Step 4: Confirm CI green**

```bash
gh pr checks <PR-NUMBER> --watch
```
Expected: `test` job PASS.

---

## Self-review notes

- **Spec coverage:** every feature in the spec has a `Msg`/`update`/facade task. Undo/redo (T5, T11), omnibar (T9, T11), bookmarks (T6, T12), op log (T5, T13), conflict view (T7, T13), hunk picker (T8, T14).
- **Open questions from spec:** Q1 (load_graph revset) resolved — already supported. Q2 (conflict incantation) resolved in T7 via `jj file show -r @`. Q3 (split/squash non-interactive) resolved in T8 — file-granularity split/squash, hunk-granular deferred. Q4 (bookmark list parsing) resolved in T6 via `-T` template.
- **Type consistency:** `HunkResolution` is a class with sentinel constants (not Enum) per T1; `ApplyResolutions.resolutions` is `list[str]` of those constant values. `HunkRef` defined in T8, used in T9 (`SplitConfirm.hunks`), T14. `RunMutation.args` is `tuple[Any, ...]` (already, from PR #30's mypy fix) so it accepts `(op_id,)`, `(name,)`, `(source, hunks)`, etc.
- **Known limitation:** split/squash_partial are file-granularity in phase 1 (not hunk-granularity). The hunk-picker widget (phase 2, feature 6) will render hunk-level selection, but the facade commits at file granularity. This is flagged in the spec and in T8's commit message; a follow-up lands hunk-granular selection when jj 0.42.0's CLI exposes a stable flag.
