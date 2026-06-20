from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from functools import cached_property
from typing import Literal


class JjError(Exception):
    """Raised when a jj operation fails. Callers catch and surface via App.error."""


class FileStatus(Enum):
    ADDED = "A"
    MODIFIED = "M"
    DELETED = "D"
    RENAMED = "R"
    CONFLICTED = "C"
    UNKNOWN = "?"


@dataclass(frozen=True, slots=True)
class FileChange:
    path: str
    status: FileStatus


@dataclass(frozen=True, slots=True)
class ChangeDetail:
    commit_id: str
    author: str
    email: str
    # Holds jj's RELATIVE time string from committer.timestamp().ago()
    # (e.g. "2 hours ago") — not an absolute timestamp; do not parse as datetime.
    timestamp: str
    description: str
    bookmarks: list[str]
    is_empty: bool
    has_conflict: bool
    files: list[FileChange]
    parents: list[str]


@dataclass(frozen=True, slots=True)
class GraphLine:
    raw: str
    change_id: str | None
    glyph_prefix: str


@dataclass(frozen=True, slots=True)
class DiffLine:
    kind: Literal["context", "add", "remove"]
    text: str


@dataclass(frozen=True, slots=True)
class DiffHunk:
    header: str
    lines: list[DiffLine]


@dataclass(frozen=True, slots=True)
class FileDiff:
    path: str
    hunks: list[DiffHunk]


@dataclass(frozen=True)
class GraphData:
    lines: list[GraphLine]
    details: dict[str, ChangeDetail]
    working_copy_index: int | None
    op_id: str

    def __post_init__(self) -> None:
        line_ids = {line.change_id for line in self.lines if line.change_id is not None}
        if line_ids != set(self.details):
            raise ValueError(
                f"GraphData details/lines change-ID mismatch: "
                f"lines={sorted(line_ids)} details={sorted(self.details)}"
            )
        wci = self.working_copy_index
        if wci is not None:
            if not (0 <= wci < len(self.lines)) or self.lines[wci].change_id is None:
                raise ValueError(f"working_copy_index {wci} is not a valid node line")

    @cached_property
    def node_indices(self) -> list[int]:
        return [i for i, line in enumerate(self.lines) if line.change_id is not None]

    def change_id_at(self, index: int) -> str | None:
        if 0 <= index < len(self.lines):
            return self.lines[index].change_id
        return None
