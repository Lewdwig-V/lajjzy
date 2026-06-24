# Daily-driver phase 2 — exemplary MVU feature widgets — design

**Date:** 2026-06-24
**Scope:** Implement the six daily-driver feature widgets (omnibar, bookmark
input, bookmark picker, operation log, conflict view, hunk picker) under a
**strict MVU discipline**: the pure `Model` is the *sole* source of truth for
all logic-bearing state; `app.py` projects it and runs effects; widgets are thin
pure projections that render reactives and dispatch `Msg`s.
**Supersedes (for phase 2):** the original daily-driver spec's "view-local
state" decisions (`docs/superpowers/specs/2026-06-22-daily-driver-essentials-design.md`).
That spec remains the reference for **widget rendering details** (colors, titles,
key maps, layout); this spec governs **where state lives and how it flows**.
**Builds on:** phase 1a (`#31`) + phase 1b (`#32`), both merged.

## Goal

Make the six features fully functional while keeping the MVU ↔ rendering-backend
separation **exemplary** — no split-brain. Every piece of state that decides
*what an action does* (cursors, query text, selections, resolution choices,
loaded data) lives in the `Model`, is mutated only by `update` via `Msg`s, and is
unit-testable in the pure core. The backend/widget layer holds only non-logical
render mechanics (Textual focus, scroll offset for display).

## Non-goals

- No new concurrency lanes; effects run on the existing worker groups.
- No push/fetch, forge, mouse, theming (roadmap depth).
- No change to the crash policy, the `pending_mutation` mutation gate, or the
  epoch guard.

## The split-brain it eliminates

Two split-brain seeds exist in merged code and are removed here:

1. **`DetailPanel` holds logic state outside the Model** — `file_cursor`,
   `mode` ("files"/"diff"), and the loaded `diff` (fetched by the view-local
   `open_diff` worker, stored on the widget). Phase 2a moves all of it into the
   Model.
2. **Flat modal fields representable in invalid combinations** — 1a's
   `modal: str` plus `op_log_entries`/`bookmarks`/`conflict_data`/`conflict_path`
   allow states like "op-log cursor set while omnibar open" and "stale
   `conflict_data` after close" (the latter was a real Codex finding). Phase 2b
   replaces them with a tagged union where state lifetime equals modal lifetime.

## Model shape

```python
# top-level (persistent app state)
graph, cursor, error, rebase_source, rebase_descendants, pending_mutation, graph_epoch
revset: str | None              # active graph filter — persists across modal open/close
detail: DetailState             # the always-present detail pane (phase 2a)
modal: ModalState | None        # the active modal, or None (phase 2b)

@dataclass(frozen=True, slots=True)
class DetailState:               # replaces DetailPanel's widget-held state
    file_cursor: int = 0
    mode: Literal["files", "diff"] = "files"
    diff: list[FileDiff] | None = None   # loaded through MVU; None until ChangeDiffLoaded

# modal: a TAGGED UNION. Each variant owns ITS cursor/query/selection AND its
# modal-scoped loaded data. `data: X | None` models the "open, still loading" instant.
@dataclass(frozen=True, slots=True)
class OmnibarState:
    query: str = ""
    completion_cursor: int = 0

@dataclass(frozen=True, slots=True)
class OpLogState:
    entries: list[OpLogEntry] | None = None
    cursor: int = 0

@dataclass(frozen=True, slots=True)
class BookmarkInputState:
    text: str = ""
    completion_cursor: int = 0
    # existing bookmark names (for completion) loaded with the modal
    names: list[str] | None = None

@dataclass(frozen=True, slots=True)
class BookmarkPickerState:
    bookmarks: list[Bookmark] | None = None
    cursor: int = 0
    picking_destination_for: str | None = None   # bookmark name being moved, or None

@dataclass(frozen=True, slots=True)
class ConflictViewState:
    path: str
    data: ConflictData | None = None
    cursor: int = 0                               # conflict-hunk index
    resolutions: tuple[HunkResolution, ...] = ()  # one per conflict hunk

@dataclass(frozen=True, slots=True)
class HunkPickerState:
    source: str
    op: Literal["split", "squash_partial"]
    diff: list[FileDiff] | None = None
    cursor: int = 0                               # flat index over file headers + hunks
    selected: frozenset[FileRef] = frozenset()

ModalState = (
    OmnibarState | OpLogState | BookmarkInputState
    | BookmarkPickerState | ConflictViewState | HunkPickerState
)
```

This **removes** 1a's `modal: str` and the flat `op_log_entries` / `bookmarks` /
`conflict_data` / `conflict_path` fields (folded into the variants). `revset`
stays top-level (a filter, not modal-scoped). `detail.diff` lives in
`DetailState`, not a modal variant, because the detail pane is always present.

**Why folding modal data in:** it ties data lifetime to modal lifetime (a load
returning after close is dropped, not orphaned) and makes invalid combinations
unrepresentable — eliminating the stale-`conflict_data` bug class structurally.
The cost is a type-narrow + nested `replace` in each `*Loaded` handler; the
narrowing's `else: drop` branch is the desired stale-load behavior.

## Data flow

- **Diff through MVU (2a).** `DetailOpenFile` on a non-conflicted file →
  `update` sets `detail.mode="diff"` and emits `LoadChangeDiff(change_id)` → a
  worker runs `jj.change_diff` → `ChangeDiffLoaded(diff)` → `update` stores it in
  `detail.diff`. This replaces the view-local `open_diff` worker. The hunk
  picker reuses `LoadChangeDiff` to populate `HunkPickerState.diff`. A
  `ChangeDiffLoaded` whose target no longer matches the current view is dropped
  (the selection/epoch is the guard — exact guard chosen in the 2a plan).
- **Loaded data folds into modal state (2b).** `OpenOpLog` →
  `modal = OpLogState()` + `[LoadOpLog]`; `OpLogLoaded(entries)` → if `modal` is
  an `OpLogState`, replace its `entries`, else drop. Same pattern for the
  picker's bookmarks, the conflict data, the bookmark-input names, and the
  hunk-picker diff.
- **Derived data stays derived (2b).** Omnibar completions (hardcoded
  revset-function list filtered by `query`) and fuzzy matches (over `graph`) are
  **pure functions in `core`** computed on demand by the widget — never stored,
  so they cannot desync. They also bound `completion_cursor` in `update`.
- **Filter submit.** `OmnibarSubmit(revset)` writes top-level `model.revset` and
  reloads (the existing 1a/1b behavior, including the pending-mutation guard and
  revset-on-reload threading); the omnibar query itself is discarded with the
  modal.

## Messages (2b)

- **Generic modal nav:** `ModalCursorUp` / `ModalCursorDown` move *the active
  modal's* cursor (serves op-log, bookmark-picker, conflict-view, hunk-picker
  uniformly; `update` narrows on the variant and clamps to that variant's item
  count). Variants without a list (omnibar, bookmark-input) ignore them.
- **Omnibar:** `OmnibarInput(char)`, `OmnibarBackspace`,
  `OmnibarAcceptCompletion`, `OmnibarSubmit(revset)`, `OmnibarCancel` — these
  now have real `update` branches that maintain `OmnibarState` (in 1a they were
  routed widget-locally; 2b moves them into the core).
- **Bookmark input:** `BookmarkInputChar(char)`, `BookmarkInputBackspace`,
  `BookmarkInputConfirm(name)`, `BookmarkInputCancel`.
- **Bookmark picker:** delete/move/jump — `BookmarkDelete`, `BookmarkMove`
  (enters `picking_destination_for` mode), `BookmarkMoveConfirm`,
  `JumpToChange(change_id)` (Enter jumps the graph cursor to the bookmarked
  change), close.
- **Conflict view:** `ConflictPickLeft`, `ConflictPickRight` (set
  `resolutions[cursor]`), `ApplyResolutions`, `ConflictViewClose`.
- **Hunk picker:** `HunkToggleSelection`, `SplitConfirm`,
  `SquashPartialConfirm`, `HunkPickerClose`.
- Commit Msgs already exist from 1a (`OpLogRestore`, `SplitConfirm`,
  `ApplyResolutions`, …) and are reused.

Every branch is covered by a pure unit test in `tests/core/test_update.py`.

## Conflict surfacing (2c)

jj 0.42.0 does not emit `C <path>` in `jj log --summary` for merge commits
(conflicts are inherited from parents, not recorded as changes on the merge), so
conflicted files never reach the detail pane today — `Enter → OpenConflictView`
is wired but unreachable. 2c adds a facade query (`jj resolve --list`, exact
invocation verified against 0.42.0 in the 2c plan) that returns the conflicted
paths for a change; the loader merges them into the change's file list (marked
`FileStatus.CONFLICTED`) so they appear in the detail pane and the routing works
end-to-end. If the jj surface proves unworkable, conflict view ships view-only
behind whatever surfacing is achievable, flagged in the 2c PR.

## Widgets (2c)

Each widget becomes a thin pure projection:

- Reads its state off the app reactives (`app.modal` narrowed to its variant,
  `app.detail`, `app.graph`, `app.revset`); renders per the original
  daily-driver spec's per-feature sections (titles, colors, layout, key maps).
- Holds **no logic-bearing state** — only render mechanics (scroll offset
  derived from the cursor; Textual focus).
- Dispatches `Msg`s on key presses; never calls the jj facade or `subprocess`
  (enforced by the existing `test_widgets_do_not_import_jj_facade_or_subprocess`
  AST guard).
- `watch_modal` gains **focus glue**: when a modal opens, focus its widget so its
  focus-scoped `BINDINGS` receive keys; when it closes, focus returns to
  `GraphView`. (Focus is render mechanics, not logic state.)

## Sub-phase split

- **2a — foundation.** `DetailState` in the Model; `LoadChangeDiff` Cmd +
  `ChangeDiffLoaded` Msg + worker; `DetailFileUp/Down`, `DetailOpenFile`,
  `DetailBack` Msgs + `update` branches; `DetailPanel` refactored to a pure
  projection (reads `app.detail`, dispatches Msgs); the view-local `open_diff`
  worker removed. Conflicted-file `Enter` still routes to `OpenConflictView`.
- **2b — modal core.** The `ModalState` tagged union replaces `modal: str` +
  the flat fields; per-variant state, all modal `Msg`s + `update` branches +
  pure derived helpers (omnibar completions/matches), fully unit-tested. 1a/1b
  code and tests that referenced the old shape are migrated. Widgets stay stubs
  (updated only enough to compile against the new reactive shape).
- **2c — widgets + surfacing.** The six widgets filled in as pure projections;
  `watch_modal` focus glue; conflict surfacing. End-to-end pilot tests.

## Migration cost

2b refactors already-merged 1a/1b code (the `modal` reactive and projection, the
removed flat fields, the workers that dispatched `*Loaded` Msgs, and their
tests). This is the deliberate price of exemplary separation: the tagged union
makes the split-brain class impossible by construction rather than by discipline.

## Testing strategy

- **2a/2b:** pure unit tests in `tests/core/test_update.py` for every new
  `update` branch (cursor clamping per variant, load-folds-into-state, stale-load
  drop, omnibar query maintenance, derived-helper correctness). Property tests
  where a strategy fits (e.g. cursor stays in range under random nav).
- **2c:** `app.run_test()` pilot tests per widget (open → render → key → effect),
  against real jj 0.42.0 (`@jj_required`).
- **Architecture guards extended:** the core-purity and widget-purity AST tests
  already exist; 2b keeps `core/` import-pure, 2c keeps widgets facade-free.

## Risks

- **Union narrowing churn** in `update` and `present` — mitigated by `match`
  statements and that mypy `--strict` enforces exhaustiveness over the union.
- **Conflict surfacing** may need a jj invocation not yet identified — 2c
  verifies against 0.42.0 and degrades to view-only if necessary.
- **2b touches merged code** — broad but mechanical; the full suite + mypy guard
  the migration.
