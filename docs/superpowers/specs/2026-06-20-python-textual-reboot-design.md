# lajjzy — Python + Textual reboot

**Status:** Design approved, pending implementation plan
**Date:** 2026-06-20
**Branch:** `reboot/python-textual`

## Why reboot

The `M8` migration (commit `731edd1`) dropped `jj-lib` and made `lajjzy-core`
a pure CLI orchestration + text-parsing layer (`jj log -T <template>`,
conflict-marker parsing). That deleted the one thing that made Rust
strategically necessary — direct linkage to jj's internals.

jj has no stable internal API but ships a powerful template language designed
for exactly this kind of programmatic consumption. So the backend is, and will
remain, "drive the `jj` CLI and parse its templated output." Rust buys almost
nothing for that work: no FFI, no hot loop, no memory-layout concern. The app's
center of gravity has moved to the TUI layer.

Four drivers, all confirmed:

1. **Dev velocity** — borrow-checker and ceremony tax on what is now glue + UI.
2. **Reactive UI dataflow** — the desired paradigm is fine-grained reactivity
   (derived state recomputes and re-renders when jj state changes), *not* the
   Elm-style `dispatch → Vec<Effect>` machine in place today. These two
   paradigms don't blend; that justifies a rewrite over a port.
3. **Contributor accessibility** — Python lowers the maintenance barrier.
4. **Architecture mismatch** — the crate split / facade ceremony is heavy for
   what the app actually is now.

**Target stack:** Python + [Textual](https://textual.textualize.io/). Textual's
`reactive()` attributes are the most mature "reactive UI dataflow for a
terminal" primitive that exists, and its async-native worker model maps directly
onto lajjzy's concurrency lanes.

Anti-recommendation, for the record: **Go + Bubble Tea is Elm-style again** — a
nicer language but the same paradigm we are deliberately leaving.

## Decisions

| Decision | Choice |
|---|---|
| Stack | Python + Textual |
| UI paradigm | Reactive (Textual `reactive`/`watch`/`compute`) |
| Scope | MVP core first, port the long tail incrementally |
| Spec workflow | Drop unslop for now; hand-write idiomatically; reintroduce once the architecture settles |
| Repo layout | New branch `reboot/python-textual`; replace `crates/` with the Python tree (burn the boats) |
| State posture | Textual-native distributed reactivity — **no** central `AppState` atom |

## Architecture

Three layers. The facade discipline from `lajjzy-core` is preserved; the Elm
core is replaced by reactivity.

- **`backend/`** — the *only* code that shells out to `jj`. Pure `async`
  functions returning typed dataclasses (`GraphData`, `ChangeDetail`,
  `ConflictData`). No Textual imports. Uses `asyncio.create_subprocess_exec`.
- **`state/` (Textual-native)** — `reactive()` attributes living on the `App`
  and widgets; derived values via `compute_*`. No subprocess calls. There is no
  single store object — state is distributed across the widgets that own it.
- **`widgets/` + `app.py`** — widgets render reactive state and bind keys to
  actions. Actions invoke backend through `@work` workers; workers write results
  into reactive state; `watch_*`/`compute_*` re-render automatically.

Data flow is a loop, not a pipeline:

```
key → action → @work(jj call) → reactive state update → watch/compute → re-render
```

### Backend boundary (carried over intact)

Nothing above `backend/` spawns a subprocess. Each jj operation is an
`async def` that runs `jj` and parses output. The jj log **templates** and the
**parsers** (including conflict-marker parsing) port almost verbatim from the
Rust `cli.rs` / `types.rs` — that logic is language-agnostic and already
battle-tested. `gh` forge integration follows the same shape (deferred past MVP).

### Concurrency lanes → Textual workers

The framework enforces the invariants that were hand-rolled in Rust:

| Rust today | Python reboot |
|---|---|
| `AppState.pending_mutation` gate | `@work(exclusive=True, group="mutation")` |
| push lane | `@work(group="push")` |
| fetch lane | `@work(group="fetch")` |
| `SuspendForEditor` effect intercepted in cli | `with app.suspend():` around the `$EDITOR` subprocess, in the app layer |

`exclusive=True` cancels any in-flight worker in the same group, giving "at most
one local mutation in flight" for free. The editor-suspend handoff stays
explicit and confined to the app layer — the same boundary as today.

### Reactive state design

- `App.graph: reactive[GraphData]` — set by the load worker; the graph widget
  watches it.
- `App.cursor: reactive[int]` — navigation; `compute_selected_change()` derives
  the highlighted change; the detail panel watches that.
- `App.error: reactive[str | None]` — any failed jj op sets it; the status bar
  watches it. (The "no panics; errors update state" rule, now framework-native.)
- **Working-copy gate** (the `@`-must-be-target rule before any filesystem op)
  stays an explicit guard in the action handler: if the target change is not
  `@`, switch the working copy first, then proceed — unchanged in spirit, and
  still visible to the user as the `@` marker moving.

## MVP scope

**Ships first:** graph view + navigation • detail/diff panel • core mutations
(`new`, `describe` via `$EDITOR` suspend, `edit`, `abandon`, `squash`,
`rebase`) • error display + status bar.

**Deferred:** omnibar / revset completion • hunk picker • conflict view •
bookmark picker/input • op log • forge (`gh`) integration.

## Distribution

Ship as a `uv tool install` / `pipx` package exposing a `lajjzy` entry point —
the dev audience already lives in that ecosystem. Single-binary parity via
PyInstaller/Nuitka is a **post-MVP** option if install friction shows up, not a
day-one requirement.

## Testing

- **Backend:** plain async `pytest` against a real temporary `jj` repo — the
  same integration strategy `cli.rs` uses today.
- **UI:** Textual's `Pilot` via `App.run_test()` for key-driven interaction
  tests.
- No unslop / spec-driven coverage for now (per decision above).

## Migration ledger

- **Port — intent survives:** jj log templates, output parsers, conflict-marker
  parsing, completion-ranking logic, status-bar priority rules, key map.
- **Retire — mechanism dies:** `Action` / `Effect` enums, `dispatch()`, the
  effect executor, the crate split, `pending_mutation` bookkeeping.
- **Reference:** the Rust tree (in git history after replacement) remains the
  behavioral spec — when a feature's behavior is ambiguous, the old code is the
  source of truth.

## Build order

1. **Backend `load_graph` + parsers**, tested against a temp repo.
2. **App shell + graph widget** rendering reactive `graph`, with navigation.
3. **Detail/diff panel** driven by `compute_selected_change`.
4. **Mutations, one lane at a time:** `new`/`abandon` (instant) → `describe`
   (editor suspend) → `squash`/`rebase`.
5. **Status bar + error reactivity.**
