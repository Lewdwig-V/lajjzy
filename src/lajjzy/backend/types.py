from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum


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
    timestamp: str
    description: str
    bookmarks: list[str]
    is_empty: bool
    conflict_count: int
    files: list[FileChange]
    parents: list[str]


@dataclass
class GraphLine:
    raw: str
    change_id: str | None
    glyph_prefix: str


@dataclass
class DiffLine:
    kind: str  # "context" | "add" | "remove"
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
        if not self.node_indices:
            self.node_indices = [
                i for i, line in enumerate(self.lines) if line.change_id is not None
            ]

    def change_id_at(self, index: int) -> str | None:
        if 0 <= index < len(self.lines):
            return self.lines[index].change_id
        return None
