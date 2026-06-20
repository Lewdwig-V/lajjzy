from __future__ import annotations

from collections.abc import Awaitable, Callable
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
        ("tab", "focus_detail", "Detail"),
        ("n", "new", "New"),
        ("d", "abandon", "Abandon"),
        ("ctrl+e", "edit", "Edit @"),
    ]

    graph: reactive[GraphData | None] = reactive(None)
    cursor: reactive[int] = reactive(0)
    error: reactive[str | None] = reactive(None)

    def __init__(self, repo_path: Path | None = None) -> None:
        super().__init__()
        self.repo_path = repo_path or Path.cwd()

    def compose(self) -> ComposeResult:
        from lajjzy.widgets import DetailPanel, GraphView

        yield GraphView()
        yield DetailPanel()

    def on_mount(self) -> None:
        from lajjzy.widgets import GraphView

        self.query_one(GraphView).focus()
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

    def action_focus_detail(self) -> None:
        from lajjzy.widgets import DetailPanel

        self.query_one(DetailPanel).focus()

    @work(group="mutation", exclusive=True)
    async def _mutate(self, op: Callable[[], Awaitable[str]]) -> None:
        try:
            message = await op()
        except JjError as exc:
            self.error = str(exc)
            return
        self.error = message
        # Reload synchronously inside this worker so the graph reflects the result.
        try:
            self.graph = await load_graph(self.repo_path)
        except JjError as exc:
            self.error = str(exc)
            return
        if self.graph.working_copy_index is not None:
            self.cursor = self.graph.working_copy_index

    def action_new(self) -> None:
        from lajjzy.backend.jj import new_change

        target = self.selected_change_id()
        if target is None:
            return
        self._mutate(lambda: new_change(self.repo_path, target))

    def action_abandon(self) -> None:
        from lajjzy.backend.jj import abandon

        target = self.selected_change_id()
        if target is None:
            return
        self._mutate(lambda: abandon(self.repo_path, target))

    def action_edit(self) -> None:
        from lajjzy.backend.jj import edit_change

        target = self.selected_change_id()
        if target is None:
            return
        self._mutate(lambda: edit_change(self.repo_path, target))

    async def ensure_working_copy(self, change_id: str) -> bool:
        """Working-copy gate: make `change_id` the @ commit before any
        filesystem-touching op. Returns True if @ is (now) the target.
        Used by deferred hunk-picker / conflict features."""
        from lajjzy.backend.jj import edit_change

        if self.graph and self.graph.working_copy_index is not None:
            current = self.graph.lines[self.graph.working_copy_index].change_id
            if current == change_id:
                return True
        try:
            await edit_change(self.repo_path, change_id)
        except JjError as exc:
            self.error = str(exc)
            return False
        return True

    @work(group="diff", exclusive=True)
    async def open_diff(self, path: str) -> None:
        from lajjzy.backend.jj import change_diff
        from lajjzy.widgets import DetailPanel

        change_id = self.selected_change_id()
        if change_id is None:
            return
        try:
            all_files = await change_diff(self.repo_path, change_id)
        except JjError as exc:
            self.error = str(exc)
            return
        panel = self.query_one(DetailPanel)
        panel.diff = [fd for fd in all_files if fd.path == path] or all_files
        panel.mode = "diff"
        panel.refresh()


def main() -> None:
    LajjzyApp().run()
