from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from functools import cached_property
from typing import ClassVar, Literal


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


HunkResolutionValue = Literal["none", "accept_left", "accept_right"]


class HunkResolution:
    """Sentinel constants for per-hunk resolution choices in the conflict view.

    Kept as plain class attributes (not an Enum) so widgets can pass them as
    plain values without importing the enum wrapper; matches how the Rust
    prototype modelled it as a plain enum we serialize to a label.
    """

    NONE: ClassVar[HunkResolutionValue] = "none"  # undecided
    ACCEPT_LEFT: ClassVar[HunkResolutionValue] = "accept_left"
    ACCEPT_RIGHT: ClassVar[HunkResolutionValue] = "accept_right"


@dataclass(frozen=True, slots=True)
class ResolvedRegion:
    """Non-conflicting content of a conflicted file (text passed through as-is)."""

    text: str


@dataclass(frozen=True, slots=True)
class ConflictHunk:
    """A three-way conflict hunk within a conflicted file."""

    base: str
    left: str  # ours
    right: str  # theirs


# One region of a conflicted file: either non-conflicting passthrough content
# (``ResolvedRegion``) or a three-way conflict (``ConflictHunk``).  Modelled as
# a type union so illegal field combinations are unrepresentable by
# construction — there is no tag to validate and no way to set conflict fields
# on resolved content (or vice versa).
ConflictRegion = ResolvedRegion | ConflictHunk


@dataclass(frozen=True, slots=True)
class ConflictData:
    regions: list[ConflictRegion]


@dataclass(frozen=True, slots=True)
class FileRef:
    """A reference to a selected file for split / partial squash.

    Phase-1 operates at **file granularity**: the whole file is selected
    whenever it appears in the list.  This is the current contract.
    Hunk-granular selection will reintroduce a richer type (carrying a hunk
    index) once jj exposes a stable non-interactive CLI flag for it — jj 0.42.0
    has no such flag, so advertising hunk precision here would be dishonest.
    """

    path: str
