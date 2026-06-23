from __future__ import annotations

from dataclasses import dataclass
from typing import Any

# Commands are *descriptions* of side effects, returned by ``update`` alongside
# the next Model. The core never performs them; a rendering backend interprets
# each Cmd, runs the real work (jj subprocess, $EDITOR), and dispatches the
# resulting Msg back into the loop. This keeps ``update`` pure.


@dataclass(frozen=True)
class LoadGraph:
    """Reload the change graph. On completion dispatch GraphLoaded(epoch, graph)
    or GraphLoadFailed(error)."""

    epoch: int
    revset: str | None = None


@dataclass(frozen=True)
class RunMutation:
    """Run a write op (identified by ``kind`` + ``args``), then reload the graph.

    On completion dispatch MutationCompleted(epoch, message, graph, load_error)
    or MutationFailed(error). ``kind`` maps to a function in the jj facade; the
    core stays free of any jj import.
    """

    epoch: int
    kind: str
    args: tuple[Any, ...]


@dataclass(frozen=True)
class EditMessage:
    """Open $EDITOR seeded with ``seed`` to compose a description. On completion
    dispatch DescribeReady(change_id, text) or DescribeAborted(error)."""

    change_id: str
    seed: str


@dataclass(frozen=True)
class LoadOpLog:
    """Load the operation log. On completion dispatch OpLogLoaded(entries)
    or OpLogLoadFailed(error)."""

    pass


@dataclass(frozen=True)
class LoadBookmarks:
    """Load the list of bookmarks. On completion dispatch BookmarksLoaded(bookmarks)
    or BookmarksLoadFailed(error)."""

    pass


@dataclass(frozen=True)
class LoadConflictData:
    """Load conflict data for a file. On completion dispatch ConflictDataLoaded(data)
    or ConflictDataLoadFailed(error)."""

    path: str


Cmd = LoadGraph | RunMutation | EditMessage | LoadOpLog | LoadBookmarks | LoadConflictData
