# Final Review Wave — Fix Report

## Fix A — `ApplyResolutions` clears `conflict_path`/`conflict_data`

**File:** `src/lajjzy/core/update.py`

Changed `_start_mutation(replace(model, modal=None), ...)` in the `ApplyResolutions` branch to also clear the conflict state:

```python
replace(model, modal=None, conflict_path=None, conflict_data=None)
```

Previously `conflict_path` and `conflict_data` were left populated after the modal closed.

---

## Fix B — Document `BookmarkMove` as widget-local

**File:** `src/lajjzy/core/update.py`

Added comment in the bookmarks section (before `OpenBookmarkSet`), matching the style of the existing omnibar widget-local comment:

```python
# BookmarkMove is handled widget-locally (the picker flips into destination-pick mode and later dispatches BookmarkMoveConfirm); no core branch.
```

`BookmarkMove` remains in the `Msg` union — only the comment was added.

---

## Fix C — `ConflictRegion.kind` narrowed to `Literal`

**File:** `src/lajjzy/backend/types.py`

Changed `kind: str` to `kind: Literal["resolved", "conflict"]` in the `ConflictRegion` dataclass. `Literal` was already imported on line 6.

---

## Fix D — Strengthen `test_hunk_resolution_values`

**File:** `tests/backend/test_parse_ext.py`

Replaced tautological `is not None` checks with concrete value assertions:

```python
assert HunkResolution.NONE == "none"
assert HunkResolution.ACCEPT_LEFT == "accept_left"
assert HunkResolution.ACCEPT_RIGHT == "accept_right"
assert HunkResolution.NONE != HunkResolution.ACCEPT_LEFT
```

---

## Fix E — Add `modal is None` assertion to `test_squash_partial_confirm_starts_mutation`

**File:** `tests/core/test_update.py`

Added `assert confirmed.modal is None` after `assert confirmed.pending_mutation is True`, mirroring the `SplitConfirm` sibling test.

---

## Fix F — Strengthen `test_op_restore_roundtrip`

**File:** `tests/backend/test_jj_facade_ext.py`

Replaced weak GraphData isinstance check with a pre/post node count comparison:

1. Load graph before change → capture `node_count_before`
2. Capture `op_id` from op log
3. Make `new_change`
4. Call `op_restore` with captured `op_id`
5. Assert `len(graph.node_indices) <= node_count_before`

---

## Fix G — Add `test_parse_op_log_skips_malformed_line`

**File:** `tests/backend/test_parse_ext.py`

New test verifying that `parse_op_log` silently skips lines with wrong field count:

- Input: `"abc\x1fnow\x1fdesc\nMALFORMED_NO_SEPARATORS\ndef\x1flater\x1fother\n"`
- Expected: 2 entries (malformed middle line skipped)

`parse_op_log` already had the `if len(parts) != 3: continue` guard — this test locks it down.

---

## Fix H — Add `test_bookmark_input_confirm_no_change_selected_sets_error`

**File:** `tests/core/test_update.py`

New test verifying that `BookmarkInputConfirm` with no change selected:
- Sets `error="No change selected"`
- Leaves `modal=None`
- Starts NO mutation (no `RunMutation` cmd)

Uses `Model()` (no graph) which causes `selected_change_id` to return `None`.

---

## Fix I — Add `test_core_modules_are_pure`

**File:** `tests/test_architecture.py`

New AST-based test that walks every `.py` file under `src/lajjzy/core/`, parses with `ast`, and asserts no `Import`/`ImportFrom` nodes reference:
- `textual`, `asyncio`, `subprocess`, `os` (top-level module check)
- `lajjzy.backend.jj` (exact full module check — jj facade forbidden, pure types allowed)

Modelled after the existing `test_parse_module_is_pure`.

---

## Test Output

```
154 passed in 7.90s
```

## Static Analysis

- `uv run ruff check .` — All checks passed
- `uv run ruff format --check .` — 36 files already formatted
- `uv run mypy src/lajjzy` — Success: no issues found in 19 source files

## Fixes Not Completed

None — all fixes A through I were applied successfully.
