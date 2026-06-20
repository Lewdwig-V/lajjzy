# lajjzy Python + Textual MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild lajjzy as a reactive Python + Textual TUI for jj, reaching MVP parity (graph + navigation + detail/diff + core mutations + status bar) on the `reboot/python-textual` branch.

**Architecture:** Three layers. `backend/` is the only code that shells out to `jj` (async subprocess + pure text parsers → dataclasses). The UI is Textual-native reactivity: `reactive()` attributes on the `App` and widgets, `watch_*`/`compute_*` for derived state, and `@work` workers for jj calls. The Elm-style `Action`/`Effect`/`dispatch` machine is dissolved into reactivity + workers, not ported.

**Tech Stack:** Python 3.11+, [Textual](https://textual.textualize.io/) 6.x, `pytest` + `pytest-asyncio` + Textual `Pilot`, `ruff` for lint/format, `uv` for environment + packaging. `jj` CLI in PATH at runtime.

## Global Constraints

- **Python floor:** 3.11+ (`requires-python = ">=3.11"`).
- **Textual:** `textual>=6.0`.
- **Facade boundary:** Only `src/lajjzy/backend/jj.py` may construct or run a `jj` subprocess. No other module imports `asyncio.create_subprocess_exec`/`subprocess`. Parsers in `backend/parse.py` are pure (string in, dataclass out, no I/O).
- **No central store:** cross-cutting reactive state (`graph`, `cursor`, `error`) lives on the `App`; widget-local state lives on the widget. No single `AppState` object.
- **No panics on repo ops:** every backend function raises a typed `JjError` on failure; callers catch it and set `App.error` — never let it crash the app.
- **Working-copy gate:** any operation that reads/writes working-tree files requires the target change to be `@`; if not, switch the working copy first (`jj edit`), then proceed. (Dormant in MVP — no MVP mutation touches the working tree — but the guard helper is built in Task 12 for the deferred hunk-picker/conflict features.)
- **Editor suspend:** `$EDITOR` launches only via `with self.app.suspend():` in the app layer. The backend never launches an editor.
- **Commits:** conventional-commit messages; end each with the `Co-Authored-By` trailer used in this repo.
- **Reference:** `crates/` stays in place as the behavioral source of truth during the build and is deleted in the final task once MVP parity is verified.

---

## File Structure

```
pyproject.toml                  # project metadata, deps, scripts, ruff/pytest config
src/lajjzy/
  __init__.py
  __main__.py                   # `python -m lajjzy` → main()
  app.py                        # LajjzyApp(App): reactives graph/cursor/error, bindings, workers, main()
  backend/
    __init__.py                 # re-exports public backend API
    types.py                    # dataclasses: GraphData, GraphLine, ChangeDetail, FileChange,
                                #   FileStatus, FileDiff, DiffHunk, DiffLine; JjError
    parse.py                    # pure parsers: parse_graph_output, parse_file_line, parse_diff_output
    jj.py                       # async: run_jj, load_graph, mutations, change_diff, file_diff
  widgets/
    __init__.py
    graph.py                    # GraphView(Widget): renders graph lines, cursor, navigation
    detail.py                   # DetailPanel(Widget): file list + diff drill-down
    status_bar.py               # StatusBar(Widget): watches error + selected change
  styles.tcss                   # Textual CSS (panel widths, cursor highlight, status bar)
tests/
  backend/
    test_types.py
    test_parse.py               # pure parser tests (no jj required)
    test_jj.py                  # integration against a temp jj repo (jj required)
  conftest.py                   # temp_repo fixture
  test_app.py                   # Pilot UI tests
```

---

## Phase 1 — Scaffolding & types

### Task 1: Python project skeleton

**Files:**
- Create: `pyproject.toml`
- Create: `src/lajjzy/__init__.py`, `src/lajjzy/__main__.py`, `src/lajjzy/app.py`
- Create: `src/lajjzy/styles.tcss`
- Create: `tests/test_smoke.py`

**Interfaces:**
- Produces: `lajjzy.app.LajjzyApp` (Textual `App` subclass), `lajjzy.app.main() -> None`.

- [ ] **Step 1: Write the failing test**

```python
# tests/test_smoke.py
def test_app_constructs():
    from lajjzy.app import LajjzyApp

    app = LajjzyApp()
    assert app is not None
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/test_smoke.py -v`
Expected: FAIL — `ModuleNotFoundError: No module named 'lajjzy'` (project not yet installed).

- [ ] **Step 3: Write the project files**

```toml
# pyproject.toml
[project]
name = "lajjzy"
version = "0.1.0"
description = "A keyboard-driven, lazygit-style TUI for Jujutsu (jj)."
requires-python = ">=3.11"
license = { text = "MPL-2.0" }
dependencies = ["textual>=6.0"]

[project.scripts]
lajjzy = "lajjzy.app:main"

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[tool.hatch.build.targets.wheel]
packages = ["src/lajjzy"]

[dependency-groups]
dev = ["pytest>=8", "pytest-asyncio>=0.24", "ruff>=0.6"]

[tool.pytest.ini_options]
asyncio_mode = "auto"
testpaths = ["tests"]

[tool.ruff]
line-length = 100
src = ["src", "tests"]
```

```python
# src/lajjzy/__init__.py
"""lajjzy — a reactive TUI for Jujutsu (jj)."""
```

```python
# src/lajjzy/app.py
from textual.app import App, ComposeResult
from textual.widgets import Static


class LajjzyApp(App[None]):
    """Root application. Owns cross-cutting reactive state and key bindings."""

    CSS_PATH = "styles.tcss"

    def compose(self) -> ComposeResult:
        yield Static("lajjzy — reboot scaffold")


def main() -> None:
    LajjzyApp().run()
```

```python
# src/lajjzy/__main__.py
from lajjzy.app import main

if __name__ == "__main__":
    main()
```

```css
/* src/lajjzy/styles.tcss */
Screen {
    layout: horizontal;
}
```

- [ ] **Step 4: Install and run the test**

Run: `uv sync && uv run pytest tests/test_smoke.py -v`
Expected: PASS.

- [ ] **Step 5: Smoke-run the app builds a frame**

Run: `uv run python -c "from lajjzy.app import LajjzyApp; import asyncio; asyncio.run(LajjzyApp().run_test().__aenter__())"` (or simply `uv run lajjzy` interactively, then `q`/Ctrl-C).
Expected: app launches without error.

- [ ] **Step 6: Commit**

```bash
git add pyproject.toml src/lajjzy tests/test_smoke.py
git commit -m "feat: Python + Textual project scaffold

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: Backend domain types

**Files:**
- Create: `src/lajjzy/backend/__init__.py`, `src/lajjzy/backend/types.py`
- Create: `tests/backend/__init__.py`, `tests/backend/test_types.py`

**Interfaces:**
- Produces: dataclasses `GraphLine`, `ChangeDetail`, `FileChange`, `FileStatus` (Enum), `GraphData`, `FileDiff`, `DiffHunk`, `DiffLine`; exception `JjError`. (Mirrors `crates/lajjzy-core/src/types.rs`.)

- [ ] **Step 1: Write the failing test**

```python
# tests/backend/test_types.py
from lajjzy.backend.types import (
    ChangeDetail, FileChange, FileStatus, GraphData, GraphLine,
)


def test_graphdata_node_indices_and_lookup():
    lines = [
        GraphLine(raw="◉ abc author 1h", change_id="abc", glyph_prefix="◉ "),
        GraphLine(raw="│", change_id=None, glyph_prefix="│"),
        GraphLine(raw="◉ def author 2h", change_id="def", glyph_prefix="◉ "),
    ]
    detail = ChangeDetail(
        commit_id="c1", author="a", email="e", timestamp="1h",
        description="d", bookmarks=[], is_empty=False, conflict_count=0,
        files=[FileChange(path="x.py", status=FileStatus.MODIFIED)], parents=[],
    )
    g = GraphData(lines=lines, details={"abc": detail, "def": detail},
                  working_copy_index=0, op_id="op1")

    assert g.node_indices == [0, 2]
    assert g.change_id_at(2) == "def"
    assert g.change_id_at(1) is None
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/backend/test_types.py -v`
Expected: FAIL — `ModuleNotFoundError: No module named 'lajjzy.backend'`.

- [ ] **Step 3: Write the types**

```python
# src/lajjzy/backend/types.py
from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum


class JjError(Exception):
    """Raised when a jj operation fails. Callers catch and surface via App.error."""


class FileStatus(Enum):
    ADDED = "A"
    MODIFIED = "M"
    DELETED = "D"
    RENAMED = "R"
    CONFLICTED = "C"
    UNKNOWN = "?"


@dataclass
class FileChange:
    path: str
    status: FileStatus


@dataclass
class ChangeDetail:
    commit_id: str
    author: str
    email: str
    timestamp: str
    description: str
    bookmarks: list[str]
    is_empty: bool
    conflict_count: int
    files: list[FileChange]
    parents: list[str]


@dataclass
class GraphLine:
    raw: str
    change_id: str | None
    glyph_prefix: str


@dataclass
class DiffLine:
    kind: str  # "context" | "add" | "remove"
    text: str


@dataclass
class DiffHunk:
    header: str
    lines: list[DiffLine]


@dataclass
class FileDiff:
    path: str
    hunks: list[DiffHunk]


@dataclass
class GraphData:
    lines: list[GraphLine]
    details: dict[str, ChangeDetail]
    working_copy_index: int | None
    op_id: str
    node_indices: list[int] = field(default_factory=list)

    def __post_init__(self) -> None:
        if not self.node_indices:
            self.node_indices = [
                i for i, line in enumerate(self.lines) if line.change_id is not None
            ]

    def change_id_at(self, index: int) -> str | None:
        if 0 <= index < len(self.lines):
            return self.lines[index].change_id
        return None
```

```python
# src/lajjzy/backend/__init__.py
from lajjzy.backend.types import (
    ChangeDetail, DiffHunk, DiffLine, FileChange, FileDiff, FileStatus,
    GraphData, GraphLine, JjError,
)

__all__ = [
    "ChangeDetail", "DiffHunk", "DiffLine", "FileChange", "FileDiff",
    "FileStatus", "GraphData", "GraphLine", "JjError",
]
```

```python
# tests/backend/__init__.py
```

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/backend/test_types.py -v`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lajjzy/backend tests/backend
git commit -m "feat: backend domain types

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Phase 2 — Backend (jj subprocess + parsers)

### Task 3: Async `run_jj` subprocess helper

**Files:**
- Create: `src/lajjzy/backend/jj.py`
- Create: `tests/conftest.py`, `tests/backend/test_jj.py`

**Interfaces:**
- Consumes: `JjError` from `backend.types`.
- Produces: `async def run_jj(args: list[str], cwd: Path) -> str` (returns stdout; raises `JjError(stderr)` on non-zero exit). `temp_repo` pytest fixture yielding a `Path` to an initialized jj repo.

- [ ] **Step 1: Write the fixture and failing test**

```python
# tests/conftest.py
import shutil
import subprocess
from pathlib import Path

import pytest

jj_required = pytest.mark.skipif(
    shutil.which("jj") is None, reason="jj CLI not in PATH"
)


@pytest.fixture
def temp_repo(tmp_path: Path) -> Path:
    repo = tmp_path / "repo"
    repo.mkdir()
    subprocess.run(["jj", "git", "init"], cwd=repo, check=True,
                   capture_output=True)
    (repo / "a.txt").write_text("hello\n")
    subprocess.run(["jj", "describe", "-m", "first change"], cwd=repo,
                   check=True, capture_output=True)
    return repo
```

```python
# tests/backend/test_jj.py
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/backend/test_jj.py -v`
Expected: FAIL — `ImportError: cannot import name 'run_jj'`.

- [ ] **Step 3: Implement `run_jj`**

```python
# src/lajjzy/backend/jj.py
from __future__ import annotations

import asyncio
from pathlib import Path

from lajjzy.backend.types import JjError


async def run_jj(args: list[str], cwd: Path) -> str:
    """Run `jj <args>` in `cwd`, returning stdout. Raises JjError on failure.

    This is the ONLY place in the codebase that spawns a jj subprocess.
    """
    proc = await asyncio.create_subprocess_exec(
        "jj", *args, cwd=str(cwd),
        stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE,
    )
    stdout, stderr = await proc.communicate()
    if proc.returncode != 0:
        raise JjError(stderr.decode("utf-8", "replace").strip())
    return stdout.decode("utf-8", "replace")
```

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/backend/test_jj.py -v`
Expected: PASS (or SKIP if `jj` not in PATH).

- [ ] **Step 5: Commit**

```bash
git add src/lajjzy/backend/jj.py tests/conftest.py tests/backend/test_jj.py
git commit -m "feat: async run_jj subprocess helper

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: Pure graph-output parser

**Files:**
- Create: `src/lajjzy/backend/parse.py`
- Create: `tests/backend/test_parse.py`

**Interfaces:**
- Consumes: dataclasses from `backend.types`.
- Produces: `parse_graph_output(output: str, op_id: str) -> GraphData`, `parse_file_line(line: str) -> FileChange | None`. Constants `UNIT_SEP = "\x1f"`, `RECORD_SEP = "\x1e"`. (Port of `parse_graph_output`/`parse_file_line` in `crates/lajjzy-core/src/cli.rs`.)

**Field layout (11 records after the unit separator), copied from the Rust template:**
`change_id ▸ commit_id ▸ author ▸ email ▸ timestamp ▸ description ▸ bookmarks ▸ empty ▸ conflict ▸ working_copy_marker ▸ parents`

- [ ] **Step 1: Write the failing test**

```python
# tests/backend/test_parse.py
from lajjzy.backend.parse import RECORD_SEP, UNIT_SEP, parse_graph_output
from lajjzy.backend.types import FileStatus


def _node(display: str, fields: list[str]) -> str:
    return display + UNIT_SEP + RECORD_SEP.join(fields)


def test_parse_two_nodes_with_working_copy_and_files():
    fields_a = ["abc", "commitA", "Alice", "a@x", "1h", "first", "main",
                "false", "false", "@", ""]
    fields_b = ["def", "commitB", "Bob", "b@x", "2h", "second", "",
                "false", "false", "", "abc"]
    output = "\n".join([
        _node("◉ abc Alice 1h", fields_a),
        "M a.txt",
        "│",
        _node("◉ def Bob 2h", fields_b),
        "A b.txt",
    ]) + "\n"

    g = parse_graph_output(output, op_id="op1")

    assert g.op_id == "op1"
    assert g.working_copy_index == 0
    assert g.node_indices == [0, 2, 3]  # node, then connector at 1, node at... see note
    assert g.lines[0].change_id == "abc"
    assert g.lines[0].glyph_prefix == "◉ "
    assert g.details["abc"].author == "Alice"
    assert g.details["abc"].bookmarks == ["main"]
    assert g.details["abc"].files[0].status == FileStatus.MODIFIED
    assert g.details["def"].parents == ["abc"]
```

> Note for implementer: file lines (`M a.txt`, `A b.txt`) are *attached to the
> current change's `files`*, not emitted as graph lines. Connector lines (`│`)
> ARE graph lines with `change_id=None`. So with input order
> [nodeA, fileLine, connector, nodeB, fileLine], `lines` = [nodeA, connector,
> nodeB] and `node_indices` = [0, 2]. Fix the assertion to
> `assert g.node_indices == [0, 2]` once you confirm this against the impl.

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/backend/test_parse.py -v`
Expected: FAIL — `ImportError`.

- [ ] **Step 3: Implement the parser**

```python
# src/lajjzy/backend/parse.py
from __future__ import annotations

from lajjzy.backend.types import (
    ChangeDetail, FileChange, FileStatus, GraphData, GraphLine,
)

UNIT_SEP = "\x1f"
RECORD_SEP = "\x1e"

_STATUS_MAP = {
    "A": FileStatus.ADDED, "M": FileStatus.MODIFIED, "D": FileStatus.DELETED,
    "R": FileStatus.RENAMED, "C": FileStatus.CONFLICTED,
}


def parse_file_line(line: str) -> FileChange | None:
    """Parse a `jj log --summary` file line like 'M path/to/file'."""
    if len(line) < 2 or line[1] != " ":
        return None
    code = line[0]
    if code not in _STATUS_MAP and code not in {"A", "M", "D", "R", "C"}:
        return None
    status = _STATUS_MAP.get(code, FileStatus.UNKNOWN)
    return FileChange(path=line[2:].strip(), status=status)


def _first_alnum(s: str) -> int:
    for i, ch in enumerate(s):
        if ch.isalnum():
            return i
    return 0


def parse_graph_output(output: str, op_id: str) -> GraphData:
    lines: list[GraphLine] = []
    details: dict[str, ChangeDetail] = {}
    working_copy_index: int | None = None
    current_change_id: str | None = None

    for raw in output.splitlines():
        sep = raw.find(UNIT_SEP)
        if sep != -1:
            display = raw[:sep]
            fields = raw[sep + 1:].split(RECORD_SEP)
            if len(fields) < 11:
                raise ValueError(
                    f"Expected 11 metadata fields, got {len(fields)}: {fields!r}"
                )
            change_id = fields[0]
            current_change_id = change_id
            if change_id in details:
                raise ValueError(
                    f"Duplicate short change ID {change_id!r} (truncation collision)."
                )
            if fields[9]:  # working-copy marker "@"
                working_copy_index = len(lines)
            details[change_id] = ChangeDetail(
                commit_id=fields[1], author=fields[2], email=fields[3],
                timestamp=fields[4], description=fields[5],
                bookmarks=fields[6].split(" ") if fields[6] else [],
                is_empty=fields[7] == "true",
                conflict_count=1 if fields[8] == "true" else 0,
                files=[],
                parents=fields[10].split(" ") if fields[10] else [],
            )
            glyph_end = _first_alnum(display)
            lines.append(GraphLine(
                raw=display, change_id=change_id,
                glyph_prefix=display[:glyph_end],
            ))
            continue

        file_change = parse_file_line(raw)
        if file_change is not None and current_change_id is not None:
            details[current_change_id].files.append(file_change)
        else:
            lines.append(GraphLine(raw=raw, change_id=None, glyph_prefix=raw))

    if output.strip() and not details:
        raise ValueError(
            "Parsed jj output but found zero change nodes; template may have changed."
        )

    return GraphData(lines=lines, details=details,
                     working_copy_index=working_copy_index, op_id=op_id)
```

- [ ] **Step 4: Fix the test's `node_indices` assertion per the note, then run**

Run: `uv run pytest tests/backend/test_parse.py -v`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lajjzy/backend/parse.py tests/backend/test_parse.py
git commit -m "feat: pure jj graph-output parser

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 5: `load_graph` (template + run_jj + parse)

**Files:**
- Modify: `src/lajjzy/backend/jj.py`
- Modify: `tests/backend/test_jj.py`

**Interfaces:**
- Consumes: `run_jj`, `parse_graph_output`.
- Produces: `async def load_graph(cwd: Path, revset: str | None = None) -> GraphData`. The template string is copied verbatim from `crates/lajjzy-core/src/cli.rs:629-646`.

- [ ] **Step 1: Write the failing integration test**

```python
# add to tests/backend/test_jj.py
from lajjzy.backend.jj import load_graph


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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/backend/test_jj.py -k load_graph -v`
Expected: FAIL — `ImportError: cannot import name 'load_graph'`.

- [ ] **Step 3: Implement `load_graph`**

```python
# add to src/lajjzy/backend/jj.py
from lajjzy.backend.parse import parse_graph_output
from lajjzy.backend.types import GraphData

_GRAPH_TEMPLATE = (
    'change_id.short() ++ " " ++ '
    'coalesce(author.name(), "anonymous") ++ " " ++ '
    "committer.timestamp().ago()"
    ' ++ "\\x1f"'
    " ++ change_id.short()"
    ' ++ "\\x1e" ++ commit_id.short()'
    ' ++ "\\x1e" ++ coalesce(author.name(), "")'
    ' ++ "\\x1e" ++ coalesce(author.email(), "")'
    ' ++ "\\x1e" ++ committer.timestamp().ago()'
    ' ++ "\\x1e" ++ coalesce(description.first_line(), "")'
    ' ++ "\\x1e" ++ bookmarks'
    ' ++ "\\x1e" ++ empty'
    ' ++ "\\x1e" ++ conflict'
    ' ++ "\\x1e" ++ if(self.current_working_copy(), "@", "")'
    ' ++ "\\x1e" ++ parents.map(|p| p.change_id().short()).join(" ")'
    ' ++ "\\n"'
)


async def _op_id(cwd: Path) -> str:
    try:
        out = await run_jj(
            ["op", "log", "--limit=1", "--no-graph", "-T", "self.id().short(16)"], cwd
        )
        return out.strip() or "unknown"
    except JjError:
        return "unknown"


async def load_graph(cwd: Path, revset: str | None = None) -> GraphData:
    op_id = await _op_id(cwd)
    args = ["log", "--summary", "--color=never", "-T", _GRAPH_TEMPLATE]
    if revset is not None:
        args += ["-r", revset]
    stdout = await run_jj(args, cwd)
    return parse_graph_output(stdout, op_id)
```

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/backend/test_jj.py -k load_graph -v`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lajjzy/backend/jj.py tests/backend/test_jj.py
git commit -m "feat: load_graph via jj log template + parser

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 6: Diff parsing (`change_diff` / `file_diff`)

**Files:**
- Modify: `src/lajjzy/backend/parse.py`, `src/lajjzy/backend/jj.py`
- Modify: `tests/backend/test_parse.py`, `tests/backend/test_jj.py`

**Interfaces:**
- Produces: `parse_file_diffs(output: str) -> list[FileDiff]` (pure), `async def change_diff(cwd, change_id) -> list[FileDiff]`. Parses `jj diff --git --color=never` output. (Port of `parse_file_diffs`/`change_diff` in `cli.rs`.)

- [ ] **Step 1: Write the failing parser test**

```python
# add to tests/backend/test_parse.py
from lajjzy.backend.parse import parse_file_diffs


def test_parse_git_diff_one_file_one_hunk():
    diff = (
        "diff --git a/a.txt b/a.txt\n"
        "index 111..222 100644\n"
        "--- a/a.txt\n"
        "+++ b/a.txt\n"
        "@@ -1,2 +1,2 @@\n"
        " context\n"
        "-old\n"
        "+new\n"
    )
    files = parse_file_diffs(diff)
    assert len(files) == 1
    assert files[0].path == "a.txt"
    assert len(files[0].hunks) == 1
    kinds = [ln.kind for ln in files[0].hunks[0].lines]
    assert kinds == ["context", "remove", "add"]
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/backend/test_parse.py -k diff -v`
Expected: FAIL — `ImportError: cannot import name 'parse_file_diffs'`.

- [ ] **Step 3: Implement the diff parser and backend call**

```python
# add to src/lajjzy/backend/parse.py
from lajjzy.backend.types import DiffHunk, DiffLine, FileDiff


def parse_file_diffs(output: str) -> list[FileDiff]:
    files: list[FileDiff] = []
    current: FileDiff | None = None
    hunk: DiffHunk | None = None

    for line in output.splitlines():
        if line.startswith("diff --git "):
            # "diff --git a/<path> b/<path>" → take the b-side path.
            b = line.split(" b/", 1)
            path = b[1] if len(b) == 2 else line
            current = FileDiff(path=path, hunks=[])
            files.append(current)
            hunk = None
        elif line.startswith("@@"):
            hunk = DiffHunk(header=line, lines=[])
            if current is not None:
                current.hunks.append(hunk)
        elif hunk is not None:
            if line.startswith("+"):
                hunk.lines.append(DiffLine(kind="add", text=line[1:]))
            elif line.startswith("-"):
                hunk.lines.append(DiffLine(kind="remove", text=line[1:]))
            elif line.startswith(" "):
                hunk.lines.append(DiffLine(kind="context", text=line[1:]))
            # ignore "index", "---", "+++", "\ No newline" lines
    return files
```

```python
# add to src/lajjzy/backend/jj.py
from lajjzy.backend.parse import parse_file_diffs
from lajjzy.backend.types import FileDiff


async def change_diff(cwd: Path, change_id: str) -> list[FileDiff]:
    stdout = await run_jj(
        ["diff", "-r", change_id, "--git", "--color=never"], cwd
    )
    return parse_file_diffs(stdout)
```

- [ ] **Step 4: Add an integration test and run all backend tests**

```python
# add to tests/backend/test_jj.py
from lajjzy.backend.jj import change_diff


@jj_required
async def test_change_diff_returns_files(temp_repo):
    g = await load_graph(temp_repo)
    wc = g.lines[g.working_copy_index].change_id
    files = await change_diff(temp_repo, wc)
    assert isinstance(files, list)
```

Run: `uv run pytest tests/backend -v`
Expected: PASS / SKIP.

- [ ] **Step 5: Commit**

```bash
git add src/lajjzy/backend tests/backend
git commit -m "feat: git-diff parser + change_diff

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Phase 3 — App shell & graph view

### Task 7: App reactive state + load worker

**Files:**
- Modify: `src/lajjzy/app.py`
- Create: `tests/test_app.py`

**Interfaces:**
- Produces: `LajjzyApp` with reactives `graph: GraphData | None`, `cursor: int`, `error: str | None`; method `def selected_change_id(self) -> str | None`; `@work(group="load", exclusive=True) async def reload(self)`. App takes `repo_path: Path` (defaults to `Path.cwd()`).

- [ ] **Step 1: Write the failing Pilot test**

```python
# tests/test_app.py
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/test_app.py -v`
Expected: FAIL — `TypeError: LajjzyApp() got an unexpected keyword 'repo_path'`.

- [ ] **Step 3: Implement reactive state + load worker**

```python
# src/lajjzy/app.py
from __future__ import annotations

from pathlib import Path

from textual import work
from textual.app import App, ComposeResult
from textual.reactive import reactive
from textual.widgets import Static

from lajjzy.backend.jj import load_graph
from lajjzy.backend.types import GraphData, JjError


class LajjzyApp(App[None]):
    CSS_PATH = "styles.tcss"

    graph: reactive[GraphData | None] = reactive(None)
    cursor: reactive[int] = reactive(0)
    error: reactive[str | None] = reactive(None)

    def __init__(self, repo_path: Path | None = None) -> None:
        super().__init__()
        self.repo_path = repo_path or Path.cwd()

    def compose(self) -> ComposeResult:
        yield Static("loading…", id="placeholder")

    def on_mount(self) -> None:
        self.reload()

    @work(group="load", exclusive=True)
    async def reload(self) -> None:
        try:
            new_graph = await load_graph(self.repo_path)
        except JjError as exc:
            self.error = str(exc)
            return
        self.error = None
        self.graph = new_graph
        # Land the cursor on the working copy if known, else the first node.
        if new_graph.working_copy_index is not None:
            self.cursor = new_graph.working_copy_index
        elif new_graph.node_indices:
            self.cursor = new_graph.node_indices[0]

    def selected_change_id(self) -> str | None:
        if self.graph is None:
            return None
        return self.graph.change_id_at(self.cursor)


def main() -> None:
    LajjzyApp().run()
```

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/test_app.py -v`
Expected: PASS / SKIP.

- [ ] **Step 5: Commit**

```bash
git add src/lajjzy/app.py tests/test_app.py
git commit -m "feat: app reactive state + load worker

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 8: GraphView widget + navigation

**Files:**
- Create: `src/lajjzy/widgets/__init__.py`, `src/lajjzy/widgets/graph.py`
- Modify: `src/lajjzy/app.py`, `src/lajjzy/styles.tcss`
- Modify: `tests/test_app.py`

**Interfaces:**
- Consumes: `App.graph`, `App.cursor`.
- Produces: `GraphView(Widget)` that watches `app.graph`/`app.cursor` and renders. App gains `BINDINGS` and `action_cursor_down/up/top/bottom/working_copy` that move `cursor` over `graph.node_indices` only (skipping connector lines).

- [ ] **Step 1: Write the failing navigation test**

```python
# add to tests/test_app.py
@jj_required
async def test_j_k_move_over_nodes_only(temp_repo: Path):
    # Build a 3-change stack so there is more than one node.
    import subprocess
    subprocess.run(["jj", "new", "-m", "second"], cwd=temp_repo, check=True,
                   capture_output=True)
    subprocess.run(["jj", "new", "-m", "third"], cwd=temp_repo, check=True,
                   capture_output=True)

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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/test_app.py -k move_over_nodes -v`
Expected: FAIL — pressing `j` does nothing (no bindings yet); `app.cursor != start` fails.

- [ ] **Step 3: Implement the widget + navigation**

```python
# src/lajjzy/widgets/graph.py
from __future__ import annotations

from rich.text import Text
from textual.widget import Widget


class GraphView(Widget):
    """Renders the change graph; highlights the cursor line."""

    def on_mount(self) -> None:
        self.watch(self.app, "graph", lambda _: self.refresh())
        self.watch(self.app, "cursor", lambda _: self.refresh())

    def render(self) -> Text:
        graph = self.app.graph
        if graph is None:
            return Text("loading…")
        text = Text()
        for i, line in enumerate(graph.lines):
            style = "reverse" if i == self.app.cursor else ""
            text.append(line.raw + "\n", style=style)
        return text
```

```python
# src/lajjzy/widgets/__init__.py
from lajjzy.widgets.graph import GraphView

__all__ = ["GraphView"]
```

```python
# modify src/lajjzy/app.py — replace compose() and add bindings/actions
    BINDINGS = [
        ("j", "cursor_down", "Down"),
        ("down", "cursor_down", "Down"),
        ("k", "cursor_up", "Up"),
        ("up", "cursor_up", "Up"),
        ("g", "cursor_top", "Top"),
        ("G", "cursor_bottom", "Bottom"),
        ("R", "reload_graph", "Refresh"),
        ("q", "quit", "Quit"),
    ]

    def compose(self) -> ComposeResult:
        from lajjzy.widgets import GraphView
        yield GraphView()

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

    def action_cursor_down(self) -> None:
        self._node_index_offset(1)

    def action_cursor_up(self) -> None:
        self._node_index_offset(-1)

    def action_cursor_top(self) -> None:
        if self.graph and self.graph.node_indices:
            self.cursor = self.graph.node_indices[0]

    def action_cursor_bottom(self) -> None:
        if self.graph and self.graph.node_indices:
            self.cursor = self.graph.node_indices[-1]

    def action_reload_graph(self) -> None:
        self.reload()
```

```css
/* add to src/lajjzy/styles.tcss */
GraphView { width: 1fr; height: 1fr; }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/test_app.py -v`
Expected: PASS / SKIP.

- [ ] **Step 5: Commit**

```bash
git add src/lajjzy/widgets src/lajjzy/app.py src/lajjzy/styles.tcss tests/test_app.py
git commit -m "feat: graph view widget + node navigation

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Phase 4 — Detail panel

### Task 9: Detail panel — file list driven by selection

**Files:**
- Create: `src/lajjzy/widgets/detail.py`
- Modify: `src/lajjzy/widgets/__init__.py`, `src/lajjzy/app.py`, `src/lajjzy/styles.tcss`
- Modify: `tests/test_app.py`

**Interfaces:**
- Consumes: `App.graph`, `App.cursor`, `App.selected_change_id()`.
- Produces: `DetailPanel(Widget)` that re-renders the selected change's file list whenever the selection changes. App `compose()` now yields `GraphView` + `DetailPanel` side by side.

- [ ] **Step 1: Write the failing test**

```python
# add to tests/test_app.py
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/test_app.py -k detail_panel -v`
Expected: FAIL — `DetailPanel` does not exist / not mounted.

- [ ] **Step 3: Implement the detail panel**

```python
# src/lajjzy/widgets/detail.py
from __future__ import annotations

from rich.text import Text
from textual.widget import Widget


class DetailPanel(Widget):
    """Shows the file list for the selected change. Diff drill-down added later."""

    def on_mount(self) -> None:
        self.watch(self.app, "graph", lambda _: self.refresh())
        self.watch(self.app, "cursor", lambda _: self.refresh())

    def render(self) -> Text:
        change_id = self.app.selected_change_id()
        graph = self.app.graph
        if change_id is None or graph is None:
            return Text("")
        detail = graph.details.get(change_id)
        if detail is None:
            return Text("")
        text = Text()
        text.append(f"{change_id}  {detail.description}\n\n", style="bold")
        if not detail.files:
            text.append("(no file changes)\n", style="dim")
        for fc in detail.files:
            text.append(f"{fc.status.value} {fc.path}\n")
        return text
```

```python
# modify src/lajjzy/widgets/__init__.py
from lajjzy.widgets.detail import DetailPanel
from lajjzy.widgets.graph import GraphView

__all__ = ["DetailPanel", "GraphView"]
```

```python
# modify src/lajjzy/app.py compose()
    def compose(self) -> ComposeResult:
        from lajjzy.widgets import DetailPanel, GraphView
        yield GraphView()
        yield DetailPanel()
```

```css
/* update src/lajjzy/styles.tcss */
GraphView { width: 1fr; height: 1fr; }
DetailPanel { width: 2fr; height: 1fr; border-left: solid $panel; }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/test_app.py -v`
Expected: PASS / SKIP.

- [ ] **Step 5: Commit**

```bash
git add src/lajjzy/widgets src/lajjzy/app.py src/lajjzy/styles.tcss tests/test_app.py
git commit -m "feat: detail panel file list

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 10: Diff view drill-down

**Files:**
- Modify: `src/lajjzy/widgets/detail.py`, `src/lajjzy/app.py`
- Modify: `tests/test_app.py`

**Interfaces:**
- Consumes: `change_diff` from `backend.jj`.
- Produces: `DetailPanel` gains a `file_cursor` reactive + a `mode` ("files" | "diff") and **its own focus-scoped `BINDINGS`** for file navigation (`j`/`k`), open-diff (`enter`), and back (`escape`) — these fire only when the panel is focused. The app exposes `open_diff(path)` as `@work(group="diff", exclusive=True)`; the panel calls it. The app keeps only `tab` → `focus_detail`. No app-level `enter` binding is introduced here — that keeps `enter` free of double duty (see Task 15).

- [ ] **Step 1: Write the failing test**

```python
# add to tests/test_app.py
@jj_required
async def test_enter_opens_diff_then_esc_returns(temp_repo: Path):
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test() as pilot:
        await app.workers.wait_for_complete()
        from lajjzy.widgets.detail import DetailPanel
        panel = app.query_one(DetailPanel)
        await pilot.press("tab")        # focus detail
        await pilot.press("enter")      # open diff for first file
        await app.workers.wait_for_complete()
        assert panel.mode == "diff"
        await pilot.press("escape")
        assert panel.mode == "files"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/test_app.py -k diff_then_esc -v`
Expected: FAIL — `panel.mode` attribute does not exist.

- [ ] **Step 3: Implement the drill-down (focus-scoped bindings on the widget)**

All file-list interaction lives on the `DetailPanel` widget, so its keys only
fire while it is focused — `j`/`k`, `enter`, and `escape` never collide with the
graph's bindings.

```python
# replace src/lajjzy/widgets/detail.py
from __future__ import annotations

from rich.text import Text
from textual.reactive import reactive
from textual.widget import Widget

from lajjzy.backend.types import FileDiff


class DetailPanel(Widget):
    can_focus = True

    # Focus-scoped: these fire ONLY when the DetailPanel has focus.
    BINDINGS = [
        ("j", "file_down", "Next file"),
        ("down", "file_down", "Next file"),
        ("k", "file_up", "Prev file"),
        ("up", "file_up", "Prev file"),
        ("enter", "open_file", "Open diff"),
        ("escape", "back", "Back"),
    ]

    file_cursor: reactive[int] = reactive(0)
    mode: reactive[str] = reactive("files")  # "files" | "diff"

    def __init__(self) -> None:
        super().__init__()
        self.diff: list[FileDiff] = []

    def on_mount(self) -> None:
        self.watch(self.app, "graph", lambda _: self._on_selection_change())
        self.watch(self.app, "cursor", lambda _: self._on_selection_change())

    def _on_selection_change(self) -> None:
        self.file_cursor = 0
        self.mode = "files"
        self.diff = []
        self.refresh()

    def current_files(self) -> list:
        change_id = self.app.selected_change_id()
        graph = self.app.graph
        if change_id is None or graph is None:
            return []
        detail = graph.details.get(change_id)
        return detail.files if detail else []

    def action_file_down(self) -> None:
        if self.mode != "files":
            return
        files = self.current_files()
        if files:
            self.file_cursor = min(len(files) - 1, self.file_cursor + 1)
            self.refresh()

    def action_file_up(self) -> None:
        if self.mode != "files":
            return
        self.file_cursor = max(0, self.file_cursor - 1)
        self.refresh()

    def action_open_file(self) -> None:
        if self.mode != "files":
            return
        files = self.current_files()
        if files:
            self.app.open_diff(files[self.file_cursor].path)

    def action_back(self) -> None:
        if self.mode == "diff":
            self.mode = "files"
            self.refresh()

    def render(self) -> Text:
        if self.mode == "diff":
            return self._render_diff()
        return self._render_files()

    def _render_files(self) -> Text:
        text = Text()
        files = self.current_files()
        if not files:
            return Text("(no file changes)", style="dim")
        for i, fc in enumerate(files):
            style = "reverse" if i == self.file_cursor else ""
            text.append(f"{fc.status.value} {fc.path}\n", style=style)
        return text

    def _render_diff(self) -> Text:
        text = Text()
        for fd in self.diff:
            text.append(f"{fd.path}\n", style="bold")
            for hunk in fd.hunks:
                text.append(hunk.header + "\n", style="cyan")
                for ln in hunk.lines:
                    style = {"add": "green", "remove": "red"}.get(ln.kind, "")
                    sign = {"add": "+", "remove": "-"}.get(ln.kind, " ")
                    text.append(f"{sign}{ln.text}\n", style=style)
        return text
```

```python
# add to src/lajjzy/app.py — ONLY a focus action + the diff worker.
# extend BINDINGS with exactly:
#   ("tab", "focus_detail", "Detail"),
# (No app-level enter/escape here — DetailPanel owns those while focused.)

    def action_focus_detail(self) -> None:
        from lajjzy.widgets import DetailPanel
        self.query_one(DetailPanel).focus()

    @work(group="diff", exclusive=True)
    async def open_diff(self, path: str) -> None:
        from lajjzy.backend.jj import change_diff
        from lajjzy.widgets import DetailPanel
        change_id = self.selected_change_id()
        if change_id is None:
            return
        try:
            all_files = await change_diff(self.repo_path, change_id)
        except JjError as exc:
            self.error = str(exc)
            return
        panel = self.query_one(DetailPanel)
        panel.diff = [fd for fd in all_files if fd.path == path] or all_files
        panel.mode = "diff"
        panel.refresh()
```

> Why this shape: by putting `enter`/`escape`/`j`/`k` on the widget rather than
> the app, each key has exactly one meaning per focus context. `enter` opens a
> diff only when the detail panel is focused; the app never needs an `enter`
> binding that branches on hidden state. This is the principle-of-least-surprise
> fix that Task 15 depends on.

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/test_app.py -v`
Expected: PASS / SKIP.

- [ ] **Step 5: Commit**

```bash
git add src/lajjzy/widgets/detail.py src/lajjzy/app.py tests/test_app.py
git commit -m "feat: diff drill-down in detail panel

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Phase 5 — Mutations

### Task 11: Mutation worker infra + `new` / `abandon`

**Files:**
- Modify: `src/lajjzy/backend/jj.py`, `src/lajjzy/app.py`
- Modify: `tests/backend/test_jj.py`, `tests/test_app.py`

**Interfaces:**
- Produces: backend `async def new_change(cwd, after) -> str`, `async def abandon(cwd, change_id) -> str` (mirror `cli.rs`: `jj new --insert-after <after>`, `jj abandon <id>`). App gains a single `@work(group="mutation", exclusive=True) async def _mutate(self, coro_factory, ...)` helper that runs a mutation then `reload()`s; actions `action_new` (`n`) and `action_abandon` (`d`).

- [ ] **Step 1: Write failing backend tests**

```python
# add to tests/backend/test_jj.py
from lajjzy.backend.jj import abandon, new_change


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
    subprocess.run(["jj", "new", "-m", "doomed"], cwd=temp_repo, check=True,
                   capture_output=True)
    g = await load_graph(temp_repo)
    target = g.lines[g.working_copy_index].change_id
    before = len(g.details)
    await abandon(temp_repo, target)
    after = len((await load_graph(temp_repo)).details)
    assert after == before - 1
```

- [ ] **Step 2: Run to verify it fails**

Run: `uv run pytest tests/backend/test_jj.py -k "new_change or abandon" -v`
Expected: FAIL — `ImportError`.

- [ ] **Step 3: Implement backend + app mutation infra**

```python
# add to src/lajjzy/backend/jj.py
async def new_change(cwd: Path, after: str) -> str:
    await run_jj(["new", "--insert-after", after], cwd)
    return f"Created new change after {after}"


async def abandon(cwd: Path, change_id: str) -> str:
    await run_jj(["abandon", change_id], cwd)
    return f"Abandoned {change_id}"
```

```python
# add to src/lajjzy/app.py
# extend BINDINGS with ("n", "new", "New"), ("d", "abandon", "Abandon")
from collections.abc import Awaitable, Callable

    @work(group="mutation", exclusive=True)
    async def _mutate(self, op: Callable[[], Awaitable[str]]) -> None:
        try:
            message = await op()
        except JjError as exc:
            self.error = str(exc)
            return
        self.error = message
        # Reload synchronously inside this worker so the graph reflects the result.
        from lajjzy.backend.jj import load_graph
        try:
            self.graph = await load_graph(self.repo_path)
        except JjError as exc:
            self.error = str(exc)
            return
        if self.graph.working_copy_index is not None:
            self.cursor = self.graph.working_copy_index

    def action_new(self) -> None:
        from lajjzy.backend.jj import new_change
        target = self.selected_change_id()
        if target is None:
            return
        self._mutate(lambda: new_change(self.repo_path, target))

    def action_abandon(self) -> None:
        from lajjzy.backend.jj import abandon
        target = self.selected_change_id()
        if target is None:
            return
        self._mutate(lambda: abandon(self.repo_path, target))
```

> Note: `group="mutation", exclusive=True` is the framework-enforced
> replacement for the Rust `pending_mutation` gate — a second mutation
> cancels the first in-flight one.

- [ ] **Step 4: Write + run an app-level mutation test**

```python
# add to tests/test_app.py
@jj_required
async def test_press_n_creates_change(temp_repo: Path):
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test() as pilot:
        await app.workers.wait_for_complete()
        before = len(app.graph.details)
        await pilot.press("n")
        await app.workers.wait_for_complete()
        assert len(app.graph.details) == before + 1
```

Run: `uv run pytest tests/backend/test_jj.py tests/test_app.py -v`
Expected: PASS / SKIP.

- [ ] **Step 5: Commit**

```bash
git add src/lajjzy/backend/jj.py src/lajjzy/app.py tests/backend/test_jj.py tests/test_app.py
git commit -m "feat: mutation worker lane + new/abandon

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 12: `edit` + working-copy gate helper

**Files:**
- Modify: `src/lajjzy/backend/jj.py`, `src/lajjzy/app.py`
- Modify: `tests/backend/test_jj.py`

**Interfaces:**
- Produces: backend `async def edit_change(cwd, change_id) -> str` (`jj edit <id>`). App `action_edit` (`e` is reserved for describe in MVP — bind edit to `ctrl+e` to match the Rust keymap "switch working copy") and `async def ensure_working_copy(self, change_id) -> bool` that edits to `change_id` if it is not already `@`. This helper is the working-copy gate for deferred filesystem features.

- [ ] **Step 1: Write the failing backend test**

```python
# add to tests/backend/test_jj.py
from lajjzy.backend.jj import edit_change


@jj_required
async def test_edit_moves_working_copy(temp_repo):
    import subprocess
    subprocess.run(["jj", "new", "-m", "child"], cwd=temp_repo, check=True,
                   capture_output=True)
    g = await load_graph(temp_repo)
    # pick a non-@ node (the parent)
    parents = g.details[g.lines[g.working_copy_index].change_id].parents
    assert parents
    await edit_change(temp_repo, parents[0])
    g2 = await load_graph(temp_repo)
    assert g2.lines[g2.working_copy_index].change_id == parents[0]
```

- [ ] **Step 2: Run to verify it fails**

Run: `uv run pytest tests/backend/test_jj.py -k edit -v`
Expected: FAIL — `ImportError`.

- [ ] **Step 3: Implement edit + gate**

```python
# add to src/lajjzy/backend/jj.py
async def edit_change(cwd: Path, change_id: str) -> str:
    await run_jj(["edit", change_id], cwd)
    return f"Now editing {change_id}"
```

```python
# add to src/lajjzy/app.py
# extend BINDINGS with ("ctrl+e", "edit", "Edit @")

    def action_edit(self) -> None:
        from lajjzy.backend.jj import edit_change
        target = self.selected_change_id()
        if target is None:
            return
        self._mutate(lambda: edit_change(self.repo_path, target))

    async def ensure_working_copy(self, change_id: str) -> bool:
        """Working-copy gate: make `change_id` the @ commit before any
        filesystem-touching op. Returns True if @ is (now) the target.
        Used by deferred hunk-picker / conflict features."""
        from lajjzy.backend.jj import edit_change
        if self.graph and self.graph.working_copy_index is not None:
            current = self.graph.lines[self.graph.working_copy_index].change_id
            if current == change_id:
                return True
        try:
            await edit_change(self.repo_path, change_id)
        except JjError as exc:
            self.error = str(exc)
            return False
        return True
```

- [ ] **Step 4: Run tests**

Run: `uv run pytest tests/backend/test_jj.py -k edit -v && uv run pytest tests/test_app.py -v`
Expected: PASS / SKIP.

- [ ] **Step 5: Commit**

```bash
git add src/lajjzy/backend/jj.py src/lajjzy/app.py tests/backend/test_jj.py
git commit -m "feat: edit change + working-copy gate helper

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 13: `describe` via `$EDITOR` suspend

**Files:**
- Modify: `src/lajjzy/backend/jj.py`, `src/lajjzy/app.py`
- Modify: `tests/backend/test_jj.py`

**Interfaces:**
- Produces: backend `async def describe(cwd, change_id, text) -> str` (`jj describe <id> -m <text>`). App `action_describe` (`e`) launches `$EDITOR` via `with self.suspend():` to capture a message, then runs `describe` on the mutation lane. The editor launch is the app layer's responsibility (Global Constraint), not the backend's.

- [ ] **Step 1: Write the failing backend test**

```python
# add to tests/backend/test_jj.py
from lajjzy.backend.jj import describe


@jj_required
async def test_describe_sets_message(temp_repo):
    g = await load_graph(temp_repo)
    wc = g.lines[g.working_copy_index].change_id
    await describe(temp_repo, wc, "a brand new message")
    g2 = await load_graph(temp_repo)
    assert g2.details[wc].description == "a brand new message"
```

- [ ] **Step 2: Run to verify it fails**

Run: `uv run pytest tests/backend/test_jj.py -k describe -v`
Expected: FAIL — `ImportError`.

- [ ] **Step 3: Implement describe + editor suspend**

```python
# add to src/lajjzy/backend/jj.py
async def describe(cwd: Path, change_id: str, text: str) -> str:
    await run_jj(["describe", change_id, "-m", text], cwd)
    first_line = text.splitlines()[0] if text.strip() else "(no message)"
    return f'Described {change_id}: "{first_line}"'
```

```python
# add to src/lajjzy/app.py
# extend BINDINGS with ("e", "describe", "Describe")
import os
import subprocess
import tempfile

    def action_describe(self) -> None:
        target = self.selected_change_id()
        if target is None or self.graph is None:
            return
        seed = self.graph.details[target].description
        message = self._edit_message_in_editor(seed)
        if message is None:
            return  # user aborted / editor unavailable
        from lajjzy.backend.jj import describe
        self._mutate(lambda: describe(self.repo_path, target, message))

    def _edit_message_in_editor(self, seed: str) -> str | None:
        editor = os.environ.get("EDITOR")
        if not editor:
            self.error = "No $EDITOR set"
            return None
        with tempfile.NamedTemporaryFile(
            "w+", suffix=".jjdescribe", delete=False
        ) as tf:
            tf.write(seed)
            path = tf.name
        with self.suspend():  # hand the terminal to $EDITOR
            subprocess.run([*editor.split(), path], check=False)
        try:
            with open(path, encoding="utf-8") as fh:
                return fh.read().strip()
        finally:
            os.unlink(path)
```

> Note: `self.suspend()` is Textual's context manager that drops the app out of
> the alternate screen so an external program owns the terminal, then restores
> on exit — the reactive-native equivalent of the Rust `SuspendForEditor`
> effect intercepted in `main.rs`.

- [ ] **Step 4: Run the backend test (editor path is exercised manually)**

Run: `uv run pytest tests/backend/test_jj.py -k describe -v`
Expected: PASS / SKIP.
Manual: `EDITOR=nano uv run lajjzy` in a repo, press `e`, edit, save — description updates and the graph reloads.

- [ ] **Step 5: Commit**

```bash
git add src/lajjzy/backend/jj.py src/lajjzy/app.py tests/backend/test_jj.py
git commit -m "feat: describe via \$EDITOR suspend

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 14: `squash` into parent

**Files:**
- Modify: `src/lajjzy/backend/jj.py`, `src/lajjzy/app.py`
- Modify: `tests/backend/test_jj.py`

**Interfaces:**
- Produces: backend `async def squash(cwd, change_id) -> str` — full squash of `change_id` into its parent via `jj squash --from <change_id>` (MVP: whole-change squash, no hunk picker). App `action_squash` (`S`) on the mutation lane.

- [ ] **Step 1: Write the failing backend test**

```python
# add to tests/backend/test_jj.py
from lajjzy.backend.jj import squash


@jj_required
async def test_squash_collapses_into_parent(temp_repo):
    import subprocess
    subprocess.run(["jj", "new", "-m", "child"], cwd=temp_repo, check=True,
                   capture_output=True)
    (temp_repo / "b.txt").write_text("more\n")
    g = await load_graph(temp_repo)
    child = g.lines[g.working_copy_index].change_id
    before = len(g.details)
    await squash(temp_repo, child)
    after = len((await load_graph(temp_repo)).details)
    assert after == before - 1
```

- [ ] **Step 2: Run to verify it fails**

Run: `uv run pytest tests/backend/test_jj.py -k squash -v`
Expected: FAIL — `ImportError`.

- [ ] **Step 3: Implement squash**

```python
# add to src/lajjzy/backend/jj.py
async def squash(cwd: Path, change_id: str) -> str:
    await run_jj(["squash", "--from", change_id], cwd)
    return f"Squashed {change_id} into its parent"
```

```python
# add to src/lajjzy/app.py
# extend BINDINGS with ("S", "squash", "Squash")
    def action_squash(self) -> None:
        from lajjzy.backend.jj import squash
        target = self.selected_change_id()
        if target is None:
            return
        self._mutate(lambda: squash(self.repo_path, target))
```

- [ ] **Step 4: Run tests**

Run: `uv run pytest tests/backend/test_jj.py -k squash -v`
Expected: PASS / SKIP.

- [ ] **Step 5: Commit**

```bash
git add src/lajjzy/backend/jj.py src/lajjzy/app.py tests/backend/test_jj.py
git commit -m "feat: squash change into parent

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 15: `rebase` with target-picking mode

**Files:**
- Modify: `src/lajjzy/backend/jj.py`, `src/lajjzy/app.py`
- Modify: `tests/backend/test_jj.py`, `tests/test_app.py`

**Interfaces:**
- Produces: backend `async def rebase_single(cwd, source, destination) -> str` (`jj rebase -r <source> --onto <destination>`) and `async def rebase_with_descendants(cwd, source, destination) -> str` (`jj rebase -s <source> --onto <destination>`). App enters a **visibly-signposted** rebase picking mode: `r` arms rebase-single, `ctrl+r` arms rebase-with-descendants; the status bar shows a prompt and the source is dimmed; navigation chooses a destination. App-level `enter` → `action_rebase_confirm` and `escape` → `action_rebase_cancel` are each **single-purpose** — `enter` is a no-op outside rebase mode, so it never does double duty with the detail panel's focus-scoped `enter`.

- [ ] **Step 1: Write the failing backend test**

```python
# add to tests/backend/test_jj.py
from lajjzy.backend.jj import rebase_single


@jj_required
async def test_rebase_single_reparents(temp_repo):
    import subprocess
    # root → A; create sibling B off root; rebase B onto A
    subprocess.run(["jj", "new", "-m", "A"], cwd=temp_repo, check=True,
                   capture_output=True)
    g = await load_graph(temp_repo)
    a = g.lines[g.working_copy_index].change_id
    subprocess.run(["jj", "new", "root()", "-m", "B"], cwd=temp_repo,
                   check=True, capture_output=True)
    g = await load_graph(temp_repo)
    b = g.lines[g.working_copy_index].change_id
    await rebase_single(temp_repo, b, a)
    g2 = await load_graph(temp_repo)
    assert a in g2.details[b].parents
```

- [ ] **Step 2: Run to verify it fails**

Run: `uv run pytest tests/backend/test_jj.py -k rebase -v`
Expected: FAIL — `ImportError`.

- [ ] **Step 3: Implement rebase backend + picking mode**

```python
# add to src/lajjzy/backend/jj.py
async def rebase_single(cwd: Path, source: str, destination: str) -> str:
    await run_jj(["rebase", "-r", source, "--onto", destination], cwd)
    return f"Rebased {source} onto {destination}"


async def rebase_with_descendants(cwd: Path, source: str, destination: str) -> str:
    await run_jj(["rebase", "-s", source, "--onto", destination], cwd)
    return f"Rebased {source} + descendants onto {destination}"
```

```python
# add to src/lajjzy/app.py
# extend BINDINGS with:
#   ("r", "rebase", "Rebase"),
#   ("ctrl+r", "rebase_descendants", "Rebase+desc"),
#   ("enter", "rebase_confirm", "Confirm rebase"),
#   ("escape", "rebase_cancel", "Cancel"),
# These app-level enter/escape are single-purpose: they act only while
# rebase mode is armed. When the DetailPanel is focused, ITS focus-scoped
# enter/escape (Task 10) handle the key first and these never fire — so no
# key ever branches on hidden state.

    rebase_source: reactive[str | None] = reactive(None)
    rebase_descendants_flag: reactive[bool] = reactive(False)

    def action_rebase(self) -> None:
        self.rebase_source = self.selected_change_id()
        self.rebase_descendants_flag = False
        if self.rebase_source:
            self.error = "Rebase: pick a destination, Enter to confirm, Esc to cancel"

    def action_rebase_descendants(self) -> None:
        self.rebase_source = self.selected_change_id()
        self.rebase_descendants_flag = True
        if self.rebase_source:
            self.error = "Rebase +desc: pick a destination, Enter to confirm, Esc to cancel"

    def action_rebase_confirm(self) -> None:
        # No-op unless rebase mode is armed — Enter does exactly one thing.
        if self.rebase_source is None:
            return
        dest = self.selected_change_id()
        src = self.rebase_source
        descend = self.rebase_descendants_flag
        self.rebase_source = None
        if dest is None or dest == src:
            self.error = "Rebase cancelled (invalid destination)"
            return
        from lajjzy.backend.jj import rebase_single, rebase_with_descendants
        op = rebase_with_descendants if descend else rebase_single
        self._mutate(lambda: op(self.repo_path, src, dest))

    def action_rebase_cancel(self) -> None:
        # No-op unless rebase mode is armed.
        if self.rebase_source is not None:
            self.rebase_source = None
            self.error = "Rebase cancelled"
```

> Why no double duty: app-level `enter` maps to `action_rebase_confirm`, which
> returns immediately unless `rebase_source` is set. Diff-opening `enter` lives
> on the focused `DetailPanel` (Task 10) and is resolved before the key bubbles
> to the app. Each binding has one meaning; neither inspects the other's state.
> The GraphView should dim `rebase_source` and its descendants while armed so
> the mode is visible, not hidden.

- [ ] **Step 4: Run tests**

Run: `uv run pytest tests/backend/test_jj.py -k rebase -v && uv run pytest tests/test_app.py -v`
Expected: PASS / SKIP.

- [ ] **Step 5: Commit**

```bash
git add src/lajjzy/backend/jj.py src/lajjzy/app.py tests/backend/test_jj.py tests/test_app.py
git commit -m "feat: rebase with target-picking mode

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Phase 6 — Status bar & error reactivity

### Task 16: Status bar widget

**Files:**
- Create: `src/lajjzy/widgets/status_bar.py`
- Modify: `src/lajjzy/widgets/__init__.py`, `src/lajjzy/app.py`, `src/lajjzy/styles.tcss`
- Modify: `tests/test_app.py`

**Interfaces:**
- Consumes: `App.error`, `App.selected_change_id()`, `App.rebase_source`.
- Produces: `StatusBar(Widget)` that watches `app.error` (shown first when set) and otherwise shows selected-change metadata; mirrors the priority ordering of `crates/lajjzy-tui/src/widgets/status_bar.rs`.

- [ ] **Step 1: Write the failing test**

```python
# add to tests/test_app.py
@jj_required
async def test_status_bar_shows_error(temp_repo: Path):
    app = LajjzyApp(repo_path=temp_repo)
    async with app.run_test():
        await app.workers.wait_for_complete()
        from lajjzy.widgets.status_bar import StatusBar
        app.error = "boom"
        bar = app.query_one(StatusBar)
        assert "boom" in str(bar.render())
```

- [ ] **Step 2: Run to verify it fails**

Run: `uv run pytest tests/test_app.py -k status_bar -v`
Expected: FAIL — `StatusBar` does not exist.

- [ ] **Step 3: Implement the status bar**

```python
# src/lajjzy/widgets/status_bar.py
from __future__ import annotations

from rich.text import Text
from textual.widget import Widget


class StatusBar(Widget):
    """Priority-ordered status line: error > rebase prompt > change metadata."""

    def on_mount(self) -> None:
        self.watch(self.app, "error", lambda _: self.refresh())
        self.watch(self.app, "cursor", lambda _: self.refresh())
        self.watch(self.app, "graph", lambda _: self.refresh())

    def render(self) -> Text:
        if self.app.error:
            return Text(self.app.error, style="bold red")
        change_id = self.app.selected_change_id()
        graph = self.app.graph
        if change_id is None or graph is None:
            return Text("")
        d = graph.details.get(change_id)
        if d is None:
            return Text("")
        parts = [change_id, d.author, d.timestamp]
        if d.bookmarks:
            parts.append("bookmarks: " + ", ".join(d.bookmarks))
        if d.conflict_count:
            parts.append("CONFLICT")
        return Text("  |  ".join(parts))
```

```python
# modify src/lajjzy/widgets/__init__.py
from lajjzy.widgets.detail import DetailPanel
from lajjzy.widgets.graph import GraphView
from lajjzy.widgets.status_bar import StatusBar

__all__ = ["DetailPanel", "GraphView", "StatusBar"]
```

```python
# modify src/lajjzy/app.py compose() to use a vertical layout with the bar at the bottom
from textual.containers import Horizontal

    def compose(self) -> ComposeResult:
        from lajjzy.widgets import DetailPanel, GraphView, StatusBar
        with Horizontal(id="panes"):
            yield GraphView()
            yield DetailPanel()
        yield StatusBar()
```

```css
/* update src/lajjzy/styles.tcss */
Screen { layout: vertical; }
#panes { height: 1fr; }
GraphView { width: 1fr; height: 1fr; }
DetailPanel { width: 2fr; height: 1fr; border-left: solid $panel; }
StatusBar { height: 1; dock: bottom; background: $panel; }
```

- [ ] **Step 4: Run all tests**

Run: `uv run pytest -v`
Expected: PASS / SKIP.

- [ ] **Step 5: Commit**

```bash
git add src/lajjzy/widgets src/lajjzy/app.py src/lajjzy/styles.tcss tests/test_app.py
git commit -m "feat: status bar with error + metadata priority

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Phase 7 — Docs & cut-over

### Task 17: Retool README for Python/Textual

**Files:**
- Modify: `README.md`
- Create: `CLAUDE.md` (replace Rust content)

**Interfaces:** none (docs only).

- [ ] **Step 1: Rewrite the install section**

Replace the cargo/binstall/Homebrew/Nix block with:

```markdown
## Install

```bash
# Recommended: isolated tool install
uv tool install lajjzy
# or
pipx install lajjzy
```

**Requirements:** Python 3.11+ and the `jj` CLI in PATH (tested with jj 0.39.0).
```

- [ ] **Step 2: Rewrite the Architecture section**

Replace the three-crate / Elm description with:

```markdown
## Architecture

Three layers with a strict facade boundary:

- **`backend/`** — the only code that shells out to `jj`. Async functions
  (`asyncio.create_subprocess_exec`) returning typed dataclasses; pure parsers
  in `parse.py`. No Textual imports.
- **reactive UI** — Textual `reactive()` attributes on the `App` and widgets,
  with `watch_*`/`compute_*` for derived state. No central store object.
- **workers** — every jj call runs in a `@work` worker. Concurrency lanes are
  worker groups: `group="mutation"` (with `exclusive=True`, the
  single-mutation gate), `group="push"`, `group="fetch"`.

There is no `dispatch`/`Effect` machine — actions invoke workers, workers write
reactive state, and the affected widgets re-render automatically.
```

- [ ] **Step 3: Trim the Features section to MVP truth**

Mark deferred features explicitly. Under each deferred section (omnibar, hunk
picker, conflict view, bookmark UI, op log, mouse, GitHub) add a leading line:
`> **Status:** planned — not yet in the Python reboot.` Update the Describe
section to drop "powered by tui-textarea" and say the long-form editor uses
`$EDITOR` via terminal suspend. Update the keymap tables to match the MVP
bindings actually implemented (`j/k/g/G`, `R`, `Tab`, `Enter`, `Esc`, `n`, `d`,
`e`, `ctrl+e`, `S`, `r`, `ctrl+r`, `q`).

- [ ] **Step 4: Replace the Roadmap**

```markdown
## Roadmap

### Reboot (in progress)
- **R1 — MVP core (this branch):** graph + navigation, detail/diff panel,
  core mutations (`new`, `describe`, `edit`, `abandon`, `squash`, `rebase`),
  status bar + error reactivity. *(shipped when this plan completes)*

### Feature-port backlog (port from the Rust reference, incrementally)
- **P1 — Omnibar:** revset search + completion (functions, bookmarks, change IDs).
- **P2 — Hunk picker:** interactive split & partial squash.
- **P3 — Conflict view:** base/left/right resolution panes.
- **P4 — Bookmark UI:** picker + set/delete.
- **P5 — Op log:** browse + restore.
- **P6 — Mouse support:** lazygit-style click/scroll.
- **P7 — GitHub integration:** `gh`-backed PR status + open/create.

### Future features (post-parity)
- **F1 — Configurable keymaps**
- **F2 — Theming:** colour sets, nerd-font / emoji support.
- **F3 — Blame / annotate**
- **F4 — Parallel-branch lane view**
- **F5 — Stacked-PR management**
```

- [ ] **Step 5: Replace the Development + Releasing sections and CLAUDE.md**

Replace the Development block with:

```markdown
## Development

```bash
uv sync                 # create the environment
uv run lajjzy           # run the TUI
uv run pytest           # run tests (jj in PATH required for integration tests)
uv run ruff check .     # lint
uv run ruff format .    # format
```
```

Replace the Releasing section with a `uv build` + `uv publish` flow. Rewrite
`CLAUDE.md` to describe the Python layout, the facade/no-central-store/worker-lane
constraints from this plan's Global Constraints, and the `uv` commands.

- [ ] **Step 6: Commit**

```bash
git add README.md CLAUDE.md
git commit -m "docs: retool README + CLAUDE.md for Python/Textual

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 18: Burn the boats — remove the Rust tree

**Files:**
- Delete: `crates/`, `Cargo.toml`, `Cargo.lock`, `rust-toolchain*`, `.unslop/` (if Rust-specific), Rust-specific CI under `.github/workflows/`
- Modify: `.gitignore` (drop `target/`, add Python ignores)

**Interfaces:** none.

- [ ] **Step 1: Verify MVP parity before deleting**

Run the full Python suite and a manual smoke test:

Run: `uv run pytest -v`
Expected: all PASS / SKIP, zero failures.
Manual: in a real jj repo, `uv run lajjzy` — navigate, open a diff, create/abandon/describe/squash/rebase a change, observe status bar + errors.

- [ ] **Step 2: Remove the Rust tree and CI**

```bash
git rm -r crates Cargo.toml Cargo.lock
git rm -rf .github/workflows  # remove Rust release/CI workflows (re-add Python CI later)
```

- [ ] **Step 3: Update `.gitignore`**

```gitignore
# Python
__pycache__/
*.py[cod]
.venv/
.pytest_cache/
.ruff_cache/
dist/
*.egg-info/
```

- [ ] **Step 4: Verify the repo still builds + tests from a clean state**

Run: `rm -rf .venv && uv sync && uv run pytest -v`
Expected: environment recreates; tests PASS / SKIP.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore: remove Rust tree — Python reboot reaches MVP parity

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Self-Review

**Spec coverage** (design § → task):
- Backend facade (jj CLI + parsers) → Tasks 3–6. ✔
- Reactive state, no central store → Task 7 (App reactives), widget-local state in Tasks 8–10, 16. ✔
- Concurrency lanes → worker groups → Task 11 (`mutation`, exclusive), diff worker Task 10 (`diff`), load worker Task 7 (`load`). Push/fetch lanes are deferred features (no MVP mutation uses them) — noted in README roadmap P-series. ✔
- Editor suspend boundary in app layer → Task 13. ✔
- Working-copy gate → Task 12 (`ensure_working_copy`). ✔
- MVP mutations (`new`, `describe`, `edit`, `abandon`, `squash`, `rebase`) → Tasks 11–15. ✔
- Status bar + error reactivity → Task 16. ✔
- Testing (pytest + Pilot, temp-repo integration) → conftest Task 3, Pilot tests Tasks 7–16. ✔
- README roadmap retool → Task 17. ✔
- Burn-the-boats (deferred to parity) → Task 18. ✔
- Distribution via `uv tool`/`pipx` → Tasks 1 (`[project.scripts]`) + 17 (install docs). ✔

**Placeholder scan:** no TBD/TODO; every code step carries real code. Both
former soft spots are now resolved in-plan, not deferred: file-list navigation
and diff-open are **focus-scoped `BINDINGS` on `DetailPanel`** (Task 10), and
rebase confirm/cancel are **single-purpose app-level actions** that no-op
outside the armed mode (Task 15). No key branches on hidden state.

**Principle of least surprise (explicit check):** `enter` has exactly one
meaning per focus context — open-diff while `DetailPanel` is focused, confirm
rebase while rebase mode is armed (graph focus). The two bindings live in
different scopes and never inspect each other's state. `escape` likewise:
back-out a diff (panel-scoped) or cancel a rebase (app-level no-op otherwise).
The rebase mode is visibly signposted (status prompt + dimmed source), so its
`enter` semantics are never a surprise.

**Type consistency:** dataclass field names (`change_id`, `glyph_prefix`, `working_copy_index`, `node_indices`, `conflict_count`) are used identically across Tasks 2, 4, 7, 8, 16. Backend function names (`load_graph`, `change_diff`, `new_change`, `abandon`, `edit_change`, `describe`, `squash`, `rebase_single`, `rebase_with_descendants`) match between their defining task and their app-layer callers. The diff worker is `App.open_diff` (called from `DetailPanel.action_open_file`). Worker group names (`load`, `diff`, `mutation`) are consistent.
