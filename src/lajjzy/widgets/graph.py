from __future__ import annotations

from typing import TYPE_CHECKING, cast

from rich.text import Text
from textual.widget import Widget

if TYPE_CHECKING:
    from lajjzy.app import LajjzyApp


class GraphView(Widget):
    """Renders the change graph; highlights the cursor line."""

    can_focus = True

    def on_mount(self) -> None:
        def _refresh(_: object) -> None:
            self.refresh()

        self.watch(self.app, "graph", _refresh)
        self.watch(self.app, "cursor", _refresh)

    def render(self) -> Text:
        app = cast("LajjzyApp", self.app)
        graph = app.graph
        if graph is None:
            return Text("loading…")
        text = Text()
        for i, line in enumerate(graph.lines):
            style = "reverse" if i == app.cursor else ""
            text.append(line.raw + "\n", style=style)
        return text
