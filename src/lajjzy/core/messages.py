from __future__ import annotations

from dataclasses import dataclass

from lajjzy.backend.types import GraphData

# ---------------------------------------------------------------------------
# User intents — produced by key bindings in the rendering backend.
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class CursorDown:
    pass


@dataclass(frozen=True)
class CursorUp:
    pass


@dataclass(frozen=True)
class CursorTop:
    pass


@dataclass(frozen=True)
class CursorBottom:
    pass


@dataclass(frozen=True)
class ReloadRequested:
    pass


@dataclass(frozen=True)
class NewChange:
    pass


@dataclass(frozen=True)
class Abandon:
    pass


@dataclass(frozen=True)
class EditChange:
    pass


@dataclass(frozen=True)
class Squash:
    pass


@dataclass(frozen=True)
class DescribeRequested:
    pass


@dataclass(frozen=True)
class RebaseStart:
    descendants: bool


@dataclass(frozen=True)
class RebaseConfirm:
    pass


@dataclass(frozen=True)
class RebaseCancel:
    pass


# ---------------------------------------------------------------------------
# Effect results — produced by the backend when a Cmd finishes.
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class GraphLoaded:
    epoch: int
    graph: GraphData


@dataclass(frozen=True)
class GraphLoadFailed:
    error: str


@dataclass(frozen=True)
class DescribeReady:
    change_id: str
    text: str


@dataclass(frozen=True)
class DescribeAborted:
    error: str | None


@dataclass(frozen=True)
class MutationFailed:
    error: str


@dataclass(frozen=True)
class MutationCompleted:
    """A mutation op finished and its follow-up graph reload was attempted.

    ``message`` is the op's success text. ``graph`` is the reloaded graph (or
    None if the reload failed), and ``load_error`` carries the reload failure.
    ``epoch`` is the graph epoch assigned when the mutation launched, used to
    discard the reload if a newer graph-producing op has since superseded it.
    """

    epoch: int
    message: str
    graph: GraphData | None
    load_error: str | None


Msg = (
    CursorDown
    | CursorUp
    | CursorTop
    | CursorBottom
    | ReloadRequested
    | NewChange
    | Abandon
    | EditChange
    | Squash
    | DescribeRequested
    | RebaseStart
    | RebaseConfirm
    | RebaseCancel
    | GraphLoaded
    | GraphLoadFailed
    | DescribeReady
    | DescribeAborted
    | MutationFailed
    | MutationCompleted
)
