from __future__ import annotations

from typing import TYPE_CHECKING, cast

from rich.text import Text
from textual.widget import Widget

if TYPE_CHECKING:
    from lajjzy.app import LajjzyApp


class StatusBar(Widget):
    """Priority-ordered status line.

    Shows app.error (bold red) when set — this covers both hard errors and
    ephemeral prompts such as rebase mode instructions, which are surfaced via
    app.error rather than a separate rendering path.  Otherwise renders
    selected-change metadata: change id, author, timestamp, bookmarks (if any),
    and a CONFLICT marker when has_conflict is True.
    """

    def on_mount(self) -> None:
        def _refresh(_: object) -> None:
            self.refresh()

        self.watch(self.app, "error", _refresh)
        self.watch(self.app, "cursor", _refresh)
        self.watch(self.app, "graph", _refresh)

    def render(self) -> Text:
        app = cast("LajjzyApp", self.app)
        if app.error:
            return Text(app.error, style="bold red")
        change_id = app.selected_change_id()
        graph = app.graph
        if change_id is None or graph is None:
            return Text("")
        d = graph.details.get(change_id)
        if d is None:
            return Text("")
        parts = [change_id, d.author, d.timestamp]
        if d.bookmarks:
            parts.append("bookmarks: " + ", ".join(d.bookmarks))
        if d.has_conflict:
            parts.append("CONFLICT")
        return Text("  |  ".join(parts))
