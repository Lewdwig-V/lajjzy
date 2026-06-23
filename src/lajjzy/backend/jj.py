from __future__ import annotations

import asyncio
from pathlib import Path

from lajjzy.backend.parse import parse_file_diffs, parse_graph_output, parse_op_log
from lajjzy.backend.types import FileDiff, GraphData, JjError, OpLogEntry


async def run_jj(args: list[str], cwd: Path) -> str:
    """Run `jj <args>` in `cwd`, returning stdout. Raises JjError on failure.

    This is the ONLY place in the codebase that spawns a jj subprocess.
    """
    proc = await asyncio.create_subprocess_exec(
        "jj",
        *args,
        cwd=str(cwd),
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    stdout, stderr = await proc.communicate()
    if proc.returncode != 0:
        raise JjError(stderr.decode("utf-8", "replace").strip())
    return stdout.decode("utf-8", "replace")


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


async def change_diff(cwd: Path, change_id: str) -> list[FileDiff]:
    stdout = await run_jj(["diff", "-r", change_id, "--git", "--color=never"], cwd)
    try:
        return parse_file_diffs(stdout)
    except ValueError as exc:
        raise JjError(str(exc)) from exc


async def load_graph(cwd: Path, revset: str | None = None) -> GraphData:
    op_id = await _op_id(cwd)
    args = ["log", "--summary", "--color=never", "-T", _GRAPH_TEMPLATE]
    if revset is not None:
        args += ["-r", revset]
    stdout = await run_jj(args, cwd)
    try:
        return parse_graph_output(stdout, op_id)
    except ValueError as exc:
        raise JjError(str(exc)) from exc


async def new_change(cwd: Path, after: str) -> str:
    await run_jj(["new", "--insert-after", after], cwd)
    return f"Created new change after {after}"


async def abandon(cwd: Path, change_id: str) -> str:
    await run_jj(["abandon", change_id], cwd)
    return f"Abandoned {change_id}"


async def edit_change(cwd: Path, change_id: str) -> str:
    await run_jj(["edit", change_id], cwd)
    return f"Now editing {change_id}"


async def describe(cwd: Path, change_id: str, text: str) -> str:
    await run_jj(["describe", change_id, "-m", text], cwd)
    first_line = text.splitlines()[0] if text.strip() else "(no message)"
    return f'Described {change_id}: "{first_line}"'


async def squash(cwd: Path, change_id: str) -> str:
    # jj squash -r <id> moves <id>'s contents into its parent and abandons <id>.
    # --use-destination-message keeps the parent's description non-interactively
    # (--from <id> would move contents INTO @ instead, not into the parent).
    await run_jj(["squash", "-r", change_id, "--use-destination-message"], cwd)
    return f"Squashed {change_id} into its parent"


async def rebase_single(cwd: Path, source: str, destination: str) -> str:
    await run_jj(["rebase", "-r", source, "--onto", destination], cwd)
    return f"Rebased {source} onto {destination}"


async def rebase_with_descendants(cwd: Path, source: str, destination: str) -> str:
    await run_jj(["rebase", "-s", source, "--onto", destination], cwd)
    return f"Rebased {source} + descendants onto {destination}"


async def undo(cwd: Path) -> str:
    await run_jj(["undo"], cwd)
    return "Undid the last operation"


async def redo(cwd: Path) -> str:
    await run_jj(["redo"], cwd)
    return "Redid the last operation"


_OP_LOG_TEMPLATE = (
    'self.id().short(16) ++ "\\x1f" ++ '
    'self.time().start().ago() ++ "\\x1f" ++ '
    'coalesce(description.first_line(), "") ++ "\\n"'
)


async def op_log(cwd: Path) -> list[OpLogEntry]:
    stdout = await run_jj(["op", "log", "--no-graph", "-T", _OP_LOG_TEMPLATE], cwd)
    return parse_op_log(stdout)


async def op_restore(cwd: Path, op_id: str) -> str:
    await run_jj(["op", "restore", op_id], cwd)
    return f"Restored operation {op_id}"
