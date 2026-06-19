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
