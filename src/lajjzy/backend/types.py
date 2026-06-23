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


@dataclass(frozen=True, slots=True)
class OpLogEntry:
    op_id: str
    timestamp: str
    description: str


@dataclass(frozen=True, slots=True)
class Bookmark:
    name: str
    change_id: str
    change_description: str


@dataclass(frozen=True, slots=True)
class CompletionItem:
    insert_text: str
    display_text: str


class HunkResolution:
    """Sentinel constants for per-hunk resolution choices in the conflict view.

    Kept as plain class attributes (not an Enum) so widgets can pass them as
    plain values without importing the enum wrapper; matches how the Rust
    prototype modelled it as a plain enum we serialize to a label.
    """

    NONE = "none"  # undecided
    ACCEPT_LEFT = "accept_left"
    ACCEPT_RIGHT = "accept_right"


@dataclass(frozen=True, slots=True)
class ConflictRegion:
    """One region of a conflicted file. Either non-conflicting content
    (``kind == "resolved"``) or a three-way conflict hunk
    (``kind == "conflict"``). Use the ``resolved(...)`` / ``conflict(...)``
    classmethods to construct — they set ``kind`` and the side fields."""

    kind: Literal["resolved", "conflict"]
    text: str = ""  # for resolved
    base: str = ""  # for conflict
    left: str = ""  # for conflict (ours)
    right: str = ""  # for conflict (theirs)

    @classmethod
    def resolved(cls, text: str) -> ConflictRegion:
        return cls(kind="resolved", text=text)

    @classmethod
    def conflict(cls, base: str, left: str, right: str) -> ConflictRegion:
        return cls(kind="conflict", base=base, left=left, right=right)


@dataclass(frozen=True, slots=True)
class ConflictData:
    regions: list[ConflictRegion]


@dataclass(frozen=True, slots=True)
class HunkRef:
    """A reference to a selected hunk for split / partial squash.

    ``hunk_idx`` is the 0-based index of the hunk within the file's diff.
    Phase-1 implementation operates at file granularity — the whole file is
    selected whenever any of its hunks appear in the list.  Hunk-granular
    selection requires a stable non-interactive jj CLI flag not yet available
    in 0.42.0.
    """

    path: str
    hunk_idx: int
