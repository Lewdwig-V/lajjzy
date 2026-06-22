# Daily-driver essentials — design

**Date:** 2026-06-22
**Scope:** Close the README "Next — daily-driver essentials" gap: undo/redo,
omnibar, bookmark management, operation log, conflict view, hunk picker.
**Reference:** Rust prototype at commit `731edd1` (under `crates/`, deleted in
`3c98bad`); README *Feature status & gaps* + *Roadmap* are the authoritative
inventory. Prior Rust-era specs live under `docs/superpowers/specs/` and are
referenced where they still describe behaviour accurately.

## Goal

Make lajjzy a viable daily driver by porting the six "Next" features from the
Rust prototype onto the existing pure-MVU core (`core/` + `runtime/` +
Textual `Backend` in `app.py`). No architectural changes — every new behaviour
is a new `Msg` + `update` branch, a new `Cmd` run by the backend, and a widget
that projects Model state and dispatches Msgs.

## Non-goals

- Push/fetch worker lanes, forge/gh integration (roadmap "depth").
- Mouse support, configurable keymaps, theming, inline describe editor (depth).
- Absorb, duplicate, revert mutations (depth).
- Help overlay (`?`) — not in the six.
- Any change to the MVU seam itself, the `pending_mutation` gate, the epoch
  guard, or the crash policy fixed in PR #30.

## Structure: two-phase, six parallel feature agents

**Phase 1 — seam scaffold (one PR, sequential).** Lands every shared-file
change required by all six features so phase-2 agents never touch a shared
file:

- All new `Msg` types in `core/messages.py`.
- All new `Cmd` types in `core/commands.py`.
- Every new `update` branch in `core/update.py` (pure transitions; mutations
  go through the existing `_start_mutation` gate; the reload-during-mutation
  guard from PR #30 is preserved).
- New `Model` fields + pure helpers in `core/model.py`.
- **All** new `backend/jj.py` facade functions implemented (mechanical ports
  from `crates/lajjzy-core/src/backend.rs`): `undo`, `redo`, `op_log`,
  `op_restore`, `bookmark_set`, `bookmark_delete`, `bookmark_move`,
`load_bookmarks`, `split`, `squash_partial`, `conflict_data`, `resolve`.
  Plus `load_graph` gains an optional `revset: str | None` parameter.
- New keybindings + `run_cmd` branches + workers in `app.py`, plus a `modal`
  reactive projected from Model for modal lifecycle.
- Six stub widget files in `widgets/` (empty render, so imports resolve).
- Pure unit tests in `tests/core/test_update.py` pinning every new `update`
  branch — these are the contract phase-2 agents must satisfy.

Phase-1 verification gate: `ruff check` + `ruff format --check` + `mypy --strict`
+ `pytest` all green with stub widgets rendering empty.

**Phase 2 — six parallel feature agents.** Each agent takes exactly one
feature and touches ONLY:

- its one widget file (`widgets/<feature>.py`);
- its one test file (`tests/widgets/test_<feature>.py` or additions to
  `tests/test_app.py`);
- nothing in `core/`, `app.py`, or `backend/jj.py`.

Phase-2 agents read `app.py` to learn which reactives/Msgs their widget
dispatches; they never edit it. Conflicts between phase-2 agents are
structurally impossible.

## Architecture rules (carried from AGENTS.md)

- **Core purity:** nothing in `core/` imports Textual, asyncio, or the jj
  facade, or performs I/O. New behaviour = new `Msg` + `update` branch + unit
  test. A transition needing the outside world emits a `Cmd`.
- **Two facade boundaries:** `backend/jj.py` is the only module that runs `jj`
  subprocesses; `app.py` is the only place effects are executed. Widget code
  never calls subprocess/asyncio and never runs effects directly.
- **Worker lanes:** all six features' effects run on the existing lanes —
  mutations on `group="mutation"` (non-exclusive, gate is the pure
  `pending_mutation` flag), graph loads on `group="load", exclusive=True`,
  detail/diff/conflict/hunk data fetches on `group="diff", exclusive=True`.
  No new lanes. Push/fetch lanes are explicitly out of scope.
- **Errors flow as messages:** `JjError` from `backend/jj.py` is caught in
  workers and dispatched back as a result `Msg` (`GraphLoadFailed`,
  `MutationFailed`, `MutationCompleted(load_error=…)`, `OpLogLoadFailed`,
  `ConflictDataLoadFailed`, `BookmarksLoadFailed`); `update` writes it to
  `Model.error`. No unhandled exception reaches the Textual event loop.
  `InvariantError` propagates per the crash policy fixed in PR #30.
- **Working-copy gate:** any op that reads or writes repo files on disk
  requires the target change to be `@`. Only conflict-apply uses this (via
  `LajjzyApp.ensure_working_copy`). Split/squash/undo/redo/bookmarks/op-restore
  do not need `@`.

## Core Model additions

New `Model` fields (all default to empty/None so existing tests stay green):

```python
op_log_entries: list[OpLogEntry] | None = None
bookmarks: list[Bookmark] | None = None
revset: str | None = None                 # active revset filter, None = unfiltered
conflict_data: ConflictData | None = None  # for the currently-open conflict view
conflict_path: str | None = None
modal: str | None = None                  # "omnibar" | "bookmark_input" | "bookmark_picker" |
                                          # "op_log" | "conflict_view" | "hunk_picker" | None
```

View-local state (NOT in Model — owned by each widget, like `DetailPanel`'s
diff browsing): omnibar query/cursor/completions/matches, op-log cursor/scroll,
conflict per-hunk resolutions/cursor/scroll, hunk-picker files/cursor/selected/
scroll, bookmark input text.

New pure helpers in `core/model.py` as needed (e.g. `bookmark_at_cursor`,
`conflict_hunk_count`).

## Keybindings

Reassignments forced by conflicts with existing Python bindings (Rust used
different keys):

| Key | Action | Note |
|-----|--------|------|
| `u` | Undo | matches Rust |
| `U` | Redo | Rust used `Ctrl-R`, taken by rebase-descendants in Python |
| `/` | Open omnibar | new |
| `B` | Open bookmark set (on selected change) | matches Rust |
| `b` | Open bookmark picker | new |
| `o` | Open operation log | new |
| `s` | Split (opens hunk picker) | matches Rust |
| `Ctrl-S` | Partial squash (opens hunk picker) | Rust used `S`, taken by whole-change squash in Python |
| (from detail pane, `Enter` on conflicted file) | Open conflict view | new |

Modal-internal keys (omnibar `Tab`/`Enter`/`Esc`; pickers `j`/`k`/`Enter`/`Esc`;
hunk picker `Space`/`Enter`/`Esc`; conflict `1`/`2`/`l`/`r`/`Enter`/`Esc`) are
handled inside each widget, not in `app.py` bindings.

## Per-feature design

### 1. Undo/redo

- **Model:** no new field.
- **Msg:** `Undo`, `Redo` (user intents). No new result Msg — reuses
  `MutationCompleted`/`MutationFailed`.
- **Cmd:** reuses `RunMutation`. New `_OPS` kinds: `"undo"`, `"redo"`, args
  `()`.
- **Facade:** `jj.undo(repo) -> str`, `jj.redo(repo) -> str` (port from
  `backend.rs`).
- **Update:** `update(model, Undo)` → `_start_mutation(model, "undo", ())`;
  `Redo` identical. Worker runs `jj.undo` then `jj.load_graph`, dispatches
  `MutationCompleted(epoch, message, graph, load_error)`. Same shape as
  `NewChange`; no new code path.
- **Widget:** none. Status bar shows the result message.
- **Tests:** `test_undo_starts_mutation`, `test_undo_blocked_while_pending`,
  `test_undo_reload_race_dropped` (reload during undo is ignored, per PR #30
  guard), integration `test_u_key_runs_jj_undo`, `test_U_key_runs_jj_redo`.

### 2. Omnibar (revset search + completion)

- **Model:** `revset: str | None`.
- **Msg:** `OpenOmnibar`, `OmnibarInput(char)`, `OmnibarBackspace`,
  `OmnibarAcceptCompletion`, `OmnibarSubmit(revset: str | None)`,
  `OmnibarCancel`.
- **Cmd:** `LoadGraph` gains optional `revset: str | None` field.
  `OmnibarSubmit` with a non-empty revset → `replace(model, revset=revset,
  modal=None)` + `[LoadGraph(epoch, revset)]`. `OmnibarSubmit(None)` (empty
  query) → clear revset + reload unfiltered. `OmnibarCancel` → `modal=None`.
- **Facade:** `jj.load_graph(repo, revset=None)` already accepts a revset
  (verify in phase 1; if not, add the `--revisions` flag).
- **Widget** (`widgets/omnibar.py`): modal overlay, view-local state
  `query/cursor/completions/matches`. Completions = hardcoded revset function
  list (`all()`, `mine()`, `@`, `heads()`, `bookmarks()`, `description(…)`,
  `author(…)`, etc.) filtered by substring on query. Matches = substring fuzzy
  over `graph.lines` (`change_id + author + description`). 4-state title per
  Rust spec (`docs/superpowers/specs/2026-03-21-m3a-omnibar-design.md` and
  `2026-03-22-m3d-revset-autocomplete-design.md` are the behaviour reference):
  completions present → `" / Tab: accept | Enter: submit as revset "`;
  empty query, no active revset → `" / Search or Revset "`;
  active revset → `" / Revset (active) "`;
  non-empty query, no active revset → `" / Search (Enter to filter as revset) "`.
  Selected row reversed. `Tab` accepts completion into query, `Enter` submits,
  `Esc` cancels, typing dispatches `OmnibarInput`.
- **Bind:** `/` → `OpenOmnibar` (sets `modal="omnibar"`).
- **Tests:** 4 render states, fuzzy match ranking, completion accept, submit
  triggers filtered reload, cancel leaves `revset` unchanged, reload during
  omnibar-open is fine (no gate needed — omnibar isn't a mutation).

### 3. Bookmark set/delete/move + picker

- **Model:** `bookmarks: list[Bookmark] | None`. `Bookmark = (name: str,
  change_id: str, change_description: str)`.
- **Msg:** `OpenBookmarkSet`, `OpenBookmarkPicker`, `BookmarkInputChar(char)`,
  `BookmarkInputBackspace`, `BookmarkInputConfirm(name: str)`,
  `BookmarkInputCancel`, `BookmarkDelete(name: str)`, `BookmarkMove(name:
  str)`, `BookmarkMoveConfirm(name: str, dest_change_id: str)`,
  `BookmarksLoaded(list[Bookmark])`, `BookmarksLoadFailed(str)`.
- **Cmd:** `LoadBookmarks` (run on mount and after every bookmark mutation's
  `MutationCompleted`). `RunMutation` kinds: `"bookmark_set"`,
  `"bookmark_delete"`, `"bookmark_move"`.
- **Facade:** `jj.bookmark_set(repo, change_id, name)`,
  `jj.bookmark_delete(repo, name)`, `jj.bookmark_move(repo, name, dest)`,
  `jj.load_bookmarks(repo) -> list[Bookmark]` (parse `jj bookmark list`).
- **Update:** set/delete/move go through `_start_mutation`; on
  `MutationCompleted` the worker also dispatches `LoadBookmarks` (or the
  completion handler re-fetches). `BookmarkMove` opens the picker in
  "pick destination" mode → `BookmarkMoveConfirm` → mutation.
- **Widget** (`widgets/bookmark_input.py`): modal single-line input, title
  `Set bookmark (Enter confirm | Esc cancel)`, view-local `text`,
  completions from existing bookmark names (case-insensitive substring, per
  Rust `bookmark_input.spec.md`). `Enter` → `BookmarkInputConfirm`, `Esc` →
  `BookmarkInputCancel`.
- **Widget** (`widgets/bookmark_picker.py`): modal list, blue border titled
  `Bookmarks`, name in magenta + change description in dark gray, cursor
  reversed, auto-scroll per Rust `bookmark_picker.spec.md`. `Enter` → jump
  cursor to that change (dispatches a cursor Msg), `d` → `BookmarkDelete`,
  `m` → `BookmarkMove` (opens picker-in-destination-mode), `Esc` → cancel.
- **Bind:** `B` → `OpenBookmarkSet` (on selected change), `b` →
  `OpenBookmarkPicker`.
- **Tests:** set/delete/move transitions through the gate, picker navigation,
  input completion filter, move-then-pick-destination flow, bookmarks reload
  after mutation.

### 4. Operation log (browse + restore)

- **Model:** `op_log_entries: list[OpLogEntry] | None`. `OpLogEntry = (op_id:
  str, timestamp: str, description: str)`.
- **Msg:** `OpenOpLog`, `OpLogClose`, `OpLogRestore(op_id: str)`,
  `OpLogLoaded(list[OpLogEntry])`, `OpLogLoadFailed(str)`.
- **Cmd:** `LoadOpLog` (run on `OpenOpLog`). `RunMutation` kind `"op_restore"`,
  args `(op_id,)`.
- **Facade:** `jj.op_log(repo) -> list[OpLogEntry]` (parse `jj op log`),
  `jj.op_restore(repo, op_id) -> str` (runs `jj op restore <op_id>`).
- **Update:** `OpenOpLog` → `replace(model, modal="op_log")` +
  `[LoadOpLog]`. `OpLogLoaded` → store entries. `OpLogRestore` →
  `_start_mutation("op_restore", (op_id,))`; worker runs `jj.op_restore` then
  `jj.load_graph`, dispatches `MutationCompleted`.
- **Widget** (`widgets/op_log.py`): modal panel, blue border titled
  `Operation Log`, renders `op_id` (yellow) + `timestamp` (cyan) +
  `description`, cursor reversed, auto-scroll per Rust `op_log.spec.md`.
  View-local `cursor/scroll`. `j`/`k` move, `Enter` → `OpLogRestore`,
  `Esc` → `OpLogClose`.
- **Bind:** `o` → `OpenOpLog`.
- **Tests:** op log loads on open, restore runs mutation + reloads graph,
  navigation, empty-state render, restore-while-pending is gated.

### 5. Conflict view (view + pick + apply)

- **Model:** `conflict_data: ConflictData | None`, `conflict_path: str | None`.
  `ConflictData = (regions: list[ConflictRegion],)` where `ConflictRegion =
  Resolved(text) | Conflict(base: list[str], left: list[str], right:
  list[str])`.
- **Msg:** `OpenConflictView(path: str)`, `ConflictViewClose`,
  `ApplyResolutions(path: str, resolutions: list[HunkResolution])`,
  `ConflictDataLoaded(ConflictData)`, `ConflictDataLoadFailed(str)`.
  `HunkResolution = None | AcceptLeft | AcceptRight`.
  (Per-hunk resolution choices are widget-local, not core Msgs — see note
  below. Only `ApplyResolutions` sends the final choices to core.)
- **Cmd:** `LoadConflictData(path)` (on `group="diff"`). `RunMutation` kind
  `"resolve"`, args `(path, resolutions)`.
- **Facade:** `jj.conflict_data(repo, path) -> ConflictData` (parses
  `jj file show` / `jj resolve --list` output for a conflicted file — the
  exact jj incantation is resolved in phase 1; the Rust `backend.rs` is the
  reference). `jj.resolve(repo, path, resolutions) -> str` constructs the
  resolved file content by concatenating accepted sides + resolved regions
  and writes it (the one non-mechanical facade function in this scope).
- **Update:** `OpenConflictView(path)` → `replace(model, modal="conflict_view",
  conflict_path=path)` + `[LoadConflictData(path)]`. `ApplyResolutions` →
  `ensure_working_copy` (on the conflicted change) then
  `_start_mutation("resolve", (path, resolutions))`. Worker runs `jj.resolve`
  then `jj.load_graph`, dispatches `MutationCompleted`.
- **Widget** (`widgets/conflict_view.py`): replaces DetailPanel content when
  `modal == "conflict_view"`. Renders regions per Rust `conflict_view.spec.md`:
  base DarkGray, left Blue, right Green, resolution status Yellow, `··· N
  lines ···` collapsed resolved, bold on current hunk, `"  (file deleted)"`
  italic for empty side. View-local `resolutions/cursor/scroll`. Keys: `j`/`k`
  move hunk, `1`/`l` → accept left, `2`/`r` → accept right, `Enter`/`a` →
  `ApplyResolutions`, `Esc` → `ConflictViewClose`.
- **Bind:** from DetailPanel, `Enter` on a file with `has_conflict=True` →
  `OpenConflictView(path)` (DetailPanel already dispatches on `Enter`; phase
  1 routes conflicted-file Enter to `OpenConflictView` instead of the diff
  view).
- **Working-copy gate:** `ApplyResolutions` calls
  `LajjzyApp.ensure_working_copy(conflict_change_id)` before the mutation; if
  it fails (can't edit to the conflicted change), `error` is set and the
  mutation is not started.
- **Tests:** view renders regions + collapsed resolved, pick updates
  resolution state, apply writes file and clears `has_conflict` on reload,
  working-copy gate enforced (can't resolve a conflict on a non-`@` change),
  empty-conflict render.

  **Note on ResolveHunk:** the per-hunk resolution choices are view-local
  (ephemeral, like DetailPanel's diff cursor) — they live in `ConflictView`
  and are only sent to core on `ApplyResolutions`. This keeps the Model
  uncluttered and matches the existing pattern. `ResolveHunk` is a widget-
  internal event, not a core `Msg`; the widget updates its `resolutions` list
  directly and re-renders.

### 6. Hunk picker (split + partial squash)

- **Model:** no new field. The picker is invoked from `Split`/`SquashPartial`
  and commits the selection as mutation args.
- **Msg:** `Split` (user intent, opens picker), `SquashPartial` (opens
  picker), `HunkPickerClose`, `SplitConfirm(source: str, selected_hunks:
  list[HunkRef])`, `SquashPartialConfirm(source: str, selected_hunks:
  list[HunkRef])`. `HunkRef = (file_path: str, hunk_idx: int)`.
- **Cmd:** `LoadChangeDiff(change_id)` (on `group="diff"`) — already exists as
  `jj.change_diff`. `RunMutation` kinds `"split"`, `"squash_partial"`.
- **Facade:** `jj.split(repo, source, selected_hunks) -> str`,
  `jj.squash_partial(repo, source, selected_hunks) -> str` (port from
  `backend.rs`). These invoke `jj split` / `jj squash` with the selected hunk
  ranges via interactive stdin or `--restore-descs`/`--interactive` flags —
  the exact mechanism is resolved in phase 1 from the Rust reference.
- **Update:** `Split` → `replace(model, modal="hunk_picker")` +
  `[LoadChangeDiff(selected_change_id)]`. `SquashPartial` → same with a
  "squash mode" flag in the widget. `SplitConfirm` →
  `_start_mutation("split", (source, hunks))`; `SquashPartialConfirm` →
  `_start_mutation("squash_partial", (source, hunks))`.
- **Widget** (`widgets/hunk_picker.py`): modal. Renders file headers (`▸
  <path>  [<selected>/<total>]` cyan bold) + hunks (`[✓]/[ ] <header>`) +
  diff lines (per-line-kind color: added green, removed red, context default,
  header dark gray), cursor reversed, `Rgb(0,40,40)` bg on selected hunks —
  per Rust `hunk_picker.spec.md`. View-local `files/cursor/selected/scroll`.
  Cursor is a flat index over file-headers + hunks (diff lines are not
  cursor-landable, per Rust spec). Keys: `j`/`k` move, `Space` toggle
  selection, `Enter` → commit (`SplitConfirm` or `SquashPartialConfirm`
  depending on mode), `Esc` → `HunkPickerClose`.
- **Bind:** `s` → `Split`, `Ctrl-S` → `SquashPartial`.
- **Tests:** picker render, cursor skips diff lines, toggle selection, commit
  runs correct mutation kind, split and partial-squash transitions through the
  gate, cancel is a no-op.

## Cross-feature consistency

- **Modal lifecycle:** one `modal: str | None` field in Model, projected to a
  `modal` reactive on `LajjzyApp`. Widgets mount based on it. `OpenX` Msgs set
  it; `XCancel`/`XClose` Msgs clear it to `None`. Commit Msgs
  (`OmnibarSubmit`, `BookmarkInputConfirm`, `OpLogRestore`,
  `ApplyResolutions`, `SplitConfirm`) also clear it. Only one modal open at a
  time. Phase 1 defines the field + the open/cancel pattern; phase-2 agents
  use it.
- **Mutation gate:** all write ops (undo, redo, bookmark set/delete/move,
  op-restore, resolve, split, squash-partial) go through `_start_mutation` and
  the `pending_mutation` flag. No new concurrency lanes. The reload-during-
  mutation guard (PR #30) means a user pressing `R` while a split runs is a
  no-op — the mutation's own follow-up reload brings the fresh graph.
- **Working-copy gate:** only conflict-apply uses `ensure_working_copy`. If it
  fails, `error` is set and no mutation starts. The gate is an out-of-band
  async helper on `LajjzyApp`, not a core `Cmd` (per AGENTS.md).
- **Errors:** every new facade function raises `JjError` on failure; workers
  catch and dispatch a `*LoadFailed` or `MutationFailed`/`MutationCompleted(
  load_error=…)` Msg; `update` writes it to `Model.error`. `InvariantError`
  propagates per crash policy.

## Testing strategy

- **Phase 1:** pure unit tests in `tests/core/test_update.py` for every new
  `update` branch — gate behavior, epoch guard, modal lifecycle, data-load
  transitions. These are the contract.
- **Phase 2:** each agent adds integration tests via `app.run_test()` for its
  widget (render states, key handling, end-to-end mutation) and unit tests
  for widget-internal logic (fuzzy match, flat-index navigation, completion
  filter). `jj_required` marker for tests that need a real jj repo.
- **Architecture tests** in `tests/test_architecture.py` already enforce core
  purity, subprocess boundary, and worker exception handling; phase 1 extends
  `test_every_work_worker_has_exception_handling` coverage to the new workers,
  and adds a check that no widget imports `lajjzy.backend.jj` or
  `subprocess`/`asyncio`.

## Risks and mitigations

- **jj CLI surface gaps:** `jj split` / `jj squash` interactive selection may
  not have a clean non-interactive flag. Phase 1 resolves this by reading the
  Rust `backend.rs` implementation; if jj requires a TTY, the facade uses
  `subprocess.run` with stdin piping (within `backend/jj.py`, the one allowed
  subprocess site). Flag in phase-1 PR description if a flag is missing.
- **Conflict parsing:** jj's conflict marker format may differ from the Rust
  prototype's assumption (jj changed conflict format in recent versions).
  Phase 1 verifies against jj 0.42.0 (the CI-pinned version) and adapts the
  parser. If the format is incompatible, conflict view ships as view-only and
  apply lands in a follow-up — but the design assumes full resolver per user
  approval.
- **Phase-1 size:** phase 1 is a large PR (all seams + all facades + 6 stubs
  + unit tests). It's intentionally sequential so phase 2 is contention-free.
  If it's too large to review comfortably, it can be split by layer (core
  seams PR, then facades PR, then stubs PR) — but that's three sequential PRs
  before any feature lands. Default is one phase-1 PR.
- **Keybinding collisions:** `U` (redo) and `Ctrl-S` (partial squash) are
  reassigned from Rust because Python already uses `Ctrl-R` and `S`. Document
  in README keybinding tables when the features ship.

## Out of scope (explicit)

- Help overlay (`?`).
- Mouse support.
- Push/fetch lanes and forge integration.
- Absorb, duplicate, revert.
- Inline describe editor (TextArea).
- Configurable keymaps, theming.
- Any change to the MVU seam, gate, epoch guard, or crash policy.

## Open questions for phase 1

These are resolved during phase-1 implementation, not blocking the design:

1. Does `jj load_graph` already accept a revset, or does the facade need to
   add `--revisions`? (Check `backend/jj.py` current implementation.)
2. Exact `jj` incantation for `conflict_data` and `resolve` against jj 0.42.0.
3. Whether `jj split`/`jj squash` support non-interactive hunk selection via
   flags or need stdin piping.
4. Whether `jj bookmark list` output parses cleanly into `(name, change_id,
   change_description)` or needs a `--template` flag.

All four are phase-1 implementation details; the design is stable regardless
of their answers.
