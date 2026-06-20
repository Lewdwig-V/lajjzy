from __future__ import annotations

from pathlib import Path

from textual import work
from textual.app import App, ComposeResult
from textual.reactive import reactive

from lajjzy.backend.jj import load_graph
from lajjzy.backend.types import GraphData, JjError


class LajjzyApp(App[None]):
    """Root application. Owns cross-cutting reactive state and key bindings."""

    CSS_PATH = "styles.tcss"

    BINDINGS = [
        ("j", "cursor_down", "Down"),
        ("down", "cursor_down", "Down"),
        ("k", "cursor_up", "Up"),
        ("up", "cursor_up", "Up"),
        ("g", "cursor_top", "Top"),
        ("G", "cursor_bottom", "Bottom"),
        ("R", "reload_graph", "Refresh"),
        ("q", "quit", "Quit"),
    ]

    graph: reactive[GraphData | None] = reactive(None)
    cursor: reactive[int] = reactive(0)
    error: reactive[str | None] = reactive(None)

    def __init__(self, repo_path: Path | None = None) -> None:
        super().__init__()
        self.repo_path = repo_path or Path.cwd()

    def compose(self) -> ComposeResult:
        from lajjzy.widgets import GraphView
        yield GraphView()

    def on_mount(self) -> None:
        self.reload()

    @work(group="load", exclusive=True)
    async def reload(self) -> None:
        try:
            new_graph = await load_graph(self.repo_path)
        except JjError as exc:
            self.error = str(exc)
            return
        self.error = None
        self.graph = new_graph
        # Land the cursor on the working copy if known, else the first node.
        if new_graph.working_copy_index is not None:
            self.cursor = new_graph.working_copy_index
        elif new_graph.node_indices:
            self.cursor = new_graph.node_indices[0]

    def selected_change_id(self) -> str | None:
        if self.graph is None:
            return None
        return self.graph.change_id_at(self.cursor)

    def _node_index_offset(self, delta: int) -> None:
        if self.graph is None or not self.graph.node_indices:
            return
        nodes = self.graph.node_indices
        try:
            pos = nodes.index(self.cursor)
        except ValueError:
            pos = 0
        pos = max(0, min(len(nodes) - 1, pos + delta))
        self.cursor = nodes[pos]

    def action_cursor_down(self) -> None:
        self._node_index_offset(1)

    def action_cursor_up(self) -> None:
        self._node_index_offset(-1)

    def action_cursor_top(self) -> None:
        if self.graph and self.graph.node_indices:
            self.cursor = self.graph.node_indices[0]

    def action_cursor_bottom(self) -> None:
        if self.graph and self.graph.node_indices:
            self.cursor = self.graph.node_indices[-1]

    def action_reload_graph(self) -> None:
        self.reload()


def main() -> None:
    LajjzyApp().run()
