from __future__ import annotations

from typing import TypedDict

from lajjzy.backend.types import (
    Bookmark,
    ChangeDetail,
    ConflictData,
    ConflictHunk,
    ConflictRegion,
    ResolvedRegion,
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

    Fields must match _OP_LOG_TEMPLATE field order in jj.py (id, timestamp, description).
    Raises ValueError on non-empty lines that don't have exactly 3 UNIT_SEP-delimited fields.
    """
    entries: list[OpLogEntry] = []
    for line in output.split("\n"):
        if not line:
            continue
        parts = line.split(UNIT_SEP)
        if len(parts) != 3:
            raise ValueError(f"op log line has {len(parts)} fields, expected 3: {line!r}")
        entries.append(OpLogEntry(op_id=parts[0], timestamp=parts[1], description=parts[2]))
    return entries


def parse_bookmarks(output: str) -> list[Bookmark]:
    """Parse `jj bookmark list -T <template>` output into Bookmark list.

    Fields must match _BOOKMARK_TEMPLATE field order in jj.py (name, change_id, description).
    Raises ValueError on non-empty lines that don't have exactly 3 UNIT_SEP-delimited fields.
    """
    bms: list[Bookmark] = []
    for line in output.split("\n"):
        if not line:
            continue
        parts = line.split(UNIT_SEP)
        if len(parts) != 3:
            raise ValueError(f"bookmark line has {len(parts)} fields, expected 3: {line!r}")
        bms.append(Bookmark(name=parts[0], change_id=parts[1], change_description=parts[2]))
    return bms


def _is_conflict_marker(stripped: str, prefix: str) -> bool:
    """True if a line (already rstripped of "\n") is the jj git-style conflict
    marker for ``prefix`` — either the bare 7-char prefix or the prefix followed
    by a space and metadata. Avoids misreading content lines like "<<<<<<<x"."""
    return stripped == prefix or stripped.startswith(prefix + " ")


def parse_conflict_data(output: str) -> ConflictData:
    """Parse a conflicted file's raw content into ConflictData.

    Handles jj's git-style conflict markers (``ui.conflict-marker-style = "git"``).
    Each marker begins with the canonical 7-char sequence but may be followed by
    extra metadata on the same line (e.g. ``<<<<<<< abc1234 "branch-name"``).
    The ``=======`` separator carries no extra metadata and is matched exactly.

    Format::

        <<<<<<< <optional metadata>
        <left (ours)>
        ||||||| <optional metadata>
        <base>
        =======
        <right (theirs)>
        >>>>>>> <optional metadata>

    Regions outside conflict hunks are non-conflicting (``resolved``).
    An empty side means that side deleted the region.
    """
    lines = output.splitlines(keepends=True)
    regions: list[ConflictRegion] = []
    i = 0
    pending_resolved: list[str] = []

    def flush_resolved() -> None:
        if pending_resolved:
            regions.append(ResolvedRegion(text="".join(pending_resolved)))
            pending_resolved.clear()

    while i < len(lines):
        stripped = lines[i].rstrip("\n")
        if _is_conflict_marker(stripped, "<<<<<<<"):
            flush_resolved()
            i += 1
            left: list[str] = []
            while i < len(lines) and not _is_conflict_marker(lines[i].rstrip("\n"), "|||||||"):
                left.append(lines[i])
                i += 1
            i += 1  # skip |||||||
            base: list[str] = []
            while i < len(lines) and not _is_conflict_marker(lines[i].rstrip("\n"), "======="):
                base.append(lines[i])
                i += 1
            i += 1  # skip =======
            right: list[str] = []
            while i < len(lines) and not _is_conflict_marker(lines[i].rstrip("\n"), ">>>>>>>"):
                right.append(lines[i])
                i += 1
            i += 1  # skip >>>>>>>
            regions.append(
                ConflictHunk(
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
