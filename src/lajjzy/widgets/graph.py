from __future__ import annotations

from rich.text import Text
from textual.widget import Widget


class GraphView(Widget):
    """Renders the change graph; highlights the cursor line."""

    can_focus = True

    def on_mount(self) -> None:
        self.watch(self.app, "graph", lambda _: self.refresh())
        self.watch(self.app, "cursor", lambda _: self.refresh())

    def render(self) -> Text:
        graph = self.app.graph
        if graph is None:
            return Text("loading…")
        text = Text()
        for i, line in enumerate(graph.lines):
            style = "reverse" if i == self.app.cursor else ""
            text.append(line.raw + "\n", style=style)
        return text
