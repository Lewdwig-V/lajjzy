from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
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


@dataclass
class FileChange:
    path: str
    status: FileStatus


@dataclass
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


@dataclass
class GraphLine:
    raw: str
    change_id: str | None
    glyph_prefix: str


@dataclass
class DiffLine:
    kind: Literal["context", "add", "remove"]
    text: str


@dataclass
class DiffHunk:
    header: str
    lines: list[DiffLine]


@dataclass
class FileDiff:
    path: str
    hunks: list[DiffHunk]


@dataclass
class GraphData:
    lines: list[GraphLine]
    details: dict[str, ChangeDetail]
    working_copy_index: int | None
    op_id: str
    node_indices: list[int] = field(default_factory=list)

    def __post_init__(self) -> None:
        # Always derive node_indices from lines — never trust a caller-supplied value.
        self.node_indices = [i for i, line in enumerate(self.lines) if line.change_id is not None]
        if self.working_copy_index is not None:
            if (
                self.working_copy_index >= len(self.lines)
                or self.lines[self.working_copy_index].change_id is None
            ):
                raise ValueError(f"working_copy_index {self.working_copy_index} invalid")

    def change_id_at(self, index: int) -> str | None:
        if 0 <= index < len(self.lines):
            return self.lines[index].change_id
        return None
