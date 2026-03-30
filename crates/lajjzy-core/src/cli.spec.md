---
managed-file: crates/lajjzy-core/src/cli.rs
intent: >
  JjCliBackend implements the RepoBackend trait by shelling out to the jj CLI
  for all repository operations. It validates the workspace on construction,
  loads the full change graph via jj log with a custom template, lazily
  computes diffs via jj diff, reads structured conflict data by parsing jj
  conflict markers from jj file show, and executes all mutations (describe,
  new, edit, abandon, undo, redo, bookmark, push, fetch, rebase, split,
  squash, absorb, duplicate, revert, resolve-file) via jj CLI subcommands.
  All errors propagate as Err; the backend never panics on repository
  operations.
intent-approved: false
intent-hash: 2d8330845aba
distilled-from:
  - path: crates/lajjzy-core/src/cli.rs
    hash: 851d5ba6fda7
non-goals:
  - Does not use jj-lib for any operations — all jj interaction is through the CLI subprocess interface
  - Does not cache jj subprocess output between calls — every method re-shells out
  - Does not perform partial-hunk selection at the diff-line level — split and squash operate on whole files, not individual diff hunks
  - Does not manage terminal state, process lifecycle, or git remotes — those are the responsibility of lajjzy-cli and the user's jj config
depends-on:
  - crates/lajjzy-core/src/backend.spec.md
  - crates/lajjzy-core/src/types.spec.md
spec-changelog:
  - intent-hash: 2d8330845aba
    timestamp: 2026-03-31T00:00:00Z
    operation: elicit-amend
    prior-intent-hash: 140f85271d86
---

## Purpose

`JjCliBackend` is the production implementation of `RepoBackend`. All jj
interaction goes through CLI subprocess calls — no jj-lib dependency.
Callers obtain a `GraphData` snapshot from `load_graph`, lazily drill into
diffs via `file_diff` / `change_diff`, inspect conflicts via `conflict_sides`,
and mutate the repository through typed methods that each return a
human-readable `String` on success or an `Err` on failure.

## Behavior

### Construction

- `JjCliBackend::new(path)` runs `jj root` in `path`; returns `Err` if `jj` is
  not installed or if `path` is not inside a jj workspace.
- On success the resolved workspace root (as reported by `jj root`) is stored
  as a `PathBuf`. Subsequent subprocess calls use this root as `current_dir`.
- No jj-lib workspace or repo handle is opened.

### Graph loading — `load_graph(revset)`

- Invokes `jj log --summary --color=never -T <template>` once per call.
- Captures the current operation id via `jj op log --limit=1` before the log
  call; the id is stored in `GraphData.op_id`.
- If `revset` is `Some`, passes `-r <revset>` to `jj log`; an invalid revset
  returns `Err`.
- Parses output into `GraphLine` entries:
  - Lines containing the `\x1F` unit-separator are change-node lines. Each
    carries 11 `\x1E`-delimited metadata fields (change_id, commit_id, author,
    email, timestamp, description, bookmarks, is_empty, has_conflict,
    is_working_copy, parent_ids).
  - Continuation lines beginning with `A `, `M `, `D `, `R `, `C `, or any
    other uppercase letter + space are file-change lines; they are compacted
    into the preceding change's `ChangeDetail.files` list and do NOT appear as
    separate `GraphLine` entries.
  - All other lines (connector glyphs, blank lines) are emitted as `GraphLine`
    entries with `change_id: None`.
- After parsing, files within each `ChangeDetail` are sorted: Conflicted first,
  then Modified, Added, Deleted, Renamed, Unknown; ties broken alphabetically.
- `ChangeDetail.conflict_count` is recomputed from the actual count of
  `FileStatus::Conflicted` files (overrides the template boolean).
- Returns `Err` if the output is non-empty but yields zero change nodes
  (template format mismatch guard).
- Returns `Err` if any short change id appears more than once (truncation
  collision guard).

### Diff loading — `file_diff(change_id, path)`

- Invokes `jj diff -r <change_id> --git --color=never <path>`.
- Parses git-format diff output into `DiffHunk` values, each with a header
  (`@@ … @@`) and typed `DiffLine` entries (`Added`, `Removed`, `Context`).
- Header-only diffs (chmod-only, binary, pure rename — no `@@` hunks) produce
  a single synthetic hunk whose lines are all typed `Header`.

### Diff loading — `change_diff(change_id)`

- Invokes `jj diff -r <change_id> --git --color=never` (no path filter).
- Parses the same git-format output, splitting on `diff --git a/… b/…` lines
  into per-file `FileDiff` entries. Each entry includes the file path and its
  hunks. Header-only files produce a synthetic hunk.

### Operation log — `op_log()`

- Invokes `jj op log --no-graph --color=never -T <template>`.
- Returns a `Vec<OpLogEntry>` with id (8-char short), description, and
  relative timestamp.
- Returns `Err` if the output is non-empty but yields zero entries.

### Simple mutations (each runs a single `jj` subprocess)

| Method | jj command | Return string |
|--------|-----------|---------------|
| `describe(change_id, text)` | `jj describe <id> -m <text>` | `Described <id>: "<first 50 chars>"` |
| `new_change(after)` | `jj new --insert-after <after>` | `Created new change after <after>` |
| `edit_change(change_id)` | `jj edit <id>` | `Now editing <id>` |
| `abandon(change_id)` | `jj abandon <id>` | `Abandoned <id>` |
| `undo()` | `jj undo` | `Undid last operation` |
| `redo()` | `jj redo` | `Redid last operation` |
| `bookmark_set(change_id, name)` | `jj bookmark set <name> -r <id>` | `Set bookmark "<name>" on <id>` |
| `bookmark_delete(name)` | `jj bookmark delete <name>` | `Deleted bookmark "<name>"` |
| `git_push(bookmark)` | `jj git push --bookmark <bookmark>` | `Pushed <bookmark>` |
| `git_fetch()` | `jj git fetch` | `Fetched from remote` |
| `rebase_single(src, dst)` | `jj rebase -r <src> --onto <dst>` | `Rebased <src> onto <dst>` |
| `rebase_with_descendants(src, dst)` | `jj rebase -s <src> --onto <dst>` | `Rebased <src> + descendants onto <dst>` |
| `absorb(change_id)` | `jj absorb --from <id>` | jj stderr/stdout or `Absorbed changes from <id>` |

- For all `run_jj` calls: jj's human-readable feedback arrives on stderr;
  stderr is preferred over stdout when non-empty. On failure, the error text
  from stderr (or stdout if stderr is empty) is returned as the `Err` message.

### `new_change` stack-insertion semantics

- Uses `--insert-after` so new changes are inserted into the stack, not forked.
  Existing descendants of `after` are reparented onto the new change.

### `split(change_id, selections)`

- A file is "fully selected" when `selected_hunks.len() == total_hunks`.
- Files NOT fully selected are passed to `jj split -r <id> -m "" -- <paths>`;
  those files remain in the original change. Fully-selected files move to the
  new child change.
- Returns `Err("Cannot split: all files are fully selected …")` if no files
  would remain in the original.
- File paths are sorted before being passed to jj (determinism).
- Uses `-m ""` to suppress `$EDITOR` for the original commit description.

### `squash_partial(change_id, selections)`

- Any file with at least one selected hunk is passed to
  `jj squash -r <id> -u -- <paths>`; those files move to the parent.
- Returns `Err("No files selected for squash")` if no file has selected hunks.
- Uses `-u` (`--use-destination-message`) to suppress `$EDITOR`.

### `duplicate(change_id)` — CLI subprocess

- Invokes `jj duplicate <change_id>`.
- Parses jj's output to extract the new change ID.
- Returns `Duplicated <change_id>` on success.

### `revert(change_id)` — CLI subprocess

- Invokes `jj revert --revisions <change_id> --onto @`.
- Returns `Reverted <change_id>` on success.
- If the resulting commit has conflicts, the return string includes
  `(new commit has conflicts)`.

### `conflict_sides(change_id, path)` — parses jj conflict markers

- Invokes `jj file show -r <change_id> <path>`.
- If the file is not conflicted (no conflict markers in output), returns `Err`.
- Parses jj's conflict marker format to extract structured regions:
  - `<<<<<<<` opens a conflict block
  - `%%%%%%%` introduces a diff-style section (base with +/- patches for left side)
  - `+++++++` introduces the right side verbatim content
  - `>>>>>>>` closes the conflict block
  - Content between conflict blocks is `ConflictRegion::Resolved`
  - Each conflict block produces `ConflictRegion::Conflict { base, left, right }`
- Returns `Err` for n-way conflicts (> 2 sides) or binary/non-UTF-8 files.
- Returns `ConflictData` with regions interleaved in file order.

### `resolve_file(change_id, path, content)`

- Defense-in-depth: verifies `change_id` equals the working-copy change id by
  running `jj log -r @ -T change_id.short()`. Returns `Err` if they differ.
- Writes `content` to `workspace_root / path` via `std::fs::write`.
- Returns `Resolved <path>` on success.

## Constraints

- All public methods return `Result`; no `unwrap` or panic on repository errors.
- `JjCliBackend` does not implement caching — each call reflects the current
  repo state.
- Short change ids from `change_id.short()` are in jj's reverse-hex alphabet
  (k–z + a–j).
- `first_line_preview` truncates descriptions to 50 characters using
  `char_indices` (safe for multibyte UTF-8); truncated strings are suffixed
  with `...`.
- File-change sort order: Conflicted(0) < Modified(1) < Added(2) < Deleted(3)
  < Renamed(4) < Unknown(5); ties broken alphabetically by path.

## Dependencies

- `jj` CLI binary in `PATH` — required at construction time and for all operations.
- `crate::backend::RepoBackend` — trait being implemented.
- `crate::types::{GraphData, GraphLine, ChangeDetail, FileChange, FileStatus,
  DiffHunk, DiffLine, DiffLineKind, FileDiff, OpLogEntry, ConflictData,
  ConflictRegion, FileHunkSelection}` — all domain types.
- `anyhow` — error propagation.

## Changelog

- **2026-03-31 elicit-amend** — M8: migrate duplicate, revert, conflict_sides from jj-lib to pure CLI. Drop jj-lib dependency. Constructor simplified to store PathBuf only.
