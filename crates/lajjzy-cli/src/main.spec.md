---
managed-file: crates/lajjzy-cli/src/main.rs
intent: >
  Binary entry point that initialises a jj workspace backend and forge,
  bootstraps AppState with the initial graph, installs a panic handler that
  restores the terminal before printing the panic, then runs an Elm-style event
  loop: render → poll crossterm events (key, mouse) → map to Actions → dispatch
  → execute returned Effects. Synchronous effects (SuspendForEditor,
  LaunchMergeTool, OpenOrCreatePr) are intercepted here, suspend the terminal,
  run subprocesses, restore the terminal, and re-enter dispatch. All other
  effects are offloaded to background threads via EffectExecutor, which sends
  completion Actions back over an mpsc channel that the loop drains before each
  render. A monotonically-incrementing graph generation counter prevents stale
  graph loads from overwriting newer ones. The active revset is snapshotted at
  mutation-completion time so background threads always refresh with the filter
  currently visible to the user.
intent-approved: false
intent-hash: 8b05a9ebf1bb
distilled-from:
  - path: crates/lajjzy-cli/src/main.rs
    hash: 2bf26bbf2d0a
non-goals:
  - Does not implement any jj repository operations — all mutations and reads are
    delegated to RepoBackend
  - Does not own UI layout, widget rendering, or keybinding tables — those live
    in lajjzy-tui
  - Does not perform input validation or state-machine logic — dispatch() in
    lajjzy-tui is the sole owner of state transitions
depends-on:
  - crates/lajjzy-tui/src/dispatch/dispatch.spec.md
  - crates/lajjzy-tui/src/effect.spec.md
  - crates/lajjzy-tui/src/action.spec.md
  - crates/lajjzy-tui/src/app.spec.md
  - crates/lajjzy-tui/src/input.spec.md
  - crates/lajjzy-tui/src/mouse.spec.md
  - crates/lajjzy-core/src/backend.spec.md
  - crates/lajjzy-core/src/forge.spec.md
  - crates/lajjzy-core/src/gh.spec.md
---

## Purpose

`main.rs` is the sole binary entry point. It wires together the backend,
forge, TUI state, and effect executor into a running application. Callers
observe a keyboard- and mouse-driven terminal UI that reflects jj repository
state in real time and responds to user input without blocking the render loop.

## Behavior

1. **Startup** — Parses CLI arguments (version/about only; no subcommands).
   Resolves the current working directory, constructs `JjCliBackend` and
   `GhCliForge`, loads the initial commit graph, and creates `AppState`.

2. **Panic handler** — Before entering the event loop, installs a custom panic
   hook that disables mouse capture and restores the terminal to its prior state
   before forwarding to the original hook. Guarantees the shell is left usable
   after a crash.

3. **Terminal lifecycle** — Calls `ratatui::init()` to enter alternate-screen
   raw mode and enables mouse capture. On exit (clean or panicked), calls
   `ratatui::restore()` and disables mouse capture.

4. **Event loop** — Renders the current state, updates viewport heights for
   hunk-picker and conflict-view, then polls for crossterm events with a 50 ms
   timeout.
   - **Key press events** — Cleared status message. Routed through picking mode
     (if active), then modal (if open), then normal `map_event`. Unhandled keys
     inside the Describe modal are forwarded to the embedded tui-textarea
     instance after crossterm version translation.
   - **Mouse events** — Cleared status message. Translated to Actions via
     `map_mouse_event`, then dispatched.
   - **Background results** — The mpsc channel is drained completely after each
     event-poll cycle; all pending `Action` completions are dispatched before
     the next render.
   - **Quit** — Loop exits when `state.should_quit` is `true`.

5. **Effect routing in `execute_effects`** — Three effects are intercepted
   synchronously; all others are forwarded to `EffectExecutor::execute`:
   - `SuspendForEditor`: disables mouse capture, restores terminal, launches
     `$EDITOR`/`$VISUAL`/`vi` with the commit message in a temp file, re-inits
     terminal, re-enables mouse capture, then dispatches `EditorComplete` or
     sets `state.error`.
   - `LaunchMergeTool`: same terminal suspend/restore cycle, runs
     `jj resolve <path> -r @` in the workspace root, then dispatches
     `MergeToolComplete` (with a freshly-loaded graph) or `MergeToolFailed`.
   - `OpenOrCreatePr`: tries `gh pr view <bookmark> --web` first; if successful
     also fetches the PR URL and dispatches `PrViewUrl`. If no PR exists,
     suspends the terminal, runs `gh pr create --head <bookmark>` interactively,
     restores the terminal, then dispatches `PrCreateComplete` or
     `PrCreateFailed`.

6. **EffectExecutor** — Owns `Arc<JjCliBackend>`, `Arc<GhCliForge>`, the mpsc
   `Sender<Action>`, a `AtomicU64` graph-generation counter, and an
   `Arc<Mutex<Option<String>>>` tracking the active revset. Each call to
   `execute()` spawns a new OS thread. Send failures (receiver dropped) are
   silently discarded — the spawned thread exits with no further work to do.

7. **Graph generation** — Every effect that will trigger a graph reload
   atomically increments the generation counter before the thread is spawned.
   Effects that do not load a graph receive generation `0`. The dispatch layer
   uses generation numbers to discard results from superseded loads.

8. **Mutation completion** — `run_mutation` calls the backend closure, then on
   success reads the active revset under lock at completion time and immediately
   calls `load_graph` in the same thread. The result is bundled into
   `Action::RepoOpSuccess { graph: Some(...) }`. On failure it sends
   `Action::RepoOpFailed`.

9. **Revset synchronisation** — After every `dispatch()` call in the event
   loop, `executor.active_revset` is updated to match `state.active_revset`
   under the mutex. This ensures background threads that complete later pick up
   the user's current filter.

10. **tui-textarea key bridging** — `key_event_to_textarea_input` manually
    translates crossterm 0.29 `KeyEvent` values to tui-textarea `Input` structs
    (which use crossterm 0.28 types internally) to keep the describe-modal text
    editor functional across the version mismatch.

## Constraints

- At most one `SuspendForEditor` or `LaunchMergeTool` can be in flight at any
  time because both are executed synchronously in the event loop before the next
  render.
- `OpenOrCreatePr` is also synchronous and blocks the event loop for the
  duration of the `gh` subprocess.
- `Effect::SuspendForEditor`, `Effect::LaunchMergeTool`, and
  `Effect::OpenOrCreatePr` must never reach `EffectExecutor::execute`; reaching
  them there is a hard `unreachable!()` panic.
- The event loop never calls backend methods or performs I/O directly; all such
  work flows through `Effect` → `execute_effects` → background thread.
- `state.should_quit` is the sole termination condition; the loop does not
  respond to signals.
- Terminal is always restored before the process exits, whether via normal
  shutdown or panic.

## Dependencies

- **Runtime:** `lajjzy-tui` (dispatch, render, AppState, Action, Effect, input
  mapping, mouse mapping, modal types), `lajjzy-core` (JjCliBackend,
  GhCliForge, RepoBackend trait, ForgeBackend trait), `ratatui`, `crossterm`,
  `clap`, `anyhow`, `tui-textarea`, `tempfile`.
- **System:** `jj` CLI in PATH (invoked synchronously by `LaunchMergeTool`),
  `gh` CLI in PATH (invoked synchronously by `OpenOrCreatePr`), `$EDITOR` /
  `$VISUAL` environment variables (with `vi` as fallback).
