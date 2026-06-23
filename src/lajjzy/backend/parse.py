from __future__ import annotations

from typing import TypedDict

from lajjzy.backend.types import (
    ChangeDetail,
    DiffHunk,
    DiffLine,
    FileDiff,
    FileChange,
    FileStatus,
    GraphData,
    GraphLine,
    OpLogEntry,
)

UNIT_SEP = "\x1f"
RECORD_SEP = "\x1e"

_STATUS_MAP = {
    "A": FileStatus.ADDED,
    "M": FileStatus.MODIFIED,
    "D": FileStatus.DELETED,
    "R": FileStatus.RENAMED,
    "C": FileStatus.CONFLICTED,
}


class _PendingChange(TypedDict):
    commit_id: str
    author: str
    email: str
    timestamp: str
    description: str
    bookmarks: list[str]
    is_empty: bool
    has_conflict: bool
    parents: list[str]


class _PendingHunk(TypedDict):
    header: str
    lines: list[DiffLine]


class _PendingFile(TypedDict):
    path: str
    hunks: list[_PendingHunk]


def parse_file_line(line: str) -> FileChange | None:
    """Parse a `jj log --summary` file line like 'M path/to/file'.

    The line may be prefixed by graph-drawing characters (e.g. '│  A path'),
    so we skip to the first ASCII letter before checking the status code.
    """
    start = _first_alnum(line)
    stripped = line[start:]
    if len(stripped) < 2 or stripped[1] != " ":
        return None
    code = stripped[0]
    if code not in _STATUS_MAP:
        return None
    return FileChange(path=stripped[2:].strip(), status=_STATUS_MAP[code])


def _first_alnum(s: str) -> int:
    """Return index of first alphanumeric character; used to skip jj graph-drawing
    prefixes (e.g. '│  ') before the status code or change-id. Returns 0 if none."""
    for i, ch in enumerate(s):
        if ch.isalnum():
            return i
    return 0


def parse_graph_output(output: str, op_id: str) -> GraphData:
    lines: list[GraphLine] = []
    # Accumulate scalar fields per change before constructing ChangeDetail.
    pending: dict[str, _PendingChange] = {}
    files_by_change: dict[str, list[FileChange]] = {}
    working_copy_index: int | None = None
    current_change_id: str | None = None

    for raw in output.split("\n"):
        if not raw:
            continue
        sep = raw.find(UNIT_SEP)
        if sep != -1:
            display = raw[:sep]
            fields = raw[sep + 1 :].split(RECORD_SEP)
            # 11 fields must match the _GRAPH_TEMPLATE field order defined in jj.py.
            if len(fields) < 11:
                raise ValueError(f"Expected 11 metadata fields, got {len(fields)}: {fields!r}")
            change_id = fields[0]
            current_change_id = change_id
            if change_id in pending:
                raise ValueError(f"Duplicate short change ID {change_id!r} (truncation collision).")
            if fields[9]:  # working-copy marker "@"
                working_copy_index = len(lines)
            pending[change_id] = _PendingChange(
                commit_id=fields[1],
                author=fields[2],
                email=fields[3],
                timestamp=fields[4],
                description=fields[5],
                bookmarks=fields[6].split() if fields[6] else [],
                is_empty=fields[7] == "true",
                has_conflict=fields[8] == "true",
                parents=fields[10].split() if fields[10] else [],
            )
            files_by_change[change_id] = []
            glyph_end = _first_alnum(display)
            lines.append(
                GraphLine(
                    raw=display,
                    change_id=change_id,
                    glyph_prefix=display[:glyph_end],
                )
            )
            continue

        file_change = parse_file_line(raw)
        if file_change is not None and current_change_id is not None:
            files_by_change[current_change_id].append(file_change)
        else:
            # Connector / non-file line (graph art only): store as GraphLine with change_id=None.
            lines.append(GraphLine(raw=raw, change_id=None, glyph_prefix=raw))

    if output.strip() and not pending:
        raise ValueError("Parsed jj output but found zero change nodes; template may have changed.")

    details: dict[str, ChangeDetail] = {
        cid: ChangeDetail(**fields, files=files_by_change.get(cid, []))
        for cid, fields in pending.items()
    }

    return GraphData(
        lines=lines, details=details, working_copy_index=working_copy_index, op_id=op_id
    )


def parse_file_diffs(output: str) -> list[FileDiff]:
    # Accumulate mutable lists during parsing; construct frozen objects once at the end.
    # Each entry: {"path": str, "hunks": [{"header": str, "lines": [DiffLine]}]}
    pending_files: list[_PendingFile] = []
    current_file: _PendingFile | None = None
    current_hunk: _PendingHunk | None = None

    for line in output.splitlines():
        if line.startswith("diff --git "):
            # "diff --git a/<path> b/<path>" → take the b-side path.
            b = line.split(" b/", 1)
            path = b[1] if len(b) == 2 else line
            current_file = _PendingFile(path=path, hunks=[])
            pending_files.append(current_file)
            current_hunk = None
        elif line.startswith("@@"):
            current_hunk = _PendingHunk(header=line, lines=[])
            if current_file is not None:
                current_file["hunks"].append(current_hunk)
        elif current_hunk is not None:
            if line.startswith("+"):
                current_hunk["lines"].append(DiffLine(kind="add", text=line[1:]))
            elif line.startswith("-"):
                current_hunk["lines"].append(DiffLine(kind="remove", text=line[1:]))
            elif line.startswith(" "):
                current_hunk["lines"].append(DiffLine(kind="context", text=line[1:]))
            # ignore "index", "---", "+++", "\ No newline" lines

    return [
        FileDiff(
            path=pf["path"],
            hunks=[DiffHunk(header=ph["header"], lines=ph["lines"]) for ph in pf["hunks"]],
        )
        for pf in pending_files
    ]


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
