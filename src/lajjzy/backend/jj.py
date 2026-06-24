from __future__ import annotations

import asyncio
from pathlib import Path

from lajjzy.backend.parse import (
    parse_bookmarks,
    parse_conflict_data,
    parse_file_diffs,
    parse_graph_output,
    parse_op_log,
)
from lajjzy.backend.types import (
    Bookmark,
    ConflictData,
    FileDiff,
    GraphData,
    HunkRef,
    HunkResolution,
    HunkResolutionValue,
    JjError,
    OpLogEntry,
)


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
    try:
        return parse_op_log(stdout)
    except ValueError as e:
        raise JjError(str(e)) from e


async def op_restore(cwd: Path, op_id: str) -> str:
    await run_jj(["op", "restore", op_id], cwd)
    return f"Restored operation {op_id}"


# In jj 0.42.0, `jj bookmark list -T` exposes a CommitRef type where fields are
# methods invoked on `self`, not bare keywords.  The template therefore uses
# `self.name()`, `self.normal_target().change_id().short()` and
# `self.normal_target().description().first_line()` — not the bare `name` /
# `change_id.short()` / `description.first_line()` that the brief assumed.
# Field order (name \x1f change_id \x1f description) is preserved so
# `parse_bookmarks` in parse.py remains correct.
_BOOKMARK_TEMPLATE = (
    'self.name() ++ "\\x1f" ++ self.normal_target().change_id().short() ++ "\\x1f" ++ '
    'coalesce(self.normal_target().description().first_line(), "") ++ "\\n"'
)


async def load_bookmarks(cwd: Path) -> list[Bookmark]:
    stdout = await run_jj(["bookmark", "list", "-T", _BOOKMARK_TEMPLATE, "--color=never"], cwd)
    try:
        return parse_bookmarks(stdout)
    except ValueError as e:
        raise JjError(str(e)) from e


async def bookmark_set(cwd: Path, change_id: str, name: str) -> str:
    await run_jj(["bookmark", "set", "-r", change_id, name], cwd)
    return f"Set bookmark {name} on {change_id}"


async def bookmark_delete(cwd: Path, name: str) -> str:
    await run_jj(["bookmark", "delete", name], cwd)
    return f"Deleted bookmark {name}"


async def bookmark_move(cwd: Path, name: str, dest_change_id: str) -> str:
    # --allow-backwards permits moving a bookmark to an ancestor (older) commit,
    # which jj refuses by default.  A TUI move operation should not impose
    # directionality constraints — that is the caller's responsibility.
    await run_jj(["bookmark", "set", "--allow-backwards", "-r", dest_change_id, name], cwd)
    return f"Moved bookmark {name} to {dest_change_id}"


async def conflict_data(cwd: Path, path: str) -> ConflictData:
    """Read a conflicted file's raw content (with jj conflict markers) and
    parse it into ConflictData. Works on the working copy (``@``).

    Forces ``ui.conflict-marker-style=git`` so the output uses the
    traditional 3-way markers (``<<<<<<<`` / ``|||||||`` / ``=======`` /
    ``>>>>>>>``) that ``parse_conflict_data`` understands.  jj 0.42.0
    defaults to a diff-based format that is structurally incompatible with
    the parser; the git style is the stable, machine-parseable subset.
    """
    stdout = await run_jj(
        ["file", "show", "-r", "@", "--config", "ui.conflict-marker-style=git", path],
        cwd,
    )
    return parse_conflict_data(stdout)


def _build_resolved_content(data: ConflictData, resolutions: list[HunkResolutionValue]) -> str:
    """Apply per-hunk resolution choices to produce the final file content.

    ``resolutions`` is one entry per conflict region (in order), each being a
    ``HunkResolution`` constant.  ``HunkResolution.NONE`` is treated as
    ``ACCEPT_LEFT`` (the widget must not let users apply with NONE set, but we
    default defensively).
    """
    n_conflicts = sum(1 for r in data.regions if r.kind == "conflict")
    if n_conflicts > len(resolutions):
        raise JjError(
            f"resolve: {n_conflicts} conflict region(s) but only {len(resolutions)} resolution(s) provided"
        )
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


async def resolve(cwd: Path, path: str, resolutions: list[HunkResolutionValue]) -> str:
    """Write the resolved file content to the working copy.

    Caller must ensure ``@`` is the conflicted change.  Does NOT mark the
    conflict resolved in jj's internal state — that happens automatically when
    the file no longer contains conflict markers.
    """
    data = await conflict_data(cwd, path)
    resolved = _build_resolved_content(data, resolutions)
    (cwd / path).write_text(resolved)
    return f"Resolved {path}"


async def split(cwd: Path, source: str, hunks: list[HunkRef]) -> str:
    """Non-interactively split ``source`` by file.

    Runs ``jj split -r <source> -m "" <paths...>``.  Empirically verified
    behaviour in jj 0.42.0:

    * The **selected** files (``<paths>``) are placed in the **first/parent**
      commit, which **retains the source's original change-id** and receives
      the empty description supplied via ``-m ""``.
    * The **unselected** files continue in a **new child** commit that becomes
      the working copy (``@``) and keeps the source's original description.

    ``hunk_idx`` fields are accepted but only the path is used in phase 1
    (file-granularity limitation — hunk-granular split needs a stable
    non-interactive jj CLI flag not available in 0.42.0).

    Raises ``JjError`` if ``hunks`` is empty or the jj command fails.
    """
    paths = sorted({h.path for h in hunks})
    if not paths:
        raise JjError("split requires at least one selected hunk")
    # -m "" suppresses the editor prompt for the selected-changes description;
    # the remaining changes keep the original description automatically.
    await run_jj(["split", "-r", source, "-m", "", *paths], cwd)
    return f"Split {len(paths)} file(s) out of {source}"


async def squash_partial(cwd: Path, source: str, hunks: list[HunkRef]) -> str:
    """Move selected files' changes from ``source`` into its parent.

    Runs ``jj squash -r <source> --use-destination-message <paths...>``.
    Only paths extracted from ``hunks`` are moved; other files in ``source``
    remain.  ``hunk_idx`` fields are accepted but only the path is used in
    phase 1 (file-granularity limitation).

    The destination is always ``source``'s parent, parallel to the
    whole-change ``squash(cwd, change_id)`` which uses
    ``jj squash -r <id> --use-destination-message``.

    Raises ``JjError`` if ``hunks`` is empty or the jj command fails.
    """
    paths = sorted({h.path for h in hunks})
    if not paths:
        raise JjError("squash_partial requires at least one selected hunk")
    await run_jj(
        ["squash", "-r", source, "--use-destination-message", *paths],
        cwd,
    )
    return f"Squashed {len(paths)} file(s) from {source} into its parent"
