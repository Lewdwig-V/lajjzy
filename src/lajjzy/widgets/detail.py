from __future__ import annotations

from rich.text import Text
from textual.widget import Widget


class DetailPanel(Widget):
    """Shows the file list for the selected change. Diff drill-down added later."""

    def on_mount(self) -> None:
        self.watch(self.app, "graph", lambda _: self.refresh())
        self.watch(self.app, "cursor", lambda _: self.refresh())

    def render(self) -> Text:
        change_id = self.app.selected_change_id()
        graph = self.app.graph
        if change_id is None or graph is None:
            return Text("")
        detail = graph.details.get(change_id)
        if detail is None:
            return Text("")
        text = Text()
        text.append(f"{change_id}  {detail.description}\n\n", style="bold")
        if not detail.files:
            text.append("(no file changes)\n", style="dim")
        for fc in detail.files:
            text.append(f"{fc.status.value} {fc.path}\n")
        return text
