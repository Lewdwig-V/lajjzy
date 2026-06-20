# lajjzy

A keyboard-driven, lazygit-style TUI for Jujutsu (jj).

## Requirements

- Python 3.11+
- `jj` CLI in PATH (tested with jj 0.42.0)

## Dev Commands

```bash
uv sync                 # create / sync the environment
uv run lajjzy           # run the TUI
uv run pytest           # run tests (jj in PATH required for integration tests)
uv run ruff check .     # lint
uv run ruff format .    # format
uv build                # build wheel + sdist
uv publish              # publish to PyPI
```

## Source Layout

```
src/lajjzy/
  app.py            # LajjzyApp — root App, reactive state, key bindings, worker dispatch
  __main__.py       # python -m lajjzy entry point
  styles.tcss       # Textual CSS
  backend/
    jj.py           # ONLY place that shells out to jj (asyncio.create_subprocess_exec)
    parse.py        # pure parsers: graph output → GraphData, diff output → FileDiff list
    types.py        # dataclasses: GraphData, GraphLine, ChangeDetail, FileDiff, JjError
  widgets/
    graph.py        # GraphView widget — renders the change DAG
    detail.py       # DetailPanel widget — file list + diff view
    status_bar.py   # StatusBar widget — reactive error/status display
```

## Architectural Constraints

- **Facade boundary:** `backend/jj.py` is the only module that runs `jj` subprocesses.
  Widget and app code never call `asyncio.create_subprocess_exec` or `subprocess` directly.
  The one exception is `app.py`'s `_edit_message_in_editor`, which calls `subprocess.run`
  to launch `$EDITOR` — this is an app-layer responsibility, not a widget responsibility.

- **No central store:** There is no global state object or Elm-style dispatch machine.
  State lives as `reactive()` attributes on `LajjzyApp`. Widgets watch the app's reactives
  via `self.watch(self.app, ...)` and re-render automatically.

- **Worker-group concurrency lanes:**
  - `group="mutation", exclusive=True` — the single-mutation gate; at most one mutation
    runs at a time. All write operations (`new`, `abandon`, `describe`, `squash`, `rebase`,
    `edit`) run in this group.
  - `group="load"` — graph reloads. Runs independently of mutations.
  - `group="diff"` — diff fetches for the detail pane. Runs independently.

- **Editor suspend in app layer only:** `$EDITOR` is launched via `self.suspend()` in
  `LajjzyApp._edit_message_in_editor`. Widget code never suspends the terminal.

- **Errors set `App.error`, never raise:** `JjError` exceptions from `backend/jj.py` are
  caught in workers and written to `self.error` (a reactive `str | None`). The `StatusBar`
  widget watches this reactive and displays the message. No unhandled exceptions reach the
  Textual event loop from backend calls.

- **Working-copy gate for filesystem ops:** Any operation that reads or writes repo files
  on disk requires the target change to be `@`. The `ensure_working_copy()` method on
  `LajjzyApp` handles this — it calls `jj edit` first if needed. Deferred features (hunk
  picker, conflict resolution) must call this before touching the working tree.

## Key Patterns

- Graph data loaded in bulk via `load_graph()` — one `jj log` subprocess call, parsed
  into `GraphData`. Not re-fetched per keypress.
- Cursor skips connector lines; always lands on change nodes (`graph.node_indices`).
- Mutations reload the graph inside the same worker, so the graph is consistent when
  the worker completes and the UI updates in one step.
- `reactive()` + `watch_*` is the only data-flow mechanism — no message buses or event
  queues beyond Textual's built-in event system.
