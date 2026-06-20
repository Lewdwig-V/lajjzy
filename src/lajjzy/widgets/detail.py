from __future__ import annotations

from rich.text import Text
from textual.reactive import reactive
from textual.widget import Widget

from lajjzy.backend.types import FileDiff


class DetailPanel(Widget):
    can_focus = True

    # Focus-scoped: these fire ONLY when the DetailPanel has focus.
    BINDINGS = [
        ("j", "file_down", "Next file"),
        ("down", "file_down", "Next file"),
        ("k", "file_up", "Prev file"),
        ("up", "file_up", "Prev file"),
        ("enter", "open_file", "Open diff"),
        ("escape", "back", "Back"),
    ]

    file_cursor: reactive[int] = reactive(0)
    mode: reactive[str] = reactive("files")  # "files" | "diff"

    def __init__(self) -> None:
        super().__init__()
        self.diff: list[FileDiff] = []

    def on_mount(self) -> None:
        self.watch(self.app, "graph", lambda _: self._on_selection_change())
        self.watch(self.app, "cursor", lambda _: self._on_selection_change())

    def _on_selection_change(self) -> None:
        self.file_cursor = 0
        self.mode = "files"
        self.diff = []
        self.refresh()

    def current_files(self) -> list:
        change_id = self.app.selected_change_id()
        graph = self.app.graph
        if change_id is None or graph is None:
            return []
        detail = graph.details.get(change_id)
        return detail.files if detail else []

    def action_file_down(self) -> None:
        if self.mode != "files":
            return
        files = self.current_files()
        if files:
            self.file_cursor = min(len(files) - 1, self.file_cursor + 1)
            self.refresh()

    def action_file_up(self) -> None:
        if self.mode != "files":
            return
        self.file_cursor = max(0, self.file_cursor - 1)
        self.refresh()

    def action_open_file(self) -> None:
        if self.mode != "files":
            return
        files = self.current_files()
        if files:
            self.app.open_diff(files[self.file_cursor].path)

    def action_back(self) -> None:
        if self.mode == "diff":
            self.mode = "files"
            self.refresh()

    def render(self) -> Text:
        if self.mode == "diff":
            return self._render_diff()
        return self._render_files()

    def _render_files(self) -> Text:
        text = Text()
        files = self.current_files()
        if not files:
            return Text("(no file changes)", style="dim")
        for i, fc in enumerate(files):
            style = "reverse" if i == self.file_cursor else ""
            text.append(f"{fc.status.value} {fc.path}\n", style=style)
        return text

    def _render_diff(self) -> Text:
        text = Text()
        for fd in self.diff:
            text.append(f"{fd.path}\n", style="bold")
            for hunk in fd.hunks:
                text.append(hunk.header + "\n", style="cyan")
                for ln in hunk.lines:
                    style = {"add": "green", "remove": "red"}.get(ln.kind, "")
                    sign = {"add": "+", "remove": "-"}.get(ln.kind, " ")
                    text.append(f"{sign}{ln.text}\n", style=style)
        return text
