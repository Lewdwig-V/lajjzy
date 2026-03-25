---
managed_file: crates/lajjzy-tui/src/input.rs
version: 1
test_policy: "Write or extend tests for all key mapping functions"
---

# Keyboard input mapper

## Purpose

Pure stateless mapper from crossterm `KeyEvent` to `Action`. Three entry points
for three input contexts: normal mode, modal mode, and picking mode. No I/O,
no state mutation — just pattern matching.

## Dependencies

- `crossterm::event::{KeyCode, KeyEvent, KeyModifiers}`
- `crate::app::{Action, DetailMode, Modal, PanelFocus, PickingMode}`

## Functions

### `map_event(event: KeyEvent, focus: PanelFocus, detail_mode: DetailMode) -> Option<Action>`

Suppress lint: `#[expect(clippy::too_many_lines)]`

**Global keys** (checked first, before focus dispatch):
- `q` (no mods) → `Quit` — suppressed during HunkPicker and ConflictView
- `Ctrl-C` → `Quit` normally, `HunkCancel` during HunkPicker, `DetailBack` during ConflictView
- `Tab` / `BackTab` → `TabFocus` / `BackTabFocus` — suppressed during HunkPicker and ConflictView
- `R` (shift, not Ctrl) → `Refresh` — suppressed during HunkPicker and ConflictView
- `@` → `JumpToWorkingCopy` — suppressed during HunkPicker and ConflictView
- `O` → `ToggleOpLog` — suppressed during HunkPicker and ConflictView
- `b` (no mods) → `OpenBookmarks` — suppressed during HunkPicker and ConflictView
- `/` → `OpenOmnibar` — suppressed during HunkPicker and ConflictView
- `?` → `OpenHelp` — always available (even during HunkPicker and ConflictView)

**Graph panel** (`PanelFocus::Graph`):
- `j`/Down → `MoveDown`, `k`/Up → `MoveUp`
- `g` → `JumpToTop`, `G` → `JumpToBottom`
- `d` → `Abandon`, `n` → `NewChange`
- `Ctrl-E` → `EditChange`, `e` → `OpenDescribe`
- `s` → `Split`, `S` → `SquashPartial`
- `u` → `Undo`
- `r` → `RebaseSingle`, `Ctrl-R` → `RebaseWithDescendants`
- `Ctrl-Shift-R` (or `Ctrl+R` uppercase) → `Redo`
- `B` → `OpenBookmarkSet`, `P` → `GitPush`, `f` → `GitFetch`
- `a` → `Absorb`, `D` → `DuplicateChange`, `x` → `Revert`
- `F` → `FetchForgeStatus`, `W` → `OpenOrCreatePr`

**Detail panel** (`PanelFocus::Detail`), dispatched by `detail_mode`:

*FileList:*
- `j`/Down → `DetailMoveDown`, `k`/Up → `DetailMoveUp`
- `n` → `NextConflictFile`, `N` → `PrevConflictFile`
- `m` → `ConflictLaunchMerge`
- `Enter` → `DetailEnter`, `Esc` → `DetailBack`

*DiffView:*
- `j`/Down → `DiffScrollDown`, `k`/Up → `DiffScrollUp`
- `n` → `DiffNextHunk`, `N` → `DiffPrevHunk`
- `Esc` → `DetailBack`

*ConflictView:*
- `j`/Down → `ConflictScrollDown`, `k`/Up → `ConflictScrollUp`
- `n` → `ConflictNextHunk`, `N` → `ConflictPrevHunk`
- `1` → `ConflictAcceptLeft`, `2` → `ConflictAcceptRight`
- `m` → `ConflictLaunchMerge`
- `Enter` → `ConflictConfirm`, `Esc` → `DetailBack`

*HunkPicker:*
- `j`/Down → `DetailMoveDown`, `k`/Up → `DetailMoveUp`
- `J` → `HunkNextFile`, `K` → `HunkPrevFile`
- `Space` → `HunkToggle`, `a` → `HunkSelectAll`, `A` → `HunkDeselectAll`
- `Enter` → `HunkConfirm`, `Esc` → `HunkCancel`

### `map_modal_event(event: KeyEvent, modal: &Modal) -> Option<Action>`

**Describe modal** (early return):
- `Ctrl-S` or `Ctrl-Enter` or `Alt-Enter` → `DescribeSave`
- `Esc` → `ModalDismiss`
- `Shift-E` → `DescribeEscalateEditor`
- All other keys → `None` (tui-textarea handles them)

**BookmarkInput modal** (early return):
- `Esc` → `ModalDismiss`
- `Enter` → `BookmarkInputConfirm`
- `Backspace` → `BookmarkInputBackspace`
- `Char(c)` (no mods or shift) → `BookmarkInputChar(c)`
- All other keys → `None`

**Common modal keys** (all remaining modals):
- `Esc` → `ModalDismiss`, `Enter` → `ModalEnter`
- `Up` → `ModalMoveUp`, `Down` → `ModalMoveDown`

**Omnibar-specific** (after common keys):
- `Tab` when completions non-empty → `OmnibarAcceptCompletion`
- `Backspace` → `OmnibarBackspace`
- `Ctrl-N` → `ModalMoveDown`, `Ctrl-P` → `ModalMoveUp`
- `Char(c)` (no mods or shift) → `OmnibarInput(c)`

**Non-omnibar modals** (after common keys):
- `q` → `ModalDismiss`
- `j` → `ModalMoveDown`, `k` → `ModalMoveUp`
- `d` in BookmarkPicker → `BookmarkDelete`
- `O` in OpLog → `ModalDismiss` (toggle off)
- `?` in Help → `ModalDismiss` (toggle off)

### `map_picking_event(event: KeyEvent, picking: &PickingMode) -> Option<Action>`

**Browsing:**
- `j`/Down → `MoveDown`, `k`/Up → `MoveUp`
- `Enter` → `PickConfirm`, `Esc` → `PickCancel`
- `Char(c)` (no mods or shift) → `PickFilterChar(c)`

**Filtering:**
- `Ctrl-J`/Down → `MoveDown`, `Ctrl-K`/Up → `MoveUp`
- `Enter` → `PickConfirm`, `Esc` → `PickCancel`
- `Backspace` → `PickFilterBackspace`
- `Char(c)` (no mods or shift) → `PickFilterChar(c)`

## Tests

Test helpers:
- `key(code: KeyCode) -> KeyEvent` — no modifiers
- `key_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent`
- `map_graph`, `map_file_list`, `map_diff_view`, `map_hunk_picker`, `map_conflict_view` — convenience wrappers

Test coverage (33 tests):
1. `global_quit_keys_work_in_any_focus` — q and Ctrl-C across graph/file_list/diff_view
2. `tab_cycles_focus` — Tab and BackTab
3. `refresh_and_at_are_global` — R and @ across all focus modes
4. `graph_navigation` — j/k/Down/Up/g/G
5. `detail_file_list_navigation` — j/k/Enter/Esc
6. `detail_diff_view_navigation` — j/k/n/N/Esc
7. `same_key_different_action_by_context` — j maps differently per context
8. `unmapped_key_returns_none` — z in all contexts
9. `modal_trigger_keys` — O/b/'/'/? in graph
10. `modal_esc_dismisses` — Esc in Help modal
11. `modal_q_dismisses_non_omnibar` — q in Help modal
12. `omnibar_q_is_text_input` — q in Omnibar → OmnibarInput
13. `modal_jk_navigation_non_fuzzy` — j/k in OpLog
14. `omnibar_ctrl_n_p_navigation` — Ctrl-N/P in Omnibar
15. `omnibar_backspace` — Backspace in Omnibar
16. `oplog_toggle_key_dismisses` — O in OpLog → dismiss
17. `help_question_mark_dismisses` — ? in Help → dismiss
18. `bookmark_input_key_routing` — Enter/Esc/Backspace/Char/Shift-Char/Ctrl-Char
19. `bookmark_picker_d_deletes` — d in BookmarkPicker
20. `graph_mutation_keys` — d/n/e/Ctrl-E/s/S/u/Ctrl-R/B/P/f
21. `mutation_keys_not_active_in_detail_context` — d/n/f in detail
22. `ctrl_e_edit_before_plain_e_describe` — distinct mappings
23. `rebase_keys_in_graph_context` — r and Ctrl-R
24. `redo_moved_to_ctrl_shift_r` — Ctrl-Shift-R is Redo, Ctrl-R is not
25. `picking_mode_browsing_key_routing` — j/k/Enter/Esc/Char
26. `picking_mode_filtering_key_routing` — Ctrl-J/Down/Backspace/Char
27. `picking_mode_blocks_global_keys` — / and ? become PickFilterChar
28. `hunk_picker_key_routing` — j/k/J/K/Space/a/A/Enter/Esc
29. `tab_accepts_completion_when_visible` — Tab with completions
30. `tab_noop_when_no_completions` — Tab without completions
31. `tab_suppressed_during_hunk_picker` — Tab/BackTab → None
32. `quit_suppressed_during_hunk_picker` — q → None, Ctrl-C → HunkCancel
33. `conflict_view_key_routing` — 1/2/n/N/m/j/k/Enter/Esc
34. `quit_suppressed_during_conflict_view` — q → None
35. `tab_suppressed_during_conflict_view` — Tab/BackTab → None
36. `ctrl_c_cancels_conflict_view` — Ctrl-C → DetailBack
37. `file_list_conflict_navigation` — n/N/m in file list
38. `s_and_s_keys_map_correctly` — s/S in graph vs detail
39. `m7_graph_mutation_keys` — a/D/x
40. `m7_keys_not_active_in_detail` — a/x in file list
41. `forge_keys_in_graph_context` — F/W
42. `forge_keys_not_active_in_detail` — F/W in file list
