from __future__ import annotations

import asyncio
from pathlib import Path

from lajjzy.backend.parse import parse_file_diffs, parse_graph_output
from lajjzy.backend.types import FileDiff, GraphData, JjError


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
    stdout = await run_jj(
        ["diff", "-r", change_id, "--git", "--color=never"], cwd
    )
    return parse_file_diffs(stdout)


async def load_graph(cwd: Path, revset: str | None = None) -> GraphData:
    op_id = await _op_id(cwd)
    args = ["log", "--summary", "--color=never", "-T", _GRAPH_TEMPLATE]
    if revset is not None:
        args += ["-r", revset]
    stdout = await run_jj(args, cwd)
    return parse_graph_output(stdout, op_id)
