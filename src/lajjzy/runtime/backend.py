from __future__ import annotations

from collections.abc import Callable
from typing import Protocol

from lajjzy.core.commands import Cmd
from lajjzy.core.messages import Msg
from lajjzy.core.model import Model
from lajjzy.core.update import update


class Backend(Protocol):
    """The swappable rendering/effect backend — the seam between the pure core
    and the outside world.

    A backend is responsible for exactly two things: presenting a Model to the
    user, and executing the Cmds the core asks for (shelling out to jj, running
    $EDITOR) and dispatching the resulting Msgs back into the loop. Textual is
    one implementation (``lajjzy.app.LajjzyApp``); a headless backend over plain
    asyncio is just as valid, because neither the Model nor ``update`` knows or
    cares which one is driving them.
    """

    def present(self, model: Model) -> None:
        """Render ``model``. Called after every state transition."""
        ...

    def run_cmd(self, cmd: Cmd, dispatch: Callable[[Msg], None]) -> None:
        """Execute ``cmd``'s side effect, calling ``dispatch`` with each result Msg."""
        ...


class Runtime:
    """The MVU loop, renderer-agnostic.

    Owns the authoritative Model, feeds each Msg through the pure ``update``,
    hands the new Model to the backend to present, and asks the backend to run
    any commands. The only mutable state is ``self.model``; everything else is
    pure functions and the backend boundary.
    """

    def __init__(self, backend: Backend, model: Model | None = None) -> None:
        self.backend = backend
        self.model = model if model is not None else Model()

    def dispatch(self, msg: Msg) -> None:
        self.model, cmds = update(self.model, msg)
        self.backend.present(self.model)
        for cmd in cmds:
            self.backend.run_cmd(cmd, self.dispatch)
