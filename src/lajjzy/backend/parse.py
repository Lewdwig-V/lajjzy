from __future__ import annotations

from lajjzy.backend.types import (
    ChangeDetail, DiffHunk, DiffLine, FileDiff, FileChange, FileStatus, GraphData, GraphLine,
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

    for raw in output.split("\n"):
        if not raw:
            continue
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
