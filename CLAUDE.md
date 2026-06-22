# lajjzy

A keyboard-driven, lazygit-style TUI for Jujutsu (jj).

## Requirements

- Python 3.11+
- `jj` CLI in PATH (tested with jj 0.42.0)

## Dev Commands

```bash
uv sync                      # create / sync the environment
uv run lajjzy                # run the TUI
uv run pytest                # run tests (jj in PATH required for integration tests)
uv run ruff check .          # lint
uv run ruff format .         # format
uv run ruff format --check . # CI format gate (don't write, just check)
uv build                     # build wheel + sdist
uv publish                   # publish to PyPI
```

CI (`.github/workflows/ci.yml`) runs `ruff check`, `ruff format --check`, and
`pytest` (with `jj` installed) on PRs and pushes to `main`. Keep these green.

## Feature parity & the Rust reference

This Python implementation is **not yet at feature parity** with the original
Rust prototype. The Rust tree was removed in the `reboot/python-textual` cut-over
and lives in **git history** (commits up to `731edd1`, under `crates/`) — it is
the behavioural reference when porting a feature. The README's
*Feature status & gaps* table is the authoritative inventory of what's shipped,
partial, and not yet ported; the *Roadmap* orders the work. When porting a Rust
feature, read its old widget/backend code for the exact behaviour, but build it
on the MVU core (pure `Model`/`Msg`/`update`) — see below — rather than as a
literal translation of the Rust Elm `Effect` machine.

## Architecture: a pure MVU core with a swappable backend

The application logic is a small, pure Model-View-Update core that knows nothing
about Textual, asyncio, or jj. Textual is one *backend* that drives it; the core
would run unchanged behind any renderer. The dividend is testability: every state
transition is asserted against plain data (`tests/core/test_update.py`), and the
whole loop is driven headless in `tests/runtime/test_runtime.py`.

The data flow is one direction only:

```
key press → Msg → update(Model, Msg) → (Model', [Cmd]) → backend presents Model'
                                                       └→ backend runs each Cmd → Msg → …
```

- **The core is pure (`core/`).** `update(model, msg) -> (Model, list[Cmd])` is the
  ONLY place state transitions happen. No I/O, no async, no Textual, no jj imports.
  `Cmd`s are *descriptions* of effects (`LoadGraph`, `RunMutation`, `EditMessage`),
  never the effects themselves. `Model` is a frozen dataclass — the single source
  of truth for graph/cursor/error/rebase/mutation-gate/epoch state.

- **The Runtime is renderer-agnostic (`runtime/`).** `Runtime` owns the `Model`,
  feeds each `Msg` through `update`, asks the backend to present the new model, and
  asks the backend to run each `Cmd`. The seam is the `Backend` Protocol
  (`present(model)` + `run_cmd(cmd, dispatch)`).

- **Textual is just one Backend (`app.py`).** `LajjzyApp` implements `Backend`:
  `present` *projects* the `Model` onto `reactive()` attributes the widgets watch;
  `run_cmd` executes effects on Textual's worker lanes. Key bindings only build a
  `Msg` and dispatch it — no logic lives in the actions. The reactives are a view
  projection, never the source of truth.

## Source Layout

```
src/lajjzy/
  app.py            # LajjzyApp — the Textual Backend: projects Model→reactives, runs Cmds on workers
  __main__.py       # python -m lajjzy entry point
  styles.tcss       # Textual CSS
  core/             # the pure MVU core — no Textual / asyncio / jj
    model.py        # Model (frozen) + pure helpers (selected_change_id, step_cursor, …)
    messages.py     # Msg union: user intents + effect-result messages
    commands.py     # Cmd union: LoadGraph, RunMutation, EditMessage (effect descriptions)
    update.py       # update(model, msg) -> (Model, [Cmd]) — the only state-transition fn
  runtime/
    backend.py      # Backend Protocol (the swappable seam) + renderer-agnostic Runtime
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

- **Purity of the core:** nothing in `core/` may import Textual, asyncio, or the jj
  facade, or perform I/O. New behaviour is a new `Msg` + a branch in `update`, with a
  unit test in `tests/core/`. If a transition needs the outside world, it emits a `Cmd`.

- **Two facade boundaries:** `backend/jj.py` is the only module that runs `jj`
  subprocesses; the `Backend` (`app.py`) is the only place effects are actually
  executed. Widget code never calls `asyncio.create_subprocess_exec`/`subprocess`,
  and never runs effects directly. The `EditMessage` cmd launches `$EDITOR` via
  `LajjzyApp._edit_message_in_editor` (`subprocess.run` + `self.suspend()`) — an
  app/backend responsibility, not a widget one.

- **Worker-group concurrency lanes** (the backend's `run_cmd` maps Cmds to these):
  - `group="mutation"` — runs a `RunMutation` cmd (the write op + its follow-up
    reload, in one worker). The single-mutation gate is the `pending_mutation` flag
    in the pure `Model`/`update`, NOT worker exclusivity.
  - `group="load", exclusive=True` — runs a `LoadGraph` cmd. A new reload cancels any
    running reload; the `graph_epoch` guard in `update` discards stale results.
  - `group="diff", exclusive=True` — diff fetches for the detail pane. Diff browsing
    is ephemeral view-local state owned by `DetailPanel`, deliberately OUTSIDE the
    Model/update loop. A new fetch cancels any in-flight diff fetch.

- **Errors flow as messages, never raise:** `JjError` from `backend/jj.py` is caught
  in the backend's workers and dispatched back as a result `Msg`
  (`GraphLoadFailed`, `MutationFailed`, `MutationCompleted(load_error=…)`); `update`
  writes it to `Model.error`, which `present` projects to the `error` reactive that
  `StatusBar` watches. No unhandled exception reaches the Textual event loop.

- **Working-copy gate for filesystem ops:** Any operation that reads or writes repo
  files on disk requires the target change to be `@`. `LajjzyApp.ensure_working_copy()`
  handles this (calls `jj edit` if needed). It is an out-of-band async helper for
  deferred filesystem features (hunk picker, conflict resolution) and sets the `error`
  reactive directly rather than flowing through the loop.

## Key Patterns

- Graph data loaded in bulk via `load_graph()` — one `jj log` subprocess call, parsed
  into `GraphData`. Not re-fetched per keypress.
- Cursor skips connector lines; always lands on change nodes (`graph.node_indices`).
- A mutation's `RunMutation` cmd reloads the graph in the same worker, so the new graph
  arrives with the result in one `MutationCompleted` message — the UI updates in one step.
- The MVU loop (`Msg` → `update` → `Cmd`) is the only data-flow mechanism. Widgets watch
  the projected reactives; there are no message buses beyond Textual's event system.
