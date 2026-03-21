# M3c: Split & Partial Squash — Interactive Hunk Picker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `s` (split) and `S` (partial squash) with an interactive hunk picker widget that replaces the detail pane, allowing file-level and hunk-level selection of changes to split or squash.

**Architecture:** The hunk picker is a `DetailMode::HunkPicker` state on the detail pane (not a modal). A flat cursor model navigates file headers and hunks. `RepoBackend::change_diff` loads all hunks at once. The backend uses file paths for unanimous file selections and `--tool` helper for mixed-hunk files. `S` (full squash) is replaced by partial squash with hunk picker — full squash is `a` then Enter.

**Tech Stack:** Rust 1.85+, ratatui 0.30, crossterm 0.29, jj CLI 0.39.0

**Spec:** `docs/superpowers/specs/2026-03-21-m3c-split-squash-design.md`

---

## File Map

### Files to create

| File | Responsibility |
|------|---------------|
| `crates/lajjzy-tui/src/widgets/hunk_picker.rs` | Hunk picker widget — file headers, hunks, selection markers, tinting |

### Files to modify

| File | Changes |
|------|---------|
| `crates/lajjzy-core/src/types.rs` | Add `FileDiff`, `FileHunkSelection` |
| `crates/lajjzy-core/src/backend.rs` | Add `change_diff`, `split`, `squash_partial` |
| `crates/lajjzy-core/src/cli.rs` | Implement methods. New `parse_file_diffs` for per-file grouping. |
| `crates/lajjzy-tui/src/action.rs` | Add actions, `HunkPickerOp`, `MutationKind` changes. Remove `Squash`. |
| `crates/lajjzy-tui/src/app.rs` | Add `HunkPicker`, `PickerFile`, `PickerHunk`, `DetailMode::HunkPicker` |
| `crates/lajjzy-tui/src/effect.rs` | Add `LoadChangeDiff`, `Split`, `SquashPartial`. Remove `Squash`. |
| `crates/lajjzy-tui/src/dispatch.rs` | Hunk picker handlers, replace Squash with SquashPartial |
| `crates/lajjzy-tui/src/input.rs` | Add `HunkPicker` key routing, `s`/`S` bindings, Tab suppression |
| `crates/lajjzy-tui/src/render.rs` | Render hunk picker in detail pane |
| `crates/lajjzy-tui/src/widgets/mod.rs` | Add `pub mod hunk_picker` |
| `crates/lajjzy-tui/src/widgets/status_bar.rs` | Hunk picker status text |
| `crates/lajjzy-tui/src/widgets/help.rs` | Update `s`/`S` keys |
| `crates/lajjzy-cli/src/main.rs` | Handle new effects, remove `Squash`, update `next_graph_generation` |

---

## Task 1: Backend — `change_diff` method + per-file diff parser

Load all file diffs for a change in one call.

**Files:**
- Modify: `crates/lajjzy-core/src/types.rs`
- Modify: `crates/lajjzy-core/src/backend.rs`
- Modify: `crates/lajjzy-core/src/cli.rs`

- [ ] **Step 1: Add `FileDiff` and `FileHunkSelection` types**

In `types.rs`:
```rust
/// All hunks for a single file in a change's diff.
#[derive(Debug, Clone, PartialEq)]
pub struct FileDiff {
    pub path: String,
    pub hunks: Vec<DiffHunk>,
}

/// User's hunk selection for a single file (sent to backend for split/squash).
#[derive(Debug, Clone, PartialEq)]
pub struct FileHunkSelection {
    pub path: String,
    pub selected_hunks: Vec<usize>,
    pub total_hunks: usize,
}
```

- [ ] **Step 2: Add `change_diff` to `RepoBackend` trait**

In `backend.rs`:
```rust
fn change_diff(&self, change_id: &str) -> Result<Vec<FileDiff>>;
```

- [ ] **Step 3: Write `parse_file_diffs` function**

In `cli.rs`, create a new parser that groups diff output by file. The existing `parse_diff_output` returns a flat `Vec<DiffHunk>`. The new function splits on `diff --git a/<path> b/<path>` lines to produce `Vec<FileDiff>`:

```rust
fn parse_file_diffs(output: &str) -> Result<Vec<FileDiff>> {
    let mut files = Vec::new();
    let mut current_path: Option<String> = None;
    let mut current_hunks: Vec<DiffHunk> = Vec::new();
    let mut current_hunk: Option<DiffHunk> = None;
    let mut header_lines: Vec<DiffLine> = Vec::new();

    for line in output.lines() {
        if line.starts_with("diff --git ") {
            // Flush previous file
            if let Some(hunk) = current_hunk.take() {
                current_hunks.push(hunk);
            }
            if !current_hunks.is_empty() || !header_lines.is_empty() {
                if let Some(path) = current_path.take() {
                    if current_hunks.is_empty() && !header_lines.is_empty() {
                        // Header-only file (chmod, binary) — synthetic hunk
                        current_hunks.push(DiffHunk { header: String::new(), lines: header_lines.drain(..).collect() });
                    }
                    files.push(FileDiff { path, hunks: current_hunks });
                    current_hunks = Vec::new();
                }
            }
            header_lines.clear();
            // Extract path from "diff --git a/<path> b/<path>"
            let path = line.split(" b/").nth(1).unwrap_or("unknown").to_string();
            current_path = Some(path);
        } else if line.starts_with("@@") {
            if let Some(hunk) = current_hunk.take() {
                current_hunks.push(hunk);
            }
            current_hunk = Some(DiffHunk { header: line.to_string(), lines: Vec::new() });
        } else if let Some(ref mut hunk) = current_hunk {
            // Parse diff line (same logic as existing parse_diff_output)
            let (kind, content) = if let Some(rest) = line.strip_prefix('+') {
                (DiffLineKind::Added, rest)
            } else if let Some(rest) = line.strip_prefix('-') {
                (DiffLineKind::Removed, rest)
            } else if let Some(rest) = line.strip_prefix(' ') {
                (DiffLineKind::Context, rest)
            } else {
                (DiffLineKind::Context, line)
            };
            hunk.lines.push(DiffLine { kind, content: content.to_string() });
        } else {
            // Pre-@@ header line
            header_lines.push(DiffLine { kind: DiffLineKind::Header, content: line.to_string() });
        }
    }

    // Flush final file
    if let Some(hunk) = current_hunk {
        current_hunks.push(hunk);
    }
    if let Some(path) = current_path {
        if current_hunks.is_empty() && !header_lines.is_empty() {
            current_hunks.push(DiffHunk { header: String::new(), lines: header_lines });
        }
        files.push(FileDiff { path, hunks: current_hunks });
    }

    Ok(files)
}
```

- [ ] **Step 4: Implement `change_diff` on `JjCliBackend`**

```rust
fn change_diff(&self, change_id: &str) -> Result<Vec<FileDiff>> {
    let output = Command::new("jj")
        .args(["diff", "-r", change_id, "--git", "--color=never"])
        .current_dir(&self.workspace_root)
        .output()
        .with_context(|| format!("Failed to run `jj diff` for {change_id}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("jj diff failed for {change_id}: {}", stderr.trim());
    }

    let stdout = String::from_utf8(output.stdout)
        .context("jj diff output was not valid UTF-8")?;

    parse_file_diffs(&stdout)
}
```

- [ ] **Step 5: Write backend test**

```rust
#[test]
fn change_diff_returns_grouped_file_diffs() {
    if !jj_available() { eprintln!("Skipping"); return; }
    let tmp = init_repo();
    let backend = JjCliBackend::new(tmp.path()).unwrap();
    // Create two files
    std::fs::write(tmp.path().join("foo.txt"), "hello\n").unwrap();
    std::fs::write(tmp.path().join("bar.txt"), "world\n").unwrap();
    backend.describe("@", "add files").unwrap();

    let files = backend.change_diff("@").unwrap();
    assert!(files.len() >= 2, "should have at least 2 files");
    assert!(files.iter().any(|f| f.path == "foo.txt"));
    assert!(files.iter().any(|f| f.path == "bar.txt"));
    // Each file should have at least one hunk
    for f in &files {
        assert!(!f.hunks.is_empty(), "file {} should have hunks", f.path);
    }
}
```

Also add a unit test for `parse_file_diffs` with hardcoded multi-file diff output.

- [ ] **Step 6: Run tests, clippy, fmt, commit**

```bash
git add -A && git commit -m "feat(core): add change_diff method + per-file diff parser"
```

---

## Task 2: Backend — `split` and `squash_partial` methods (file-level only)

Start with the file-level fast path. Hunk-level `--tool` fallback is Task 9 (stretch).

**Files:**
- Modify: `crates/lajjzy-core/src/backend.rs`
- Modify: `crates/lajjzy-core/src/cli.rs`

- [ ] **Step 1: Add methods to trait**

```rust
fn split(&self, change_id: &str, selections: &[FileHunkSelection]) -> Result<String>;
fn squash_partial(&self, change_id: &str, selections: &[FileHunkSelection]) -> Result<String>;
```

- [ ] **Step 2: Implement file-level split**

For M3c, implement the file-level fast path. **Key semantics:** `jj split <paths>` puts the specified paths in the FIRST (original) change; the rest go to the SECOND (child). Since our spec says "selected = what moves to the child," we pass the **UNselected** file paths to `jj split` — those stay in the original, and the selected files end up in the child.

**MANDATORY:** Run `jj split --help` to verify this before implementing.

```rust
fn split(&self, change_id: &str, selections: &[FileHunkSelection]) -> Result<String> {
    // "selected = moves to child" means we pass the COMPLEMENT to jj split.
    // jj split <paths> keeps <paths> in the original; the rest go to the child.
    let all_paths: HashSet<&str> = selections.iter().map(|s| s.path.as_str()).collect();
    let selected_paths: HashSet<&str> = selections.iter()
        .filter(|s| s.selected_hunks.len() == s.total_hunks)
        .map(|s| s.path.as_str())
        .collect();
    let keep_in_original: Vec<&str> = all_paths.difference(&selected_paths).copied().collect();
    if keep_in_original.is_empty() {
        // All files selected — nothing stays in original. This is a degenerate case.
        bail!("Cannot split: all files selected (nothing would remain in original)");
    }
    let mut args = vec!["split", "-r", change_id, "--"];
    args.extend(keep_in_original);
    self.run_jj(&args)?;
    Ok(format!("Split {change_id}"))
}
```

- [ ] **Step 3: Implement file-level squash_partial**

```rust
fn squash_partial(&self, change_id: &str, selections: &[FileHunkSelection]) -> Result<String> {
    let selected_paths: Vec<&str> = selections.iter()
        .filter(|s| !s.selected_hunks.is_empty())
        .map(|s| s.path.as_str())
        .collect();
    if selected_paths.is_empty() {
        bail!("No files selected for squash");
    }
    // -u prevents jj from opening $EDITOR for combined description (would hang the TUI)
    let mut args = vec!["squash", "-r", change_id, "-u", "--"];
    args.extend(selected_paths);
    self.run_jj(&args)?;
    Ok(format!("Squashed from {change_id}"))
}
```

`jj squash <paths>` moves the specified file changes into the parent.

- [ ] **Step 4: Write backend tests**

```rust
#[test]
fn split_on_real_repo() {
    if !jj_available() { eprintln!("Skipping"); return; }
    let tmp = init_repo();
    let backend = JjCliBackend::new(tmp.path()).unwrap();
    std::fs::write(tmp.path().join("keep.txt"), "keep\n").unwrap();
    std::fs::write(tmp.path().join("move.txt"), "move\n").unwrap();
    backend.describe("@", "two files").unwrap();

    let selections = vec![FileHunkSelection {
        path: "move.txt".into(),
        selected_hunks: vec![0],
        total_hunks: 1,
    }];
    backend.split("@", &selections).unwrap();

    let graph = backend.load_graph(None).unwrap();
    // Should now have an extra change
    assert!(graph.node_indices().len() >= 3); // root + original + new child
}

#[test]
fn squash_partial_on_real_repo() {
    if !jj_available() { eprintln!("Skipping"); return; }
    let tmp = init_repo();
    let backend = JjCliBackend::new(tmp.path()).unwrap();
    backend.describe("@", "parent").unwrap();
    backend.new_change("@").unwrap();
    std::fs::write(tmp.path().join("file.txt"), "content\n").unwrap();
    backend.describe("@", "child").unwrap();

    let selections = vec![FileHunkSelection {
        path: "file.txt".into(),
        selected_hunks: vec![0],
        total_hunks: 1,
    }];
    backend.squash_partial("@", &selections).unwrap();

    let graph = backend.load_graph(None).unwrap();
    // The child should still exist (unless it became empty and was abandoned)
    // Parent should now have the file
}
```

- [ ] **Step 5: Run tests, clippy, fmt, commit**

```bash
git add -A && git commit -m "feat(core): add split and squash_partial methods (file-level fast path)"
```

---

## Task 3: Replace `Squash` with `Split`/`SquashPartial` types

Remove the old instant `Squash` action/effect/mutation and add new types. Breaking change — all references to `Squash` must be updated.

**This task must be committed atomically** — partial completion will not compile because removing `MutationKind::Squash` breaks `clear_op_gate`, dispatch, input, and the executor simultaneously.

**Files:**
- Modify: `crates/lajjzy-tui/src/action.rs`
- Modify: `crates/lajjzy-tui/src/effect.rs`
- Modify: `crates/lajjzy-tui/src/app.rs`
- Modify: `crates/lajjzy-tui/src/dispatch.rs`
- Modify: `crates/lajjzy-tui/src/input.rs`
- Modify: `crates/lajjzy-cli/src/main.rs`

- [ ] **Step 1: Update `action.rs`**

Remove: `Action::Squash`, `MutationKind::Squash`

Add:
```rust
// Trigger actions
Split,
SquashPartial,

// Hunk picker actions
ChangeDiffLoaded { operation: HunkPickerOp, result: Result<Vec<FileDiff>, String> },
HunkToggle,
HunkSelectAll,
HunkDeselectAll,
HunkNextFile,
HunkPrevFile,
HunkConfirm,
HunkCancel,

// Types
/// Must derive Debug, Clone, PartialEq — embedded in Action and Effect which derive these.
#[derive(Debug, Clone, PartialEq)]
pub enum HunkPickerOp {
    Split { source: String },
    Squash { source: String, destination: String },
}
```

Add to `MutationKind`: `Split`, `SquashPartial`

Also ensure `FileHunkSelection` in `types.rs` derives `Clone` (needed by `Effect::Split`/`SquashPartial` which derive `Clone`).

Note: `HunkPickerOp` needs `use lajjzy_core::types::FileDiff;` for `ChangeDiffLoaded`.

- [ ] **Step 2: Update `effect.rs`**

Remove: `Effect::Squash { change_id }`

Add:
```rust
LoadChangeDiff { change_id: String, operation: HunkPickerOp },
Split { change_id: String, selections: Vec<FileHunkSelection> },
SquashPartial { change_id: String, selections: Vec<FileHunkSelection> },
```

- [ ] **Step 3: Add state types to `app.rs`**

```rust
pub struct HunkPicker {
    pub operation: HunkPickerOp,
    pub files: Vec<PickerFile>,
    pub cursor: usize,
    pub scroll: usize,
}

pub struct PickerFile {
    pub path: String,
    pub hunks: Vec<PickerHunk>,
}

pub struct PickerHunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
    pub selected: bool,
}
```

Add `pub hunk_picker: Option<HunkPicker>` to `AppState`, init to `None`.
Add `HunkPicker` to `DetailMode` enum.

- [ ] **Step 4: Update `dispatch.rs`**

Replace `Action::Squash` handler with placeholder `Action::Split` and `Action::SquashPartial` handlers. Add placeholder arms for all hunk picker actions + `ChangeDiffLoaded`. Update `clear_op_gate()` to replace `Squash` with `Split | SquashPartial`.

- [ ] **Step 5: Update `input.rs`**

Change `S` from `Action::Squash` to `Action::SquashPartial`. Add `s` → `Action::Split` in Graph context.

- [ ] **Step 6: Update `main.rs` executor**

Remove `Effect::Squash` handler. Add placeholder arms for `LoadChangeDiff`, `Split`, `SquashPartial`. Update `next_graph_generation` (remove `Squash`, add `Split`, `SquashPartial`). `LoadChangeDiff` goes in the non-graph-generation arm (like `LoadFileDiff`).

- [ ] **Step 6b: Remove old `squash()` from `RepoBackend` trait and `JjCliBackend` impl**

In `backend.rs`, remove `fn squash(&self, change_id: &str) -> Result<String>`. In `cli.rs`, remove the `squash()` implementation and the `squash_on_real_repo` test. This is dead code now that `S` opens the hunk picker.

- [ ] **Step 7: Fix all compilation errors from Squash removal**

Search for every remaining `Squash` reference across all files (test assertions, etc.) and update. The old `squash_emits_effect_and_sets_gate` test becomes `squash_partial_emits_load_change_diff`.

- [ ] **Step 8: Run tests, clippy, fmt, commit**

```bash
git add -A && git commit -m "refactor: replace Squash with Split/SquashPartial types, add hunk picker state"
```

---

## Task 4: Hunk picker dispatch logic

The core dispatch handlers for entering, navigating, toggling, confirming, and canceling.

**Files:**
- Modify: `crates/lajjzy-tui/src/dispatch.rs`

- [ ] **Step 1: Write failing tests**

Key tests:
```rust
#[test]
fn split_emits_load_change_diff() {
    let mut state = AppState::new(sample_graph());
    let effects = dispatch(&mut state, Action::Split);
    assert_eq!(effects.len(), 1);
    assert!(matches!(&effects[0], Effect::LoadChangeDiff { .. }));
}

#[test]
fn squash_partial_emits_load_change_diff() {
    let mut state = AppState::new(sample_graph_with_parents());
    let effects = dispatch(&mut state, Action::SquashPartial);
    assert_eq!(effects.len(), 1);
    assert!(matches!(&effects[0], Effect::LoadChangeDiff { .. }));
}

#[test]
fn squash_partial_on_root_shows_error() {
    let mut state = AppState::new(sample_graph());
    // Navigate to root change (no parents)
    // ... dispatch SquashPartial
    // assert error set
}

#[test]
fn split_on_empty_change_shows_error() {
    // ChangeDiffLoaded with empty Vec<FileDiff>
}

#[test]
fn change_diff_loaded_opens_hunk_picker() {
    // Simulate ChangeDiffLoaded with FileDiffs
    // Verify detail_mode == HunkPicker
    // Verify all hunks unselected
}

#[test]
fn hunk_toggle_selects_and_deselects() { ... }
#[test]
fn hunk_toggle_on_file_header_toggles_all() { ... }
#[test]
fn hunk_select_all_and_deselect_all() { ... }
#[test]
fn hunk_next_file_and_prev_file() { ... }
#[test]
fn hunk_confirm_emits_split_effect() { ... }
#[test]
fn hunk_confirm_with_nothing_selected_shows_error() { ... }
#[test]
fn hunk_cancel_exits_picker() { ... }
#[test]
fn detail_move_down_up_in_hunk_picker() { ... }
```

- [ ] **Step 2: Implement `Split` and `SquashPartial` handlers**

```rust
Action::Split => {
    if state.pending_mutation.is_some() || state.hunk_picker.is_some() {
        state.status_message = Some("Operation in progress…".into());
        return vec![];
    }
    if let Some(cid) = state.selected_change_id().map(String::from) {
        return vec![Effect::LoadChangeDiff {
            change_id: cid.clone(),
            operation: HunkPickerOp::Split { source: cid },
        }];
    }
}
```

SquashPartial: same but checks for parent, constructs `HunkPickerOp::Squash`.

- [ ] **Step 3: Implement `ChangeDiffLoaded` handler**

Convert `Vec<FileDiff>` into `HunkPicker` state:
```rust
Action::ChangeDiffLoaded { operation, result } => {
    match result {
        Ok(file_diffs) => {
            if file_diffs.is_empty() {
                state.status_message = Some("Nothing to split: change is empty".into());
                return vec![];
            }
            let files = file_diffs.into_iter().map(|fd| PickerFile {
                path: fd.path,
                hunks: fd.hunks.into_iter().map(|h| PickerHunk {
                    header: h.header,
                    lines: h.lines,
                    selected: false,
                }).collect(),
            }).collect();
            state.hunk_picker = Some(HunkPicker {
                operation,
                files,
                cursor: 0,
                scroll: 0,
            });
            state.detail_mode = DetailMode::HunkPicker;
            state.focus = PanelFocus::Detail;
        }
        Err(e) => {
            state.error = Some(format!("Failed to load diff: {e}"));
        }
    }
}
```

- [ ] **Step 4: Implement hunk picker navigation and selection**

Helper functions for the flat cursor model:
```rust
/// Total number of selectable items (file headers + hunks)
fn picker_item_count(picker: &HunkPicker) -> usize {
    picker.files.iter().map(|f| 1 + f.hunks.len()).sum()
}

/// Returns (file_index, None) for file header or (file_index, Some(hunk_index)) for a hunk
fn picker_item_at(picker: &HunkPicker, flat_index: usize) -> Option<(usize, Option<usize>)> {
    let mut pos = 0;
    for (fi, file) in picker.files.iter().enumerate() {
        if pos == flat_index { return Some((fi, None)); }
        pos += 1;
        for hi in 0..file.hunks.len() {
            if pos == flat_index { return Some((fi, Some(hi))); }
            pos += 1;
        }
    }
    None
}
```

`DetailMoveDown`/`DetailMoveUp`: when `detail_mode == HunkPicker`, increment/decrement `hunk_picker.cursor` within bounds.

`HunkToggle`: use `picker_item_at` to find what the cursor is on. If file header → toggle all hunks in file. If hunk → toggle that hunk.

`HunkSelectAll`/`HunkDeselectAll`: iterate all files and hunks.

`HunkNextFile`/`HunkPrevFile`: scan flat indices for next/prev file header.

- [ ] **Step 5: Implement `HunkConfirm`**

Build `Vec<FileHunkSelection>` from picker state, emit effect:
```rust
Action::HunkConfirm => {
    if let Some(picker) = state.hunk_picker.take() {
        let total_selected: usize = picker.files.iter()
            .flat_map(|f| &f.hunks)
            .filter(|h| h.selected)
            .count();
        if total_selected == 0 {
            state.hunk_picker = Some(picker);
            state.status_message = Some("No hunks selected".into());
            return vec![];
        }
        // Validate no mixed-hunk selections (file-level backend only for M3c)
        let has_mixed = picker.files.iter().any(|f| {
            let selected = f.hunks.iter().filter(|h| h.selected).count();
            selected > 0 && selected < f.hunks.len()
        });
        if has_mixed {
            state.hunk_picker = Some(picker);
            state.status_message = Some(
                "Mixed hunk selection within a file not yet supported. Select all or none per file.".into()
            );
            return vec![];
        }
        let selections: Vec<FileHunkSelection> = picker.files.iter()
            .map(|f| FileHunkSelection {
                path: f.path.clone(),
                selected_hunks: f.hunks.iter().enumerate()
                    .filter(|(_, h)| h.selected)
                    .map(|(i, _)| i)
                    .collect(),
                total_hunks: f.hunks.len(),
            })
            .filter(|s| !s.selected_hunks.is_empty())
            .collect();
        let (effect, mutation_kind) = match picker.operation {
            HunkPickerOp::Split { source } => (
                Effect::Split { change_id: source, selections },
                MutationKind::Split,
            ),
            HunkPickerOp::Squash { source, .. } => (
                Effect::SquashPartial { change_id: source, selections },
                MutationKind::SquashPartial,
            ),
        };
        state.pending_mutation = Some(mutation_kind);
        state.detail_mode = DetailMode::FileList;
        return vec![effect];
    }
}
```

- [ ] **Step 6: Implement `HunkCancel`**

```rust
Action::HunkCancel => {
    state.hunk_picker = None;
    state.detail_mode = DetailMode::FileList;
}
```

- [ ] **Step 7: Run tests, clippy, fmt, commit**

```bash
git add -A && git commit -m "feat: hunk picker dispatch — enter, toggle, select, confirm, cancel, navigation"
```

---

## Task 5: Input routing for hunk picker

**Files:**
- Modify: `crates/lajjzy-tui/src/input.rs`

- [ ] **Step 1: Add `DetailMode::HunkPicker` branch**

In `map_event`, `PanelFocus::Detail`:
```rust
DetailMode::HunkPicker => match (event.code, event.modifiers) {
    (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => Some(Action::DetailMoveDown),
    (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => Some(Action::DetailMoveUp),
    (KeyCode::Char('J'), _) => Some(Action::HunkNextFile),
    (KeyCode::Char('K'), _) => Some(Action::HunkPrevFile),
    (KeyCode::Char(' '), _) => Some(Action::HunkToggle),
    (KeyCode::Char('a'), KeyModifiers::NONE) => Some(Action::HunkSelectAll),
    (KeyCode::Char('A'), _) => Some(Action::HunkDeselectAll),
    (KeyCode::Enter, _) => Some(Action::HunkConfirm),
    (KeyCode::Esc, _) => Some(Action::HunkCancel),
    _ => None,
},
```

- [ ] **Step 2: Suppress Tab/BackTab during hunk picker**

In the global keys section, check `state.detail_mode`:
```rust
// Suppress Tab during hunk picker to prevent graph-focused-while-picker-open confusion
if detail_mode == DetailMode::HunkPicker {
    return None; // swallow Tab
}
```

Note: `map_event` receives `detail_mode` as a parameter, so this check is straightforward.

Actually, the global Tab handler fires before the focus-specific match. We need to suppress Tab when `detail_mode == HunkPicker`. The cleanest way: add the check in the global Tab match:

```rust
(KeyCode::Char('q'), KeyModifiers::NONE) | (KeyCode::Char('c'), KeyModifiers::CONTROL)
    if detail_mode != DetailMode::HunkPicker => {
    return Some(Action::Quit);
}
(KeyCode::Tab, _) if detail_mode != DetailMode::HunkPicker => return Some(Action::TabFocus),
(KeyCode::BackTab, _) if detail_mode != DetailMode::HunkPicker => return Some(Action::BackTabFocus),
```

This suppresses `q`/`Ctrl-C`, Tab, and BackTab during hunk picker. The user must Esc first to exit the picker.

- [ ] **Step 3: Update existing S binding**

Change:
```rust
(KeyCode::Char('S'), _) => Some(Action::Squash),
```
to:
```rust
(KeyCode::Char('S'), _) => Some(Action::SquashPartial),
```

Add:
```rust
(KeyCode::Char('s'), KeyModifiers::NONE) => Some(Action::Split),
```

- [ ] **Step 4: Write input tests**

```rust
#[test]
fn hunk_picker_key_routing() { /* j/k/J/K/Space/a/A/Enter/Esc */ }

#[test]
fn s_key_maps_to_split() {
    assert_eq!(map_graph(key(KeyCode::Char('s'))), Some(Action::Split));
}

#[test]
fn S_key_maps_to_squash_partial() {
    assert_eq!(
        map_graph(key_mod(KeyCode::Char('S'), KeyModifiers::SHIFT)),
        Some(Action::SquashPartial)
    );
}

#[test]
fn tab_suppressed_during_hunk_picker() {
    // map_event with DetailMode::HunkPicker and Tab key → None
}
```

- [ ] **Step 5: Update existing tests that reference `Action::Squash`**

- [ ] **Step 6: Run tests, clippy, fmt, commit**

```bash
git add -A && git commit -m "feat: hunk picker input routing, Tab suppression, s/S key bindings"
```

---

## Task 6: Executor wiring

**Files:**
- Modify: `crates/lajjzy-cli/src/main.rs`

- [ ] **Step 1: Handle new effects**

Replace `Effect::Squash` with the new effects:

```rust
Effect::LoadChangeDiff { change_id, operation } => {
    let result = backend.change_diff(&change_id).map_err(|e| e.to_string());
    let _ = tx.send(Action::ChangeDiffLoaded { operation, result });
}
Effect::Split { change_id, selections } => {
    run_mutation(&backend, &tx, MutationKind::Split, generation, &active_revset, || {
        backend.split(&change_id, &selections)
    });
}
Effect::SquashPartial { change_id, selections } => {
    run_mutation(&backend, &tx, MutationKind::SquashPartial, generation, &active_revset, || {
        backend.squash_partial(&change_id, &selections)
    });
}
```

- [ ] **Step 2: Update `next_graph_generation`**

Remove `Effect::Squash`. Add `Effect::Split` and `Effect::SquashPartial` to the graph-incrementing arm. `Effect::LoadChangeDiff` goes in the non-graph arm (like `LoadFileDiff`).

- [ ] **Step 3: Run tests, clippy, fmt, commit**

```bash
git add -A && git commit -m "feat: executor handles LoadChangeDiff, Split, SquashPartial effects"
```

---

## Task 7: Hunk picker widget

**Files:**
- Create: `crates/lajjzy-tui/src/widgets/hunk_picker.rs`
- Modify: `crates/lajjzy-tui/src/widgets/mod.rs`
- Modify: `crates/lajjzy-tui/src/render.rs`
- Modify: `crates/lajjzy-tui/src/widgets/status_bar.rs`

- [ ] **Step 1: Create `hunk_picker.rs` widget**

The widget renders:
- File headers with `[n/m]` selection count
- Hunks with `[✓]`/`[ ]` markers
- Diff lines with existing color scheme
- Selected hunks get a subtle background tint (cyan on dark terminals)
- Cursor highlights the current item

The widget accepts `&HunkPicker` and renders the flat list with scroll offset.

- [ ] **Step 2: Register in `widgets/mod.rs`**

```rust
pub mod hunk_picker;
```

- [ ] **Step 3: Render in `render.rs`**

When `state.detail_mode == DetailMode::HunkPicker && state.hunk_picker.is_some()`, render the hunk picker widget in the detail pane area.

- [ ] **Step 4: Update status bar**

When `hunk_picker` is `Some`, show picking-mode text:
- Split: `Split: n/m hunks selected → new change after <source>`
- Squash: `Squash: n/m hunks from <source> → into <destination>`

Add `hunk_picker: Option<&HunkPicker>` parameter to `StatusBarWidget`.

- [ ] **Step 5: Write widget tests**

```rust
#[test]
fn hunk_picker_renders_files_and_hunks() { ... }
#[test]
fn hunk_picker_selected_hunks_have_tint() { ... }
#[test]
fn hunk_picker_file_header_shows_count() { ... }
```

- [ ] **Step 6: Run tests, clippy, fmt, commit**

```bash
git add -A && git commit -m "feat: hunk picker widget with file headers, selection markers, and tinting"
```

---

## Task 8: Help widget + final integration

**Files:**
- Modify: `crates/lajjzy-tui/src/widgets/help.rs`
- Modify: `crates/lajjzy-tui/src/modal.rs`

- [ ] **Step 1: Update help**

Change `S` entry from "Squash into parent" to "Squash (select hunks)". Add `s` entry: "Split (select hunks)".

Update `line_count()`.

- [ ] **Step 2: Full test suite**

Run: `cargo test`

- [ ] **Step 3: Clippy and fmt**

Run: `cargo clippy --all-targets -- -D warnings && cargo fmt --check`

- [ ] **Step 4: Manual smoke test**

1. Create change with two files modified
2. Press `s` → hunk picker opens, all unselected
3. Navigate with `j`/`k`, toggle with Space
4. Press `a` → all selected, `A` → all deselected
5. Select one file's hunks, press Enter → change splits
6. `u` to undo
7. Press `S` → hunk picker for squash, `a` then Enter → full squash
8. `u` to undo
9. Esc cancels picker

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "docs: update help widget + integration testing"
```

---

## Task 9 (stretch): Hunk-level `--tool` backend fallback

For mixed-hunk files (some hunks selected, some not). Deferred if M3c scope needs trimming.

**Files:**
- Modify: `crates/lajjzy-core/src/cli.rs`

- [ ] **Step 1: Implement `--tool` helper for mixed-hunk split**

When `FileHunkSelection` has `selected_hunks.len() < total_hunks` and `selected_hunks.len() > 0`, the file-level fast path doesn't work. Implement a helper that:
1. Writes a script to a tempfile that copies pre-computed file contents
2. Passes it to `jj split --tool <script_path>`

This is complex and may require the C4 audit to determine the best approach. For M3c, if a file has mixed hunk selection, show an error: "Mixed hunk selection within a file requires jj-lib (coming soon). Select all or none for each file."

- [ ] **Step 2: Test and commit**

```bash
git add -A && git commit -m "feat: mixed-hunk split/squash via --tool helper (or graceful error)"
```
