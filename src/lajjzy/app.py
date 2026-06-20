from __future__ import annotations

import os
import subprocess
import sys
import tempfile
from collections.abc import Awaitable, Callable
from pathlib import Path

from textual import work
from textual.app import App, ComposeResult
from textual.containers import Horizontal
from textual.reactive import reactive

from lajjzy.backend.jj import load_graph
from lajjzy.backend.types import GraphData, JjError
from lajjzy.invariants import InvariantError


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
        ("e", "describe", "Describe"),
        ("S", "squash", "Squash"),
        ("r", "rebase", "Rebase"),
        ("ctrl+r", "rebase_descendants", "Rebase+desc"),
        ("enter", "rebase_confirm", "Confirm rebase"),
        ("escape", "rebase_cancel", "Cancel"),
    ]

    graph: reactive[GraphData | None] = reactive(None)
    cursor: reactive[int] = reactive(0)
    error: reactive[str | None] = reactive(None)
    rebase_source: reactive[str | None] = reactive(None)
    rebase_descendants_flag: reactive[bool] = reactive(False)

    def __init__(self, repo_path: Path | None = None) -> None:
        super().__init__()
        self.repo_path = repo_path or Path.cwd()
        self.pending_mutation = False
        self._graph_epoch = 0
        self._invariant_error: InvariantError | None = None

    def _handle_exception(self, error: Exception) -> None:
        """Intercept InvariantError before Textual's default handler.

        Textual 8.x stores exceptions in `self._exception` and exits the app
        but does NOT re-raise them out of `run()`.  We capture any
        `InvariantError` (including ones wrapped in `WorkerFailed`) here so
        that `main()` can re-raise it after the app shuts down.
        """
        from textual.worker import WorkerFailed

        cause = error
        if isinstance(error, WorkerFailed) and isinstance(error.__cause__, InvariantError):
            cause = error.__cause__
        if isinstance(cause, InvariantError):
            self._invariant_error = cause
        super()._handle_exception(error)

    def compose(self) -> ComposeResult:
        from lajjzy.widgets import DetailPanel, GraphView, StatusBar

        with Horizontal(id="panes"):
            yield GraphView()
            yield DetailPanel()
        yield StatusBar()

    def on_mount(self) -> None:
        from lajjzy.widgets import GraphView

        self.query_one(GraphView).focus()
        self.reload()

    @work(group="load", exclusive=True)
    async def reload(self) -> None:
        self._graph_epoch += 1
        epoch = self._graph_epoch
        try:
            new_graph = await load_graph(self.repo_path)
        except JjError as exc:
            self.error = str(exc)
            return
        except InvariantError:
            raise
        except Exception as exc:
            self.error = f"Unexpected error: {exc}"
            return
        # Discard this result if a newer graph-producing op has superseded us.
        # Also do NOT clear error or touch state for a stale load.
        if epoch != self._graph_epoch:
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
        # self.cursor is an index into graph.lines (not an ordinal into node_indices);
        # navigation steps between node_indices entries, skipping connector lines.
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

    def _dispatch_mutation(self, op: Callable[[], Awaitable[str]]) -> None:
        """Synchronous gate: reject (not cancel) a second mutation while one is in flight.

        Textual processes each key action to completion before the next, so the
        check-and-set here is atomic with respect to other key events — no race.
        """
        if self.pending_mutation:
            self.error = "A mutation is already in progress"
            return
        self.pending_mutation = True
        self._run_mutation(op)

    @work(group="mutation")
    async def _run_mutation(self, op: Callable[[], Awaitable[str]]) -> None:
        try:
            try:
                message = await op()
            except JjError as exc:
                self.error = str(exc)
                return
            except InvariantError:
                raise
            except Exception as exc:
                self.error = f"Unexpected error: {exc}"
                return
            self.error = message
            # Reload synchronously inside this worker so the graph reflects the result.
            # Use the epoch guard so a racing manual reload cannot overwrite us, and
            # so we do not overwrite a reload that started after us.
            self._graph_epoch += 1
            epoch = self._graph_epoch
            try:
                new_graph = await load_graph(self.repo_path)
            except JjError as exc:
                self.error = str(exc)
                return
            except InvariantError:
                raise
            except Exception as exc:
                self.error = f"Unexpected error: {exc}"
                return
            if epoch != self._graph_epoch:
                return  # a newer graph-producing op superseded this load; discard
            self.graph = new_graph
            if self.graph.working_copy_index is not None:
                self.cursor = self.graph.working_copy_index
            elif self.graph.node_indices:
                self.cursor = self.graph.node_indices[0]
        finally:
            self.pending_mutation = False

    def action_new(self) -> None:
        from lajjzy.backend.jj import new_change

        target = self.selected_change_id()
        if target is None:
            self.error = "No change selected"
            return
        self._dispatch_mutation(lambda: new_change(self.repo_path, target))

    def action_abandon(self) -> None:
        from lajjzy.backend.jj import abandon

        target = self.selected_change_id()
        if target is None:
            self.error = "No change selected"
            return
        self._dispatch_mutation(lambda: abandon(self.repo_path, target))

    def action_edit(self) -> None:
        from lajjzy.backend.jj import edit_change

        target = self.selected_change_id()
        if target is None:
            self.error = "No change selected"
            return
        self._dispatch_mutation(lambda: edit_change(self.repo_path, target))

    def action_squash(self) -> None:
        from lajjzy.backend.jj import squash

        target = self.selected_change_id()
        if target is None:
            self.error = "No change selected"
            return
        self._dispatch_mutation(lambda: squash(self.repo_path, target))

    def action_describe(self) -> None:
        target = self.selected_change_id()
        if target is None or self.graph is None:
            self.error = "No change selected"
            return
        detail = self.graph.details.get(target)
        if detail is None:
            return
        seed = detail.description
        message = self._edit_message_in_editor(seed)
        if message is None:
            return  # user aborted / editor unavailable
        from lajjzy.backend.jj import describe

        self._dispatch_mutation(lambda: describe(self.repo_path, target, message))

    def action_rebase(self) -> None:
        self.rebase_source = self.selected_change_id()
        self.rebase_descendants_flag = False
        if self.rebase_source:
            self.error = "Rebase: pick a destination, Enter to confirm, Esc to cancel"
        else:
            self.error = "No change selected"

    def action_rebase_descendants(self) -> None:
        self.rebase_source = self.selected_change_id()
        self.rebase_descendants_flag = True
        if self.rebase_source:
            self.error = "Rebase +desc: pick a destination, Enter to confirm, Esc to cancel"
        else:
            self.error = "No change selected"

    def action_rebase_confirm(self) -> None:
        # No-op unless rebase mode is armed — Enter does exactly one thing.
        if self.rebase_source is None:
            return
        dest = self.selected_change_id()
        src = self.rebase_source
        descend = self.rebase_descendants_flag
        self.rebase_source = None
        if dest is None or dest == src:
            self.error = "Rebase cancelled (invalid destination)"
            return
        from lajjzy.backend.jj import rebase_single, rebase_with_descendants

        op = rebase_with_descendants if descend else rebase_single
        self._dispatch_mutation(lambda: op(self.repo_path, src, dest))

    def action_rebase_cancel(self) -> None:
        # No-op unless rebase mode is armed.
        if self.rebase_source is not None:
            self.rebase_source = None
            self.error = "Rebase cancelled"

    def _edit_message_in_editor(self, seed: str) -> str | None:
        editor = os.environ.get("EDITOR")
        if not editor:
            self.error = "No $EDITOR set"
            return None
        with tempfile.NamedTemporaryFile(
            "w+", suffix=".jjdescribe", delete=False, encoding="utf-8"
        ) as tf:
            tf.write(seed)
            path = tf.name
        try:
            try:
                with self.suspend():  # hand the terminal to $EDITOR
                    result = subprocess.run([*editor.split(), path], check=False)
                if result.returncode != 0:
                    self.error = f"Editor exited with code {result.returncode}"
                    return None
                with open(path, encoding="utf-8") as fh:
                    return fh.read().strip()
            except FileNotFoundError:
                self.error = f"Editor not found: {editor!r}"
                return None
            except OSError as exc:
                self.error = f"Editor error: {exc}"
                return None
        finally:
            try:
                os.unlink(path)
            except OSError:
                pass

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
        except InvariantError:
            raise
        except Exception as exc:
            self.error = f"Unexpected error: {exc}"
            return
        panel = self.query_one(DetailPanel)
        panel.diff = [fd for fd in all_files if fd.path == path] or all_files
        panel.mode = "diff"
        panel.refresh()


def main() -> None:
    app = LajjzyApp()
    try:
        app.run()
    except InvariantError as exc:
        # Crash policy: a broken internal model. Textual restores the terminal
        # on app teardown; surface the breach loudly and exit non-zero.
        print(f"lajjzy: internal invariant violated: {exc}", file=sys.stderr)
        print("This is a bug — please report it.", file=sys.stderr)
        sys.exit(70)
    # Re-raise any InvariantError captured via _handle_exception (raised from a
    # worker, where Textual swallows the exception and does not propagate it out
    # of run()).
    if app._invariant_error is not None:
        exc = app._invariant_error
        print(f"lajjzy: internal invariant violated: {exc}", file=sys.stderr)
        print("This is a bug — please report it.", file=sys.stderr)
        sys.exit(70)
