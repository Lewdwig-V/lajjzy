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
    # I1 (the test that would have caught Codex P1): _run_mutation must not be
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
