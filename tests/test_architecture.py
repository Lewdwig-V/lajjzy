import ast
import typing
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
        for marker in (
            "create_subprocess_exec",
            "subprocess.run",
            "subprocess.Popen",
            "subprocess.call",
        ):
            if marker in text:
                # app.py is allowed exactly one subprocess.run (the editor launch)
                if (
                    rel == "app.py"
                    and marker == "subprocess.run"
                    and text.count("subprocess.run") == 1
                ):
                    continue
                offenders.append(f"{rel}: {marker}")
    assert not offenders, f"subprocess outside the facade: {offenders}"


def test_mutation_worker_is_not_exclusive():
    # I1 (the test that would have caught Codex P1): _worker_mutation must not
    # be decorated @work(..., exclusive=True). (Renamed from _run_mutation when
    # the MVU refactor moved the mutation gate into the pure update function.)
    tree = _tree(SRC / "app.py")
    found = False
    for node in ast.walk(tree):
        if isinstance(node, ast.AsyncFunctionDef) and node.name == "_worker_mutation":
            found = True
            for dec in node.decorator_list:
                if isinstance(dec, ast.Call):
                    for kw in dec.keywords:
                        if kw.arg == "exclusive":
                            raise AssertionError("_worker_mutation must not use exclusive=")
    assert found, "_worker_mutation not found"


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


def test_ops_covers_all_mutation_kinds():
    """Every kind string passed as a literal to _start_mutation() in update.py
    must have an entry in _OPS in app.py. This catches a core mutation kind
    with no _OPS entry (the most dangerous drift — it produces a runtime
    InvariantError instead of a user-visible JjError).

    Note: `rebase` and `rebase_descendants` are assembled via a variable
    (`kind = "rebase_descendants" if descend else "rebase"`) rather than
    inline string literals, so they do not appear in this AST scan; they are
    verified indirectly by the existing integration tests that exercise the
    rebase flow.
    """
    from lajjzy.app import _OPS

    update_path = SRC / "core" / "update.py"
    tree = _tree(update_path)

    # Collect every string literal passed as the second positional arg to
    # _start_mutation(model, <kind>, ...) calls.
    core_kinds: set[str] = set()
    for node in ast.walk(tree):
        if (
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Name)
            and node.func.id == "_start_mutation"
            and len(node.args) >= 2
            and isinstance(node.args[1], ast.Constant)
            and isinstance(node.args[1].value, str)
        ):
            core_kinds.add(node.args[1].value)

    ops_kinds = set(_OPS.keys())
    missing_in_ops = core_kinds - ops_kinds
    assert not missing_in_ops, (
        f"Core emits mutation kind(s) not wired in _OPS: {sorted(missing_in_ops)}"
    )


def test_run_cmd_handles_all_cmds():
    """Every member of the Cmd union must have a corresponding isinstance check
    in run_cmd. This ensures run_cmd cannot silently ignore a new Cmd type."""
    from lajjzy.core.commands import Cmd as CmdUnion

    app_path = SRC / "app.py"
    tree = _tree(app_path)

    # Find the run_cmd method and collect all isinstance(cmd, X) checks.
    handled: set[str] = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.FunctionDef) and node.name == "run_cmd":
            for child in ast.walk(node):
                if (
                    isinstance(child, ast.Call)
                    and isinstance(child.func, ast.Name)
                    and child.func.id == "isinstance"
                    and len(child.args) == 2
                    and isinstance(child.args[1], ast.Name)
                ):
                    handled.add(child.args[1].id)

    # Derive the Cmd union members from the type alias's __args__.
    cmd_members: set[str] = set()
    args = typing.get_args(CmdUnion)
    for t in args:
        cmd_members.add(t.__name__)

    missing = cmd_members - handled
    assert not missing, f"run_cmd does not handle Cmd union member(s): {sorted(missing)}"


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


def test_core_modules_are_pure():
    """No module under src/lajjzy/core/ may import Textual, asyncio, subprocess,
    os, or the jj facade (lajjzy.backend.jj). Pure types from lajjzy.backend.types
    are allowed; only the I/O facade is forbidden."""
    _BANNED_TOPS = {"textual", "asyncio", "subprocess", "os"}
    _BANNED_MODULES = {"lajjzy.backend.jj"}

    core_dir = SRC / "core"
    offenders: list[str] = []

    for path in sorted(core_dir.rglob("*.py")):
        tree = _tree(path)
        rel = path.relative_to(SRC).as_posix()
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                for alias in node.names:
                    top = alias.name.split(".")[0]
                    full = alias.name
                    if top in _BANNED_TOPS or full in _BANNED_MODULES:
                        offenders.append(f"{rel}: imports {alias.name}")
            elif isinstance(node, ast.ImportFrom) and node.module:
                top = node.module.split(".")[0]
                full = node.module
                if top in _BANNED_TOPS or full in _BANNED_MODULES:
                    offenders.append(f"{rel}: imports from {node.module}")

    assert not offenders, f"core/ purity violations: {offenders}"


def test_widgets_do_not_import_jj_facade_or_subprocess():
    """Widgets project Model state and dispatch Msgs; they must never call the
    jj facade or spawn subprocesses directly (the two-facade-boundary rule).
    Textual and lajjzy.core/types imports are fine — only I/O is forbidden."""
    _BANNED_TOPS = {"subprocess"}
    _BANNED_MODULES = {"lajjzy.backend.jj"}

    widgets_dir = SRC / "widgets"
    offenders: list[str] = []

    for path in sorted(widgets_dir.rglob("*.py")):
        tree = _tree(path)
        rel = path.relative_to(SRC).as_posix()
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                for alias in node.names:
                    if alias.name.split(".")[0] in _BANNED_TOPS or alias.name in _BANNED_MODULES:
                        offenders.append(f"{rel}: imports {alias.name}")
            elif isinstance(node, ast.ImportFrom) and node.module:
                if node.module.split(".")[0] in _BANNED_TOPS or node.module in _BANNED_MODULES:
                    offenders.append(f"{rel}: imports from {node.module}")

    assert not offenders, f"widget purity violations: {offenders}"
