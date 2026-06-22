# Invariant guardrails — design

**Status:** Design approved, pending implementation plan
**Date:** 2026-06-20
**Branch:** follows the Python reboot (`reboot/python-textual` / `main` after merge)

## Why

A Codex review caught a P1 concurrency bug (`@work(exclusive=True)` cancels an
in-flight mutation rather than rejecting a new one — it does **not** kill the
already-spawned `jj` subprocess, so rapid keypresses could drop the worker that
reloads/reports while the repo mutation still lands). The earlier design reasoning
had wrongly claimed `exclusive=True` *was* the mutation gate. Nothing mechanical
would have caught the mistake: there is no type checker, the dataclasses are
mutable and unvalidated, the facade boundary is convention-only, and the
concurrency invariants live as runtime logic with no assertions.

Goal: put guardrails in place so **hard invariants are enforced**, **invalid
states are unrepresentable**, and **this class of miss cannot silently recur**.

## Decisions (locked)

| Decision | Choice |
|---|---|
| Layers | Type-level unrepresentability • Runtime invariant assertions • Architectural & property tests (no standalone registry doc — invariants documented inline + a short table in CLAUDE.md) |
| Type checker | `mypy --strict`, gating CI |
| Assertion style | Explicit `if not cond: raise InvariantError(...)` (survives `python -O`); bare `assert` only for debug-only redundant checks |
| Property tests | `hypothesis` (dev dep) |
| Failure policy | Crash hard on internal-invariant breach via a clean top-level handler; repo state is safe on disk (jj op log) |
| Invariant helper | One central `invariant(cond, msg)` helper (greppable, uniform) over scattered raises |

## The invariant catalogue

| # | Invariant | Class | Primary enforcement |
|---|---|---|---|
| I1 | At most one mutation in flight | internal | runtime `invariant()` at the gate + arch test (worker not `exclusive`) + existing deterministic gate tests |
| I2 | `GraphData` consistent: `node_indices` derived from `lines`; `working_copy_index` in range and on a node; `details` ↔ `lines` referential integrity | data | construction-time validation (`__post_init__`, frozen type) + hypothesis property test |
| I3 | Cursor always lands on a node line, never a connector | internal | runtime `invariant()` after cursor moves + hypothesis property test |
| I4 | Only `backend/jj.py` spawns a `jj` subprocess (sole exception: `app.py` `$EDITOR` launch) | architectural | AST arch test |
| I5 | Every `@work` worker contains exception handling (no silent worker death) | architectural | AST arch test |
| I6 | Backend public async fns raise only `JjError` | architectural | AST arch test + existing wrapping |
| I7 | `DiffLine.kind` / `DetailPanel.mode` are valid literals | type | `Literal` + mypy |
| I8 | Stale reloads never overwrite a fresher graph | internal | epoch monotonic (shipped) + deterministic test |

## Layer 1 — Type-level (invalid states unrepresentable)

- **`mypy --strict`**: add to `[dependency-groups] dev`, a `[tool.mypy]` section
  (strict, `files = ["src/lajjzy"]`), and a CI step. **Biggest single effort** —
  strict on a Textual codebase surfaces real work, especially around `reactive()`
  typing. The plan ramps it: annotate + fix `src/lajjzy` to zero errors *before*
  the CI step becomes a hard gate; tests may run at a looser setting initially.
- **Freeze the domain types**: `@dataclass(frozen=True, slots=True)` on
  `GraphData`, `GraphLine`, `ChangeDetail`, `FileChange`, `FileDiff`, `DiffHunk`,
  `DiffLine`. Immutable values can't drift after construction.
- **`GraphData.node_indices` becomes a `@cached_property`** derived from `lines`
  (no stored field, not a constructor arg) — eliminates the "caller supplies a
  wrong/stale list" bypass and makes the frozen dataclass clean. `change_id_at`
  stays.
- Keep existing enums / `Literal`s (I7); mypy turns them from decorative into
  enforced.

> Note: `NewType` change-IDs were considered and cut (YAGNI — high churn, modest
> gain over the existing validation).

## Layer 2 — Runtime invariant assertions (crash on breach)

- **`src/lajjzy/invariants.py`**: `class InvariantError(Exception)` and
  `def invariant(cond: bool, msg: str) -> None: ` raising `InvariantError(msg)`
  when `not cond`. Explicit raise — not stripped under `-O`.
- **Internal vs external split (load-bearing):**
  - *Model/state breaches* (I1, I3) → `invariant()` → `InvariantError` → crash.
  - *Data-shape breaches* (I2) → `ValueError` in `GraphData.__post_init__`;
    `load_graph`/`change_diff` already wrap `ValueError → JjError`, so
    parser/template-drift stays a graceful user-facing error.
  - *User/jj errors* → `JjError → self.error` (unchanged).
- **Crash wiring:** in each `@work` worker, order the handlers
  `except JjError:` (report) → `except InvariantError: raise` (let it crash) →
  `except Exception:` (report). `@work`'s default `exit_on_error=True` tears the
  app down when `InvariantError` propagates. `main()` wraps `app.run()`: on
  `InvariantError`, ensure the terminal is restored, print the violated invariant
  + a "please report this bug" hint to stderr, and `sys.exit(70)`.
- **Assertion sites:** mutation gate (`_run_mutation` asserts `pending_mutation`
  was set on entry — exactly one worker); after each cursor mutation
  (`cursor ∈ node_indices`, or graph empty).

## Layer 3 — Architectural & property tests (codify rules in CI)

- **`tests/test_architecture.py`** (stdlib `ast`, no new deps):
  - I4: walk every `src/lajjzy/**/*.py`; the only `subprocess` /
    `asyncio.create_subprocess_exec` call sites allowed are in `backend/jj.py`
    and the single `app.py` `$EDITOR` launch. Anything else fails.
  - I6: the `_run_mutation` `@work` decorator must not pass `exclusive=` (this is
    the test that would have caught P1). Optionally assert mutation group spelling.
  - I5: every method decorated `@work` has a `try` with an `except` in its body.
  - parse.py purity: no `import subprocess` / `asyncio` / file I/O in `parse.py`.
- **`tests/test_properties.py`** (`hypothesis`):
  - I2: strategies build arbitrary `lines`/`details`; constructing `GraphData`
    either raises (inconsistent input) or yields a value satisfying every I2
    clause (`node_indices` correct, `working_copy_index` valid, referential
    integrity).
  - I3: given an arbitrary graph and an arbitrary sequence of nav actions
    (down/up/top/bottom), the cursor is always on a node line.
  - Concurrency (I1/I8) stays as the existing deterministic gate/epoch tests +
    the arch test — fuzzing async timing reliably is not worth the flakiness.

## Layer 4 — CLAUDE.md invariant table (lightweight no-repeat thread)

Add a short `## Invariants` section to `CLAUDE.md` listing I1–I8 and, for each,
the mechanism that enforces it. Convention: adding a new hard invariant means
adding its row **and** its enforcing check. Not a separate registry doc — just
the one table where agents already look.

## CI

Extend `.github/workflows/ci.yml`: add `uv run mypy src/lajjzy` (after the ramp
reaches zero errors) alongside the existing `ruff check`, `ruff format --check`,
and `pytest` steps. `hypothesis` and `mypy` join the `dev` dependency group.

## Out of scope (explicitly)

- `NewType`/value-object wrappers for IDs (YAGNI for now).
- Property-fuzzing async worker timing (flaky; covered by deterministic tests).
- A standalone invariant-registry document (replaced by the CLAUDE.md table).
- icontract/deal-style decorator DbC (the central `invariant()` helper covers the
  need without a new dependency).

## Sequencing (for the plan)

1. `invariants.py` (`InvariantError` + `invariant`) + crash wiring in workers and
   `main()`. (Self-contained; no type-churn.)
2. Freeze domain types + `node_indices` → `cached_property` + extend `GraphData`
   validation (I2). Update construction/consumers.
3. Runtime `invariant()` sites (I1, I3).
4. `tests/test_architecture.py` (I4/I5/I6/purity).
5. `tests/test_properties.py` (I2/I3) + `hypothesis` dep.
6. `mypy --strict` ramp: add config + deps, annotate/fix `src/lajjzy` to zero,
   then add the CI gate.
7. CLAUDE.md `## Invariants` table + CI mypy step.
