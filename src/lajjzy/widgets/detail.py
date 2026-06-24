from __future__ import annotations

from typing import TYPE_CHECKING, cast

from rich.text import Text
from textual.widget import Widget

from lajjzy.backend.types import FileChange

if TYPE_CHECKING:
    from lajjzy.app import LajjzyApp


class DetailPanel(Widget):
    can_focus = True

    # Focus-scoped: these fire ONLY when the DetailPanel has focus. Each
    # dispatches a Msg; all state lives on Model.detail.
    BINDINGS = [
        ("j", "file_down", "Next file"),
        ("down", "file_down", "Next file"),
        ("k", "file_up", "Prev file"),
        ("up", "file_up", "Prev file"),
        ("enter", "open_file", "Open diff"),
        ("escape", "back", "Back"),
    ]

    def on_mount(self) -> None:
        # Re-render whenever the projected detail state or selection changes.
        def _refresh(_: object) -> None:
            self.refresh()

        self.watch(self.app, "detail", _refresh)
        self.watch(self.app, "graph", _refresh)
        self.watch(self.app, "cursor", _refresh)

    def _app(self) -> LajjzyApp:
        return cast("LajjzyApp", self.app)

    def current_files(self) -> list[FileChange]:
        app = self._app()
        change_id = app.selected_change_id()
        graph = app.graph
        if change_id is None or graph is None:
            return []
        detail = graph.details.get(change_id)
        return detail.files if detail else []

    def action_file_down(self) -> None:
        from lajjzy.core import DetailFileDown

        self._app().runtime.dispatch(DetailFileDown())

    def action_file_up(self) -> None:
        from lajjzy.core import DetailFileUp

        self._app().runtime.dispatch(DetailFileUp())

    def action_open_file(self) -> None:
        from lajjzy.core import DetailOpenFile

        self._app().runtime.dispatch(DetailOpenFile())

    def action_back(self) -> None:
        from lajjzy.core import DetailBack

        self._app().runtime.dispatch(DetailBack())

    def render(self) -> Text:
        detail = self._app().detail
        if detail.mode == "diff":
            return self._render_diff()
        return self._render_files()

    def _render_files(self) -> Text:
        files = self.current_files()
        if not files:
            return Text("(no file changes)", style="dim")
        cursor = self._app().detail.file_cursor
        text = Text()
        for i, fc in enumerate(files):
            style = "reverse" if i == cursor else ""
            text.append(f"{fc.status.value} {fc.path}\n", style=style)
        return text

    def _render_diff(self) -> Text:
        diff = self._app().detail.diff
        if diff is None:
            return Text("(loading diff…)", style="dim")
        if diff == []:
            return Text("(no changes)", style="dim")
        files = self.current_files()
        cursor = self._app().detail.file_cursor
        # Show the opened file's diff (preserves the prior single-file view).
        path = files[cursor].path if 0 <= cursor < len(files) else None
        shown = [fd for fd in diff if fd.path == path] or diff
        text = Text()
        for fd in shown:
            text.append(f"{fd.path}\n", style="bold")
            for hunk in fd.hunks:
                text.append(hunk.header + "\n", style="cyan")
                for ln in hunk.lines:
                    style = {"add": "green", "remove": "red"}.get(ln.kind, "")
                    sign = {"add": "+", "remove": "-"}.get(ln.kind, " ")
                    text.append(f"{sign}{ln.text}\n", style=style)
        return text
