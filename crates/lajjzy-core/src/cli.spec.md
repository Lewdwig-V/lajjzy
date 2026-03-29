---
managed-file: crates/lajjzy-core/src/cli.rs
intent: >
  JjCliBackend implements the RepoBackend trait by shelling out to the jj CLI (and selectively using jj-lib for in-process operations). It validates the workspace on construction, loads the full change graph with per-change metadata and file summaries in a single jj log invocation, lazily computes file-level and change-level diffs, reads structured conflict data via jj-lib, and executes all repository mutations (describe, new, edit, abandon, undo, redo, bookmark set/delete, git push/fetch, rebase single, rebase with descendants, split, squash-partial, absorb, duplicate, revert, and resolve-file) returning human-readable confirmation strings. All errors propagate as Err; the backend never panics on repository operations.
intent-approved: false
intent-hash: 140f85271d86
distilled-from:
  - path: crates/lajjzy-core/src/cli.rs
    hash: 851d5ba6fda7
non-goals:
  - Does not cache jj subprocess output between calls — every method re-opens the workspace or re-shells out
  - Does not perform partial-hunk selection at the diff-line level — split and squash operate on whole files, not individual diff hunks
  - Does not manage terminal state, process lifecycle, or git remotes — those are the responsibility of lajjzy-cli and the user's jj config
depends-on:
  - crates/lajjzy-core/src/backend.spec.md
  - crates/lajjzy-core/src/types.spec.md
---

## Purpose

`JjCliBackend` is the production implementation of `RepoBackend`. Callers obtain
a `GraphData` snapshot (change graph, per-change metadata, file lists) from
`load_graph`, lazily drill into diffs via `file_diff` / `change_diff`, inspect
conflicts via `conflict_sides`, and mutate the repository through a set of
typed methods that each return a human-readable `String` on success or an
`Err` on failure.

## Behavior

### Construction

- `JjCliBackend::new(path)` runs `jj root` in `path`; returns `Err` if `jj` is
  not installed or if `path` is not inside a jj workspace.
- On success the resolved workspace root (as reported by `jj root`) is stored.
  Subsequent subprocess calls use this root as `current_dir`.

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

### `duplicate(change_id)` — uses jj-lib in-process

- Opens the jj-lib workspace at head (re-opened per call).
- Resolves the short change id via `resolve_change` (reverse-hex prefix lookup,
  fails if not found, ambiguous, or divergent).
- Creates a transaction, calls `duplicate_commits_onto_parents`, commits.
- Returns `Duplicated <change_id> → <new_change_id>` (new id in full hex).

### `revert(change_id)` — uses jj-lib in-process

- Opens the jj-lib workspace at head.
- Computes `merge(wc_tree, parent_tree, commit_tree)` (inverse-apply semantics).
- Creates a new commit as a child of `@` with description `revert <change_id>`.
- If the resulting tree has conflicts, the return string includes
  `(new commit has conflicts)`.

### `conflict_sides(change_id, path)` — uses jj-lib in-process

- Opens the jj-lib repo at head; resolves the change id.
- Returns `Err` if the file is not conflicted, is an n-way conflict (> 2
  sides), or is a non-file entry.
- Returns `Err` with a user-facing message advising an external tool for
  binary / non-UTF-8 files.
- Returns `ConflictData` with `ConflictRegion::Resolved` and
  `ConflictRegion::Conflict { base, left, right }` regions interleaved in
  file order.

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
  (k–z + a–j); internal resolution uses `try_from_reverse_hex`.
- The jj-lib workspace is re-opened from disk on every call that uses it
  (`duplicate`, `revert`, `conflict_sides`) to pick up CLI mutation effects.
- `first_line_preview` truncates descriptions to 50 characters using
  `char_indices` (safe for multibyte UTF-8); truncated strings are suffixed
  with `...`.
- File-change sort order: Conflicted(0) < Modified(1) < Added(2) < Deleted(3)
  < Renamed(4) < Unknown(5); ties broken alphabetically by path.

## Dependencies

- `jj` CLI binary in `PATH` — required at construction time and for all CLI
  mutations and graph/diff loading.
- `jj-lib` crate — used in-process for `duplicate`, `revert`,
  `conflict_sides`; loaded with user config from
  `~/.config/jj/config.toml` when present.
- `pollster` — blocks async jj-lib futures synchronously.
- `dirs` — resolves user config directory for jj-lib user settings.
- `crate::backend::RepoBackend` — trait being implemented.
- `crate::types::{GraphData, GraphLine, ChangeDetail, FileChange, FileStatus,
  DiffHunk, DiffLine, DiffLineKind, FileDiff, OpLogEntry, ConflictData,
  ConflictRegion, FileHunkSelection}` — all domain types.
- `anyhow` — error propagation.
