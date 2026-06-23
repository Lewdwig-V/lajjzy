# Task 7 Report: conflict_data + resolve facades

**Status:** DONE
**Commit:** `c018b02`
**Tests:** 10/10 passed (tests/backend/test_jj_facade_ext.py)

---

## Test Output

```
tests/backend/test_jj_facade_ext.py::test_undo_returns_message PASSED
tests/backend/test_jj_facade_ext.py::test_redo_returns_message PASSED
tests/backend/test_jj_facade_ext.py::test_op_log_returns_entries PASSED
tests/backend/test_jj_facade_ext.py::test_op_restore_roundtrip PASSED
tests/backend/test_jj_facade_ext.py::test_load_bookmarks_empty_repo PASSED
tests/backend/test_jj_facade_ext.py::test_bookmark_set_and_load PASSED
tests/backend/test_jj_facade_ext.py::test_bookmark_delete PASSED
tests/backend/test_jj_facade_ext.py::test_bookmark_move PASSED
tests/backend/test_jj_facade_ext.py::test_conflict_data_no_conflict PASSED
tests/backend/test_jj_facade_ext.py::test_resolve_accept_left PASSED

10 passed in 1.34s
```

Gate: `ruff check`, `ruff format --check`, `mypy src/lajjzy` all clean.
Full suite: 111 passed.

---

## Conflict-Creation Recipe (jj 0.42.0)

```bash
# 1. Create a base commit with the file
jj new -m "base"
echo "LINE" > c.txt
base_id=$(jj log --no-graph -T 'change_id.short(8) ++ "\n"' -r @ | head -1)

# 2. "Left" branch from base
jj new -m "left" "$base_id"
echo "LEFT" > c.txt
left_id=$(jj log --no-graph -T 'change_id.short(8) ++ "\n"' -r @ | head -1)

# 3. "Right" branch from base
jj new -m "right" "$base_id"
echo "RIGHT" > c.txt
right_id=$(jj log --no-graph -T 'change_id.short(8) ++ "\n"' -r @ | head -1)

# 4. Merge commit with both as parents
jj new -m "merge" "$left_id" "$right_id"
# -> @ is now a conflict commit; c.txt contains jj conflict markers
```

---

## Marker Format Findings

### jj 0.42.0 Default (`ui.conflict-marker-style = "diff"`)

```
<<<<<<< conflict 1 of 1
+++++++ <right-id> "right"
RIGHT
%%%%%%% diff from: <base-id> "base"
\\\\\\\        to: <left-id> "left"
-LINE
+LEFT
>>>>>>> conflict 1 of 1 ends
```

**Incompatible** with the 3-way parse model (`left / base / right` sections). Cannot be parsed by `parse_conflict_data`.

### `ui.conflict-marker-style = "snapshot"`

```
<<<<<<< conflict 1 of 1
+++++++ <left-id> "left"
LEFT
------- <base-id> "base"
LINE
+++++++ <right-id> "right"
RIGHT
>>>>>>> conflict 1 of 1 ends
```

Snapshot-style explicitly lists all 3 sides but uses `+++++++` / `-------` / `+++++++` markers, not the 3-way `<<<` / `|||` / `===` / `>>>` convention.

### `ui.conflict-marker-style = "git"` (what we use)

```
<<<<<<< <left-id> "left"
LEFT
||||||| <base-id> "base"
LINE
=======
RIGHT
>>>>>>> <right-id> "right"
```

This is the closest to the traditional 3-way format. It is the **only** format that parse_conflict_data can understand. We force this via `--config ui.conflict-marker-style=git` in the `jj file show` call.

### Parent Ordering

`jj new left_id right_id` puts:
- `left_id` content in the `<<<` section (→ `region.left`)
- base in the `|||` section (→ `region.base`)
- `right_id` content in the `===` section (→ `region.right`)

So `ACCEPT_LEFT` → `region.left` → `"LEFT\n"` is correct.

---

## Deviations from the Brief

### 1. parse_conflict_data updated (EXPLICIT, not silent)

**Brief constraint:** "do NOT silently change the Task 4 parser"

**What changed:** `parse_conflict_data` in `src/lajjzy/backend/parse.py` was updated to use `startswith()` for `<<<<<<<`, `|||||||`, and `>>>>>>>` marker detection instead of exact string equality. The `=======` separator still uses exact equality (jj never appends metadata to it).

**Rationale:** jj 0.42.0 git-style markers include trailing metadata (`<<<<<<< abc1234 "branch"`). The original exact-match logic rejected all markers as non-special, placing the entire file content (including markers) into a single `resolved` region. The fix is minimal, backwards-compatible (bare `<<<<<<<` still matches via `startswith`), and structurally identical in behavior.

**This is explicitly reported, not a silent change.**

### 2. Test setup recipe changed

**Brief recipe** used `jj new --after "@-"` and `--allow-empty` flags that do not exist in jj 0.42.0's `new` subcommand.

**New recipe** uses explicit parent IDs: `jj new -m "merge" left_id right_id`, which creates a true merge commit with both parents. This produces a real 2-sided conflict in the working copy.

### 3. conflict_data uses `--config ui.conflict-marker-style=git`

**Brief** said to call `run_jj(["file", "show", "-r", "@", path], cwd)` without any config override.

**Implementation** adds `--config ui.conflict-marker-style=git` to the argument list. This is required because jj's default diff-style output cannot be parsed by `parse_conflict_data`. This is documented in the function docstring.

---

## Review fix wave

Fixes applied to commit `c018b02` per code review findings.

### Fix 1 (Important) — Tighten conflict-marker detection (`parse.py`)

**Problem:** `parse_conflict_data` used bare `startswith("<<<<<<<")` etc., which false-positives on content lines like `<<<<<<<x` or `|||||||notes`.

**Solution:** Added module-level helper `_is_conflict_marker(stripped, prefix)` that matches only exact 7-char prefix OR prefix followed by a space (e.g. `<<<<<<< Side #1`). Replaced all three directional `startswith(...)` calls and the `=======` separator check (`== "======="`) to use `_is_conflict_marker(...)` instead. Bare-marker tests remain valid since bare prefix still satisfies `stripped == prefix`.

### Fix 2 (Minor) — Remove dead constant (`parse.py`)

**Problem:** `_CONFLICT_MARKERS = ("<<<<<<<", "|||||||", "=======", ">>>>>>>")` was defined at module level but never referenced anywhere.

**Solution:** Removed the tuple entirely. No callers exist inside or outside the module.

### Fix 3 (Minor) — Bounds-check resolutions before the loop (`jj.py`)

**Problem:** `_build_resolved_content` would raise a raw `IndexError` if `resolutions` had fewer entries than conflict regions.

**Solution:** Added a pre-loop guard that counts conflict regions via `sum(1 for r in data.regions if r.kind == "conflict")` and raises `JjError(f"resolve: {n} conflict region(s) but only {len(resolutions)} resolution(s) provided")` if under-supplied. `JjError` was already imported.

### Test output

```
uv run pytest tests/backend/ -v
55 passed in 3.34s
```

Gate: `ruff check`, `ruff format --check`, `mypy src/lajjzy` — all clean.
