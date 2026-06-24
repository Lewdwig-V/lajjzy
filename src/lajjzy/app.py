from __future__ import annotations

import os
import subprocess
import sys
import tempfile
from collections.abc import Awaitable, Callable
from pathlib import Path
from typing import Any, assert_never

from textual import work
from textual.app import App, ComposeResult
from textual.containers import Horizontal
from textual.reactive import reactive

import lajjzy.backend.jj as jj
from lajjzy.backend.types import Bookmark, ConflictData, GraphData, JjError, OpLogEntry
from lajjzy.core import (
    Abandon,
    Cmd,
    CursorBottom,
    CursorDown,
    CursorTop,
    CursorUp,
    DescribeAborted,
    DescribeReady,
    DescribeRequested,
    EditChange,
    EditMessage,
    GraphLoaded,
    GraphLoadFailed,
    LoadBookmarks,
    LoadConflictData,
    LoadGraph,
    LoadOpLog,
    Modal,
    Model,
    Msg,
    MutationCompleted,
    MutationFailed,
    NewChange,
    OpenBookmarkPicker,
    OpenBookmarkSet,
    OpenOmnibar,
    OpenOpLog,
    RebaseCancel,
    RebaseConfirm,
    RebaseStart,
    Redo,
    ReloadRequested,
    RunMutation,
    Split,
    Squash,
    SquashPartial,
    Undo,
    selected_change_id,
)
from lajjzy.core.messages import (
    BookmarksLoadFailed,
    BookmarksLoaded,
    ConflictDataLoadFailed,
    ConflictDataLoaded,
    OpLogLoadFailed,
    OpLogLoaded,
)
from lajjzy.invariants import InvariantError
from lajjzy.runtime import Runtime
from textual.worker import WorkerFailed

# Maps a RunMutation.kind to the jj-facade coroutine that performs it. Looked up
# through the `jj` module at call time so tests can monkeypatch the facade.
_OPS: dict[str, Callable[[Path, tuple[Any, ...]], Awaitable[str]]] = {
    "new": lambda cwd, a: jj.new_change(cwd, *a),
    "abandon": lambda cwd, a: jj.abandon(cwd, *a),
    "edit": lambda cwd, a: jj.edit_change(cwd, *a),
    "squash": lambda cwd, a: jj.squash(cwd, *a),
    "describe": lambda cwd, a: jj.describe(cwd, *a),
    "rebase": lambda cwd, a: jj.rebase_single(cwd, *a),
    "rebase_descendants": lambda cwd, a: jj.rebase_with_descendants(cwd, *a),
    "undo": lambda cwd, a: jj.undo(cwd, *a),
    "redo": lambda cwd, a: jj.redo(cwd, *a),
    "bookmark_set": lambda cwd, a: jj.bookmark_set(cwd, *a),
    "bookmark_delete": lambda cwd, a: jj.bookmark_delete(cwd, *a),
    "bookmark_move": lambda cwd, a: jj.bookmark_move(cwd, *a),
    "op_restore": lambda cwd, a: jj.op_restore(cwd, *a),
    "resolve": lambda cwd, a: jj.resolve(cwd, *a),
    "split": lambda cwd, a: jj.split(cwd, *a),
    "squash_partial": lambda cwd, a: jj.squash_partial(cwd, *a),
}


class LajjzyApp(App[None]):
    """Textual implementation of the rendering/effect Backend.

    The authoritative state lives in ``self.runtime.model`` (a pure ``Model``);
    this class only *projects* that model onto reactive attributes the widgets
    watch, and *runs* the effects the core requests on Textual's worker lanes.
    Key bindings translate to ``Msg`` values dispatched through the runtime — no
    state transition logic lives here.
    """

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
        ("u", "undo", "Undo"),
        ("U", "redo", "Redo"),
        ("/", "open_omnibar", "Omnibar"),
        ("B", "open_bookmark_set", "Set bookmark"),
        ("b", "open_bookmark_picker", "Bookmarks"),
        ("o", "open_op_log", "Op log"),
        ("s", "split", "Split"),
        ("ctrl+s", "squash_partial", "Squash partial"),
    ]

    # Reactives are a *projection* of the Model for the widget layer to watch;
    # they are written only by `present`, never treated as the source of truth.
    graph: reactive[GraphData | None] = reactive(None)
    cursor: reactive[int] = reactive(0)
    error: reactive[str | None] = reactive(None)
    rebase_source: reactive[str | None] = reactive(None)
    modal: reactive[Modal | None] = reactive(None)
    op_log_entries: reactive[list[OpLogEntry] | None] = reactive(None)
    bookmarks: reactive[list[Bookmark] | None] = reactive(None)
    revset: reactive[str | None] = reactive(None)
    conflict_data: reactive[ConflictData | None] = reactive(None)
    conflict_path: reactive[str | None] = reactive(None)

    def __init__(self, repo_path: Path | None = None) -> None:
        super().__init__()
        self.repo_path = repo_path or Path.cwd()
        self.runtime = Runtime(self)
        # Plain mirror of model.pending_mutation (no widget watches it).
        self.pending_mutation = False
        # Captured when an InvariantError fires inside a @work worker (Textual
        # wraps it in WorkerFailed and swallows it); main() reads this to exit 70.
        self._invariant_error: InvariantError | None = None

    def _handle_exception(self, error: Exception) -> None:
        # Workers wrap raised exceptions in WorkerFailed; the original is on
        # .error, not __cause__. Capture invariant breaches so main() can exit
        # 70 per the crash policy, then defer to Textual to tear the app down.
        actual = error.error if isinstance(error, WorkerFailed) else error
        if isinstance(actual, InvariantError):
            self._invariant_error = actual
        super()._handle_exception(error)

    # -- model accessors --------------------------------------------------

    @property
    def model(self) -> Model:
        return self.runtime.model

    def selected_change_id(self) -> str | None:
        return selected_change_id(self.model)

    # -- Backend.present: project Model onto the watched reactives --------

    def present(self, model: Model) -> None:
        self.graph = model.graph
        self.cursor = model.cursor
        self.error = model.error
        self.rebase_source = model.rebase_source
        self.pending_mutation = model.pending_mutation
        self.modal = model.modal
        self.op_log_entries = model.op_log_entries
        self.bookmarks = model.bookmarks
        self.revset = model.revset
        self.conflict_data = model.conflict_data
        self.conflict_path = model.conflict_path

    # -- Backend.run_cmd: interpret a Cmd on the right concurrency lane ----

    def run_cmd(self, cmd: Cmd, dispatch: Callable[[Msg], None]) -> None:
        if isinstance(cmd, LoadGraph):
            self._worker_load(cmd.epoch, cmd.revset)
        elif isinstance(cmd, RunMutation):
            self._worker_mutation(cmd.epoch, cmd.kind, cmd.args)
        elif isinstance(cmd, EditMessage):
            # $EDITOR needs the terminal synchronously (suspend), so this runs
            # inline rather than on a worker and dispatches its result at once.
            self._run_editor(cmd.change_id, cmd.seed)
        elif isinstance(cmd, LoadOpLog):
            self._worker_load_op_log()
        elif isinstance(cmd, LoadBookmarks):
            self._worker_load_bookmarks()
        elif isinstance(cmd, LoadConflictData):
            self._worker_load_conflict(cmd.path)
        else:
            assert_never(cmd)

    @work(group="load", exclusive=True)
    async def _worker_load(self, epoch: int, revset: str | None = None) -> None:
        # group="load", exclusive: a new reload cancels any in-flight reload.
        try:
            graph = await jj.load_graph(self.repo_path, revset)
        except JjError as exc:
            self.runtime.dispatch(GraphLoadFailed(str(exc)))
        except InvariantError:
            # Must propagate (crash policy), not become a GraphLoadFailed.
            raise
        except Exception as exc:
            self.runtime.dispatch(GraphLoadFailed(f"Unexpected error: {exc}"))
        else:
            self.runtime.dispatch(GraphLoaded(epoch, graph))

    @work(group="mutation")
    async def _worker_mutation(self, epoch: int, kind: str, args: tuple[Any, ...]) -> None:
        # group="mutation": op + follow-up reload run in one worker, so the graph
        # reflects the result. The single-mutation gate lives in `update` (the
        # pending_mutation flag), so this group need not be exclusive.
        op = _OPS.get(kind)
        if op is None:
            # An unwired kind is an internal model breach (core asked for an effect
            # the backend can't perform) — crash per the crash policy.
            raise InvariantError(f"RunMutation kind not wired in _OPS: {kind!r}")
        try:
            message = await op(self.repo_path, args)
        except JjError as exc:
            self.runtime.dispatch(MutationFailed(str(exc)))
            return
        except InvariantError:
            raise
        except Exception as exc:
            self.runtime.dispatch(MutationFailed(f"Unexpected error: {exc}"))
            return
        # Thread the active revset through the follow-up reload so a mutation done
        # while a filter is active reloads the filtered graph (not the full graph).
        current_revset = self.runtime.model.revset
        try:
            graph = await jj.load_graph(self.repo_path, current_revset)
        except JjError as exc:
            self.runtime.dispatch(MutationCompleted(epoch, message, None, str(exc)))
            return
        except InvariantError:
            raise
        except Exception as exc:
            self.runtime.dispatch(
                MutationCompleted(epoch, message, None, f"Unexpected error: {exc}")
            )
            return
        # For bookmark mutations, also refresh the bookmarks list so the
        # picker reflects the change in the same step as the graph reload.
        if kind in ("bookmark_set", "bookmark_delete", "bookmark_move"):
            try:
                bms = await jj.load_bookmarks(self.repo_path)
            except InvariantError:
                raise  # crash policy: an invariant breach must always propagate
            except Exception:
                bms = None  # non-fatal: graph reload is the primary result
            self.runtime.dispatch(MutationCompleted(epoch, message, graph, None, bookmarks=bms))
            return
        self.runtime.dispatch(MutationCompleted(epoch, message, graph, None))

    @work(group="oplog", exclusive=True)
    async def _worker_load_op_log(self) -> None:
        # group="oplog", exclusive: a new load cancels any in-flight op-log load.
        try:
            entries = await jj.op_log(self.repo_path)
        except JjError as exc:
            self.runtime.dispatch(OpLogLoadFailed(str(exc)))
        except InvariantError:
            raise
        except Exception as exc:
            self.runtime.dispatch(OpLogLoadFailed(f"Unexpected error: {exc}"))
        else:
            self.runtime.dispatch(OpLogLoaded(entries))

    @work(group="bookmarks", exclusive=True)
    async def _worker_load_bookmarks(self) -> None:
        # group="bookmarks", exclusive: a new load cancels any in-flight bookmark load.
        try:
            bookmarks = await jj.load_bookmarks(self.repo_path)
        except JjError as exc:
            self.runtime.dispatch(BookmarksLoadFailed(str(exc)))
        except InvariantError:
            raise
        except Exception as exc:
            self.runtime.dispatch(BookmarksLoadFailed(f"Unexpected error: {exc}"))
        else:
            self.runtime.dispatch(BookmarksLoaded(bookmarks))

    @work(group="conflict", exclusive=True)
    async def _worker_load_conflict(self, path: str) -> None:
        # group="conflict", exclusive: a new load cancels any in-flight conflict load.
        try:
            data = await jj.conflict_data(self.repo_path, path)
        except JjError as exc:
            self.runtime.dispatch(ConflictDataLoadFailed(str(exc)))
        except InvariantError:
            raise
        except Exception as exc:
            self.runtime.dispatch(ConflictDataLoadFailed(f"Unexpected error: {exc}"))
        else:
            self.runtime.dispatch(ConflictDataLoaded(data))

    def _run_editor(self, change_id: str, seed: str) -> None:
        text, err = self._edit_message_in_editor(seed)
        if text is not None:
            self.runtime.dispatch(DescribeReady(change_id, text))
        else:
            self.runtime.dispatch(DescribeAborted(err))

    # -- Textual lifecycle ------------------------------------------------

    def compose(self) -> ComposeResult:
        from lajjzy.widgets import (
            BookmarkInput,
            BookmarkPicker,
            ConflictView,
            DetailPanel,
            GraphView,
            HunkPicker,
            Omnibar,
            OpLog,
            StatusBar,
        )

        with Horizontal(id="panes"):
            yield GraphView()
            yield DetailPanel()
        yield StatusBar()
        # Modals are always mounted but hidden; visibility follows self.modal.
        # (Mounting once avoids mount/unmount churn on every modal open/close.)
        yield Omnibar(id="omnibar")
        yield BookmarkInput(id="bookmark_input")
        yield BookmarkPicker(id="bookmark_picker")
        yield OpLog(id="op_log")
        yield ConflictView(id="conflict_view")
        yield HunkPicker(id="hunk_picker")

    def watch_modal(self, modal: str | None) -> None:
        from textual.widget import Widget

        from lajjzy.widgets import (
            BookmarkInput,
            BookmarkPicker,
            ConflictView,
            HunkPicker,
            Omnibar,
            OpLog,
        )

        mapping: dict[str, type[Widget]] = {
            "omnibar": Omnibar,
            "bookmark_input": BookmarkInput,
            "bookmark_picker": BookmarkPicker,
            "op_log": OpLog,
            "conflict_view": ConflictView,
            "hunk_picker": HunkPicker,
        }
        for name, cls in mapping.items():
            try:
                w = self.query_one(cls)
            except Exception:
                continue
            w.display = modal == name

    def on_mount(self) -> None:
        from lajjzy.widgets import (
            BookmarkInput,
            BookmarkPicker,
            ConflictView,
            GraphView,
            HunkPicker,
            Omnibar,
            OpLog,
        )

        self.query_one(GraphView).focus()
        # Modals start hidden; watch_modal shows the active one.
        for cls in (Omnibar, BookmarkInput, BookmarkPicker, OpLog, ConflictView, HunkPicker):
            self.query_one(cls).display = False
        self.runtime.dispatch(ReloadRequested())

    # -- key bindings → messages -----------------------------------------

    def action_cursor_down(self) -> None:
        self.runtime.dispatch(CursorDown())

    def action_cursor_up(self) -> None:
        self.runtime.dispatch(CursorUp())

    def action_cursor_top(self) -> None:
        self.runtime.dispatch(CursorTop())

    def action_cursor_bottom(self) -> None:
        self.runtime.dispatch(CursorBottom())

    def action_reload_graph(self) -> None:
        self.runtime.dispatch(ReloadRequested())

    def action_focus_detail(self) -> None:
        from lajjzy.widgets import DetailPanel

        self.query_one(DetailPanel).focus()

    def action_new(self) -> None:
        self.runtime.dispatch(NewChange())

    def action_abandon(self) -> None:
        self.runtime.dispatch(Abandon())

    def action_edit(self) -> None:
        self.runtime.dispatch(EditChange())

    def action_squash(self) -> None:
        self.runtime.dispatch(Squash())

    def action_describe(self) -> None:
        self.runtime.dispatch(DescribeRequested())

    def action_rebase(self) -> None:
        self.runtime.dispatch(RebaseStart(descendants=False))

    def action_rebase_descendants(self) -> None:
        self.runtime.dispatch(RebaseStart(descendants=True))

    def action_rebase_confirm(self) -> None:
        self.runtime.dispatch(RebaseConfirm())

    def action_rebase_cancel(self) -> None:
        self.runtime.dispatch(RebaseCancel())

    def action_undo(self) -> None:
        self.runtime.dispatch(Undo())

    def action_redo(self) -> None:
        self.runtime.dispatch(Redo())

    def action_open_omnibar(self) -> None:
        self.runtime.dispatch(OpenOmnibar())

    def action_open_bookmark_set(self) -> None:
        self.runtime.dispatch(OpenBookmarkSet())

    def action_open_bookmark_picker(self) -> None:
        self.runtime.dispatch(OpenBookmarkPicker())

    def action_open_op_log(self) -> None:
        self.runtime.dispatch(OpenOpLog())

    def action_split(self) -> None:
        self.runtime.dispatch(Split())

    def action_squash_partial(self) -> None:
        self.runtime.dispatch(SquashPartial())

    # -- $EDITOR (app-layer terminal suspend) ----------------------------

    def _edit_message_in_editor(self, seed: str) -> tuple[str | None, str | None]:
        """Launch $EDITOR seeded with ``seed``. Returns ``(text, None)`` on
        success or ``(None, error)`` on failure/abort. Does not touch app state —
        the result flows back through a Msg so the core owns the error."""
        editor = os.environ.get("EDITOR")
        if not editor:
            return None, "No $EDITOR set"
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
                    return None, f"Editor exited with code {result.returncode}"
                with open(path, encoding="utf-8") as fh:
                    return fh.read().strip(), None
            except FileNotFoundError:
                return None, f"Editor not found: {editor!r}"
            except OSError as exc:
                return None, f"Editor error: {exc}"
        finally:
            try:
                os.unlink(path)
            except OSError:
                pass

    # -- out-of-loop helpers ---------------------------------------------

    async def ensure_working_copy(self, change_id: str) -> bool:
        """Working-copy gate: make ``change_id`` the @ commit before any
        filesystem-touching op. Returns True if @ is (now) the target.

        This is an out-of-band async helper for deferred filesystem features
        (hunk picker, conflict resolution); it sets the ``error`` reactive
        directly rather than flowing through the MVU loop.
        """
        graph = self.model.graph
        if graph and graph.working_copy_index is not None:
            current = graph.lines[graph.working_copy_index].change_id
            if current == change_id:
                return True
        try:
            await jj.edit_change(self.repo_path, change_id)
        except JjError as exc:
            self.error = str(exc)
            return False
        return True

    @work(group="diff", exclusive=True)
    async def open_diff(self, path: str) -> None:
        # Diff browsing is ephemeral view-local state owned by the detail pane,
        # not core application state, so it stays outside the Model/update loop.
        from lajjzy.widgets import DetailPanel

        change_id = self.selected_change_id()
        if change_id is None:
            return
        try:
            all_files = await jj.change_diff(self.repo_path, change_id)
        except JjError as exc:
            self.error = str(exc)
            return
        except InvariantError:
            # Must propagate (crash policy), not become a status-bar message.
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
    worker_invariant_error = app._invariant_error
    if worker_invariant_error is not None:
        print(f"lajjzy: internal invariant violated: {worker_invariant_error}", file=sys.stderr)
        print("This is a bug — please report it.", file=sys.stderr)
        sys.exit(70)
