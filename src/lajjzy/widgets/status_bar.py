from __future__ import annotations

from rich.text import Text
from textual.widget import Widget


class StatusBar(Widget):
    """Priority-ordered status line: error > rebase prompt > change metadata."""

    def on_mount(self) -> None:
        self.watch(self.app, "error", lambda _: self.refresh())
        self.watch(self.app, "cursor", lambda _: self.refresh())
        self.watch(self.app, "graph", lambda _: self.refresh())

    def render(self) -> Text:
        if self.app.error:
            return Text(self.app.error, style="bold red")
        change_id = self.app.selected_change_id()
        graph = self.app.graph
        if change_id is None or graph is None:
            return Text("")
        d = graph.details.get(change_id)
        if d is None:
            return Text("")
        parts = [change_id, d.author, d.timestamp]
        if d.bookmarks:
            parts.append("bookmarks: " + ", ".join(d.bookmarks))
        if d.conflict_count:
            parts.append("CONFLICT")
        return Text("  |  ".join(parts))
