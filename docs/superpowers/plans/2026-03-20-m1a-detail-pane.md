# M1a Detail Pane Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a lazygit-style detail pane with file list and hunk diff view, panel focus system, and split layout.

**Architecture:** Extend the existing three-crate workspace. Core layer gets new types (FileChange, DiffHunk) and `file_diff()` on RepoBackend. TUI layer gets panel modules (graph.rs, detail.rs) with shared flat AppState, context-sensitive input routing, and two new widgets (FileListWidget, DiffViewWidget). Layout splits 1/3 graph + 2/3 detail.

**Tech Stack:** Rust (stable), ratatui 0.30, crossterm 0.29, anyhow

**Spec:** `docs/superpowers/specs/2026-03-20-m1a-detail-pane-design.md`

---

## File Structure

```
crates/
├── lajjzy-core/src/
│   ├── types.rs                    # MODIFY: add FileChange, FileStatus, DiffHunk, DiffLine, DiffLineKind
│   │                               #         add files: Vec<FileChange> to ChangeDetail
│   ├── backend.rs                  # MODIFY: add file_diff() to RepoBackend
│   └── cli.rs                      # MODIFY: update template (remove description line, add --summary)
│                                   #         add file line parsing to parse_graph_output
│                                   #         add file_diff() impl + parse_diff_output()
├── lajjzy-tui/src/
│   ├── lib.rs                      # MODIFY: add pub mod panels;
│   ├── app.rs                      # MODIFY: add PanelFocus, DetailMode, new AppState fields,
│   │                               #         new Action variants, refactor dispatch to delegate
│   ├── input.rs                    # MODIFY: map_event gains focus + detail_mode params
│   ├── render.rs                   # MODIFY: split layout 1/3+2/3, delegate to panel renderers
│   ├── panels/
│   │   ├── mod.rs                  # CREATE
│   │   ├── graph.rs                # CREATE: graph panel handle + render
│   │   └── detail.rs              # CREATE: detail panel handle + render
│   └── widgets/
│       ├── mod.rs                  # MODIFY: add file_list, diff_view
│       ├── file_list.rs            # CREATE: FileListWidget
│       └── diff_view.rs            # CREATE: DiffViewWidget
└── lajjzy-cli/src/
    └── main.rs                     # MODIFY: pass focus + detail_mode to map_event
```

---

### Task 1: New Core Types

**Files:**
- Modify: `crates/lajjzy-core/src/types.rs`

Add the new types at the end of the file (before tests). These are standalone — no existing code changes yet.

- [ ] **Step 1: Add FileChange and FileStatus types**

Add after the `ChangeDetail` struct (after line 37):

```rust
/// A file changed in a change (parsed from `jj log --summary`).
#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: String,
    pub status: FileStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    /// Rename: path contains `{old => new}` format from jj.
    Renamed,
}

impl std::fmt::Display for FileStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Added => write!(f, "A"),
            Self::Modified => write!(f, "M"),
            Self::Deleted => write!(f, "D"),
            Self::Renamed => write!(f, "R"),
        }
    }
}
```

- [ ] **Step 2: Add DiffHunk and DiffLine types**

Add after `FileStatus`:

```rust
/// A hunk from a file diff (parsed from `jj diff --git`).
#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
    Header,
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p lajjzy-core`

- [ ] **Step 4: Commit**

```bash
git add crates/lajjzy-core/src/types.rs
git commit -m "feat(core): add FileChange, FileStatus, DiffHunk, DiffLine types"
```

---

### Task 2: Add `files` Field to ChangeDetail

**Files:**
- Modify: `crates/lajjzy-core/src/types.rs`
- Modify: `crates/lajjzy-core/src/cli.rs` (construction sites)
- Modify: `crates/lajjzy-tui/src/app.rs` (test fixtures)

This is a breaking change — every place that constructs a `ChangeDetail` needs `files: vec![]`.

- [ ] **Step 1: Add field to ChangeDetail**

In `types.rs`, add to `ChangeDetail` struct (after `has_conflict: bool`):

```rust
    pub files: Vec<FileChange>,
```

- [ ] **Step 2: Update parser construction in cli.rs**

In `cli.rs`, in `parse_graph_output()` where `ChangeDetail` is constructed (around line 85), add:

```rust
                    files: vec![],
```

This is temporary — Task 4 will populate files from `--summary` output.

- [ ] **Step 3: Update all test fixtures**

Every `ChangeDetail { ... }` in test code needs `files: vec![]` added. Files to update:
- `crates/lajjzy-core/src/types.rs` (test module `sample_graph()`)
- `crates/lajjzy-core/src/cli.rs` (test assertions that construct `ChangeDetail`)
- `crates/lajjzy-tui/src/app.rs` (test module `sample_graph()`)
- `crates/lajjzy-tui/src/widgets/status_bar.rs` (test module `sample_detail()`)

In each, add `files: vec![],` to every `ChangeDetail` literal.

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: All 36 existing tests pass.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(core): add files field to ChangeDetail (empty for now)"
```

---

### Task 3: Extend RepoBackend with `file_diff()`

**Files:**
- Modify: `crates/lajjzy-core/src/backend.rs`
- Modify: `crates/lajjzy-core/src/cli.rs` (stub impl)
- Modify: `crates/lajjzy-tui/src/app.rs` (MockBackend + FailingBackend)

- [ ] **Step 1: Add file_diff to trait**

In `backend.rs`, add to the `RepoBackend` trait:

```rust
    /// Compute diff hunks for a specific file in a change.
    /// Lazy — called only when user drills into a file.
    fn file_diff(&self, change_id: &str, path: &str) -> Result<Vec<crate::types::DiffHunk>>;
```

- [ ] **Step 2: Add stub impl to JjCliBackend**

In `cli.rs`, add to `impl RepoBackend for JjCliBackend`:

```rust
    fn file_diff(&self, _change_id: &str, _path: &str) -> Result<Vec<crate::types::DiffHunk>> {
        todo!("Implemented in Task 5")
    }
```

- [ ] **Step 3: Update MockBackend in app.rs tests**

In `crates/lajjzy-tui/src/app.rs`, update `MockBackend`:

```rust
    impl RepoBackend for MockBackend {
        fn load_graph(&self) -> Result<GraphData> {
            Ok(self.graph.clone())
        }
        fn file_diff(&self, _change_id: &str, _path: &str) -> Result<Vec<lajjzy_core::types::DiffHunk>> {
            Ok(vec![])
        }
    }
```

And `FailingBackend`:

```rust
    impl RepoBackend for FailingBackend {
        fn load_graph(&self) -> Result<GraphData> {
            anyhow::bail!("connection lost")
        }
        fn file_diff(&self, _change_id: &str, _path: &str) -> Result<Vec<lajjzy_core::types::DiffHunk>> {
            anyhow::bail!("connection lost")
        }
    }
```

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(core): add file_diff() to RepoBackend trait"
```

---

### Task 4: Parser — load_graph with --summary and File Parsing

**Files:**
- Modify: `crates/lajjzy-core/src/cli.rs`

This is the most complex core task. It updates the template, adds file line parsing, and removes the description continuation line.

- [ ] **Step 1: Write parser unit tests for file lines**

Add these tests to the `tests` module in `cli.rs`:

```rust
    #[test]
    fn parse_graph_output_with_file_summary() {
        let output = "\
@  mpvponzr add bar\x1Fmpvponzr\x1Edbd5259e\x1ELewdwig\x1Etest@test.com\x1E1m ago\x1Eadd bar\x1E\x1Efalse\x1Efalse\x1E@
│  A bar.txt
│  M foo.txt
○  mrvmvrsz add foo\x1Fmrvmvrsz\x1Ecbfd5aa0\x1ELewdwig\x1Etest@test.com\x1E2m ago\x1Eadd foo\x1E\x1Efalse\x1Efalse\x1E
│  A foo.txt
◆  zzzzzzzz (no description)\x1Fzzzzzzzz\x1E000000000000\x1E\x1E\x1E56y ago\x1E\x1E\x1Etrue\x1Efalse\x1E";

        let graph = parse_graph_output(output).unwrap();

        // First change should have 2 files
        let detail = graph.details.get("mpvponzr").unwrap();
        assert_eq!(detail.files.len(), 2);
        assert_eq!(detail.files[0].path, "bar.txt");
        assert_eq!(detail.files[0].status, crate::types::FileStatus::Added);
        assert_eq!(detail.files[1].path, "foo.txt");
        assert_eq!(detail.files[1].status, crate::types::FileStatus::Modified);

        // Second change should have 1 file
        let detail2 = graph.details.get("mrvmvrsz").unwrap();
        assert_eq!(detail2.files.len(), 1);
        assert_eq!(detail2.files[0].path, "foo.txt");

        // Root should have 0 files
        let detail3 = graph.details.get("zzzzzzzz").unwrap();
        assert!(detail3.files.is_empty());
    }

    #[test]
    fn parse_graph_output_rename() {
        let output = "\
@  abc rename\x1Fabc\x1E111\x1Ea\x1Ea@b\x1E1m\x1Erename\x1E\x1Efalse\x1Efalse\x1E@
│  R {foo.txt => bar.txt}";

        let graph = parse_graph_output(output).unwrap();
        let detail = graph.details.get("abc").unwrap();
        assert_eq!(detail.files.len(), 1);
        assert_eq!(detail.files[0].status, crate::types::FileStatus::Renamed);
        assert!(detail.files[0].path.contains("=>"));
    }

    #[test]
    fn parse_graph_output_no_files_for_empty_change() {
        let output = "\
@  abc (no description)\x1Fabc\x1E111\x1Ea\x1Ea@b\x1E1m\x1E\x1E\x1Etrue\x1Efalse\x1E@";

        let graph = parse_graph_output(output).unwrap();
        let detail = graph.details.get("abc").unwrap();
        assert!(detail.files.is_empty());
    }
```

- [ ] **Step 2: Run tests to see them fail**

Run: `cargo test -p lajjzy-core parse_graph_output_with_file`
Expected: Fails — files are always `vec![]`.

- [ ] **Step 3: Add file-line parsing to parse_graph_output**

Add a helper function before `parse_graph_output`:

```rust
/// Strip leading graph glyphs from a line to get the content.
fn strip_graph_glyphs(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Skip whitespace
        if bytes[i] == b' ' {
            i += 1;
            continue;
        }
        // Skip common ASCII graph characters
        if bytes[i] == b'|' || bytes[i] == b'-' || bytes[i] == b'@' {
            i += 1;
            continue;
        }
        // Skip multi-byte UTF-8 graph glyphs (│○◆◉├─ etc.)
        // These are all 3-byte UTF-8 sequences starting with 0xE2
        if i + 2 < bytes.len() && bytes[i] == 0xE2 {
            i += 3;
            continue;
        }
        break;
    }
    &line[i..]
}

/// Try to parse a continuation line as a file change summary.
/// Returns None if the line doesn't match the expected format.
fn parse_file_line(raw_line: &str) -> Option<crate::types::FileChange> {
    let content = strip_graph_glyphs(raw_line);
    if content.len() < 2 {
        return None;
    }

    let status_char = content.as_bytes()[0];
    let after_status = &content[1..];

    match status_char {
        b'A' if after_status.starts_with(' ') => Some(crate::types::FileChange {
            path: after_status.trim().to_string(),
            status: crate::types::FileStatus::Added,
        }),
        b'M' if after_status.starts_with(' ') => Some(crate::types::FileChange {
            path: after_status.trim().to_string(),
            status: crate::types::FileStatus::Modified,
        }),
        b'D' if after_status.starts_with(' ') => Some(crate::types::FileChange {
            path: after_status.trim().to_string(),
            status: crate::types::FileStatus::Deleted,
        }),
        b'R' if after_status.starts_with(' ') => Some(crate::types::FileChange {
            path: after_status.trim().to_string(),
            status: crate::types::FileStatus::Renamed,
        }),
        _ => None,
    }
}
```

Then update `parse_graph_output` to track the current change and parse file lines. Replace the `else` branch (the non-`\x1F` branch) with:

```rust
        } else if let Some(file_change) = parse_file_line(raw_line) {
            // Associate file with the most recent node
            if let Some(last_id) = &current_change_id {
                if let Some(detail) = details.get_mut(last_id) {
                    detail.files.push(file_change);
                }
            }
            // Still add as a display line
            lines.push(crate::types::GraphLine {
                raw: raw_line.to_string(),
                change_id: None,
            });
        } else {
            lines.push(crate::types::GraphLine {
                raw: raw_line.to_string(),
                change_id: None,
            });
        }
```

Add `let mut current_change_id: Option<String> = None;` at the top of the function, and set it in the `\x1F` branch:

```rust
            current_change_id = Some(change_id.clone());
```

- [ ] **Step 4: Update the template**

In `load_graph()`, make two changes:

1. Add `"--summary"` to the jj command args
2. Remove the description continuation line and add trailing `\n`

Replace the template `concat!` and command:

```rust
        let template = concat!(
            "change_id.short() ++ \" \" ++ ",
            "coalesce(author.name(), \"anonymous\") ++ \" \" ++ ",
            "committer.timestamp().ago()",
            " ++ \"\\x1f\"",
            " ++ change_id.short()",
            " ++ \"\\x1e\" ++ commit_id.short()",
            " ++ \"\\x1e\" ++ coalesce(author.name(), \"\")",
            " ++ \"\\x1e\" ++ coalesce(author.email(), \"\")",
            " ++ \"\\x1e\" ++ committer.timestamp().ago()",
            " ++ \"\\x1e\" ++ coalesce(description.first_line(), \"\")",
            " ++ \"\\x1e\" ++ bookmarks",
            " ++ \"\\x1e\" ++ empty",
            " ++ \"\\x1e\" ++ conflict",
            " ++ \"\\x1e\" ++ if(self.current_working_copy(), \"@\", \"\")",
            " ++ \"\\n\"",
        );

        let output = Command::new("jj")
            .args(["log", "--summary", "--color=never", "-T", template])
            .current_dir(&self.workspace_root)
            .output()
            .context("Failed to run `jj log`")?;
```

- [ ] **Step 5: Run all core tests**

Run: `cargo test -p lajjzy-core`
Expected: All tests pass including new file parsing tests.

**Note:** The existing `parse_graph_output_basic` test has description continuation lines (`│  fix bug`, `│  add feature`) that no longer appear in real output (the M1a template removes the description from graph output). These continuation lines will remain as display-only `GraphLine` entries with `change_id: None` — the parser handles them correctly as non-file lines. The test still passes as-is, but you may optionally replace the description continuation lines with file summary lines to better match real M1a output. The integration test `load_graph_on_real_repo` will validate against real jj.

- [ ] **Step 6: Commit**

```bash
git add crates/lajjzy-core/
git commit -m "feat(core): parse file summaries from jj log --summary output"
```

---

### Task 5: Diff Parser and file_diff() Implementation

**Files:**
- Modify: `crates/lajjzy-core/src/cli.rs`

- [ ] **Step 1: Write diff parser unit tests**

```rust
    #[test]
    fn parse_diff_output_single_hunk() {
        let output = "\
diff --git a/foo.txt b/foo.txt
index ce01362..2e09960 100644
--- a/foo.txt
+++ b/foo.txt
@@ -1,1 +1,1 @@
-hello
+modified";

        let hunks = parse_diff_output(output).unwrap();
        assert_eq!(hunks.len(), 1);
        assert!(hunks[0].header.contains("-1,1 +1,1"));
        assert_eq!(hunks[0].lines.len(), 2);
        assert_eq!(hunks[0].lines[0].kind, crate::types::DiffLineKind::Removed);
        assert_eq!(hunks[0].lines[0].content, "hello");
        assert_eq!(hunks[0].lines[1].kind, crate::types::DiffLineKind::Added);
        assert_eq!(hunks[0].lines[1].content, "modified");
    }

    #[test]
    fn parse_diff_output_new_file() {
        let output = "\
diff --git a/bar.txt b/bar.txt
new file mode 100644
index 0000000..cc628cc
--- /dev/null
+++ b/bar.txt
@@ -0,0 +1,1 @@
+world";

        let hunks = parse_diff_output(output).unwrap();
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].lines.len(), 1);
        assert_eq!(hunks[0].lines[0].kind, crate::types::DiffLineKind::Added);
    }

    #[test]
    fn parse_diff_output_multi_hunk() {
        let output = "\
diff --git a/foo.txt b/foo.txt
--- a/foo.txt
+++ b/foo.txt
@@ -1,3 +1,3 @@
 line1
-old2
+new2
 line3
@@ -10,3 +10,3 @@
 line10
-old11
+new11
 line12";

        let hunks = parse_diff_output(output).unwrap();
        assert_eq!(hunks.len(), 2);
        assert_eq!(hunks[0].lines.len(), 4); // context, removed, added, context
        assert_eq!(hunks[1].lines.len(), 3); // context, removed, added
    }

    #[test]
    fn parse_diff_output_empty() {
        let hunks = parse_diff_output("").unwrap();
        assert!(hunks.is_empty());
    }
```

- [ ] **Step 2: Implement parse_diff_output**

Add before the `impl RepoBackend for JjCliBackend`:

```rust
/// Parse git-format diff output into hunks.
fn parse_diff_output(output: &str) -> Result<Vec<crate::types::DiffHunk>> {
    let mut hunks = Vec::new();
    let mut current_hunk: Option<crate::types::DiffHunk> = None;

    for line in output.lines() {
        if line.starts_with("@@") {
            // Start a new hunk
            if let Some(hunk) = current_hunk.take() {
                hunks.push(hunk);
            }
            current_hunk = Some(crate::types::DiffHunk {
                header: line.to_string(),
                lines: Vec::new(),
            });
        } else if let Some(ref mut hunk) = current_hunk {
            let (kind, content) = if let Some(rest) = line.strip_prefix('+') {
                (crate::types::DiffLineKind::Added, rest)
            } else if let Some(rest) = line.strip_prefix('-') {
                (crate::types::DiffLineKind::Removed, rest)
            } else if let Some(rest) = line.strip_prefix(' ') {
                (crate::types::DiffLineKind::Context, rest)
            } else {
                (crate::types::DiffLineKind::Context, line)
            };
            hunk.lines.push(crate::types::DiffLine {
                kind,
                content: content.to_string(),
            });
        }
        // Lines before the first @@ (headers like "diff --git", "index", "---", "+++") are skipped
        // They could be captured as DiffLineKind::Header if needed in the future
    }

    if let Some(hunk) = current_hunk {
        hunks.push(hunk);
    }

    Ok(hunks)
}
```

- [ ] **Step 3: Implement file_diff on JjCliBackend**

Replace the `todo!()` stub:

```rust
    fn file_diff(&self, change_id: &str, path: &str) -> Result<Vec<crate::types::DiffHunk>> {
        let output = Command::new("jj")
            .args(["diff", "-r", change_id, "--git", "--color=never", path])
            .current_dir(&self.workspace_root)
            .output()
            .with_context(|| format!("Failed to run `jj diff` for {path}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("jj diff failed for {path}: {}", stderr.trim());
        }

        let stdout =
            String::from_utf8(output.stdout).context("jj diff output was not valid UTF-8")?;

        parse_diff_output(&stdout)
    }
```

- [ ] **Step 4: Write integration test**

```rust
    #[test]
    fn file_diff_on_real_repo() {
        if !jj_available() {
            eprintln!("Skipping: jj not in PATH");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        Command::new("jj").args(["git", "init"]).current_dir(tmp.path()).status().unwrap();
        std::fs::write(tmp.path().join("test.txt"), "hello\n").unwrap();
        Command::new("jj").args(["describe", "-m", "add test"]).current_dir(tmp.path()).status().unwrap();

        let backend = JjCliBackend::new(tmp.path()).unwrap();

        // Get the working copy change ID
        let graph = backend.load_graph().unwrap();
        let wc_idx = graph.working_copy_index.unwrap();
        let change_id = graph.lines[wc_idx].change_id.as_ref().unwrap();

        let hunks = backend.file_diff(change_id, "test.txt").unwrap();
        assert!(!hunks.is_empty());
        // Should have at least one added line
        assert!(hunks[0].lines.iter().any(|l| l.kind == crate::types::DiffLineKind::Added));
    }
```

- [ ] **Step 5: Run all core tests**

Run: `cargo test -p lajjzy-core`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/lajjzy-core/src/cli.rs
git commit -m "feat(core): implement file_diff() with git-format diff parser"
```

---

### Task 6: AppState Additions and New Actions

**Files:**
- Modify: `crates/lajjzy-tui/src/app.rs`

- [ ] **Step 1: Add PanelFocus, DetailMode, and new AppState fields**

Add to `app.rs` (after imports, before `AppState`):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelFocus {
    Graph,
    Detail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailMode {
    FileList,
    DiffView,
}
```

Add new fields to `AppState`:

```rust
pub struct AppState {
    pub graph: GraphData,
    cursor: usize,
    pub should_quit: bool,
    pub error: Option<String>,
    pub focus: PanelFocus,
    detail_cursor: usize,
    pub detail_mode: DetailMode,
    pub diff_scroll: usize,
    pub diff_data: Option<(String, Vec<DiffHunk>)>,
}
```

Add import for `DiffHunk` at the top:
```rust
use lajjzy_core::types::{ChangeDetail, DiffHunk, GraphData};
```

- [ ] **Step 2: Update AppState::new() and add new accessors**

Update `new()` to initialize new fields:

```rust
    pub fn new(graph: GraphData) -> Self {
        let cursor = graph
            .working_copy_index
            .unwrap_or_else(|| graph.node_indices().first().copied().unwrap_or(0));
        Self {
            graph,
            cursor,
            should_quit: false,
            error: None,
            focus: PanelFocus::Graph,
            detail_cursor: 0,
            detail_mode: DetailMode::FileList,
            diff_scroll: 0,
            diff_data: None,
        }
    }
```

Add getter:
```rust
    pub fn detail_cursor(&self) -> usize {
        self.detail_cursor
    }
```

Add a helper to reset detail state (used by graph cursor moves):
```rust
    pub fn reset_detail(&mut self) {
        self.detail_cursor = 0;
        self.detail_mode = DetailMode::FileList;
        self.diff_scroll = 0;
        self.diff_data = None;
    }
```

- [ ] **Step 3: Add new Action variants**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    MoveUp,
    MoveDown,
    Quit,
    Refresh,
    JumpToTop,
    JumpToBottom,
    TabFocus,
    BackTabFocus,
    DetailMoveUp,
    DetailMoveDown,
    DetailEnter,
    DetailBack,
    DiffScrollUp,
    DiffScrollDown,
    DiffNextHunk,
    DiffPrevHunk,
    JumpToWorkingCopy,
}
```

- [ ] **Step 4: Add test helper for setting detail_cursor**

In the test module:

```rust
    #[cfg(test)]
    impl AppState {
        fn set_detail_cursor_for_test(&mut self, index: usize) {
            self.detail_cursor = index;
        }
    }
```

(Keep the existing `set_cursor_for_test` as well.)

- [ ] **Step 5: Write tests for new state behavior**

```rust
    #[test]
    fn new_state_initializes_detail_fields() {
        let state = AppState::new(sample_graph());
        assert_eq!(state.focus, PanelFocus::Graph);
        assert_eq!(state.detail_cursor(), 0);
        assert_eq!(state.detail_mode, DetailMode::FileList);
        assert_eq!(state.diff_scroll, 0);
        assert!(state.diff_data.is_none());
    }

    #[test]
    fn tab_focus_toggles() {
        let mock = MockBackend { graph: sample_graph() };
        let mut state = AppState::new(sample_graph());
        assert_eq!(state.focus, PanelFocus::Graph);

        dispatch(&mut state, Action::TabFocus, &mock);
        assert_eq!(state.focus, PanelFocus::Detail);

        dispatch(&mut state, Action::TabFocus, &mock);
        assert_eq!(state.focus, PanelFocus::Graph);
    }

    #[test]
    fn graph_cursor_move_resets_detail() {
        let mock = MockBackend { graph: sample_graph() };
        let mut state = AppState::new(sample_graph());
        state.set_detail_cursor_for_test(2);
        state.detail_mode = DetailMode::DiffView;
        state.diff_data = Some(("test".into(), vec![]));

        dispatch(&mut state, Action::MoveDown, &mock);
        assert_eq!(state.detail_cursor(), 0);
        assert_eq!(state.detail_mode, DetailMode::FileList);
        assert!(state.diff_data.is_none());
    }

    #[test]
    fn jump_to_working_copy() {
        let mock = MockBackend { graph: sample_graph() };
        let mut state = AppState::new(sample_graph());
        state.set_cursor_for_test(4); // at ghi

        dispatch(&mut state, Action::JumpToWorkingCopy, &mock);
        assert_eq!(state.cursor(), 0); // back to abc (working copy)
    }

    #[test]
    fn jump_to_working_copy_noop_when_none() {
        let mock = MockBackend { graph: sample_graph() };
        let mut graph = sample_graph();
        graph.working_copy_index = None;
        let mut state = AppState::new(graph);
        state.set_cursor_for_test(2);

        dispatch(&mut state, Action::JumpToWorkingCopy, &mock);
        assert_eq!(state.cursor(), 2); // unchanged
    }
```

- [ ] **Step 5b: Write tests for DetailBack and DetailEnter**

```rust
    #[test]
    fn detail_back_from_diff_returns_to_file_list() {
        let mock = MockBackend { graph: sample_graph() };
        let mut state = AppState::new(sample_graph());
        state.detail_mode = DetailMode::DiffView;
        state.diff_data = Some(("test.txt".into(), vec![]));
        state.focus = PanelFocus::Detail;

        dispatch(&mut state, Action::DetailBack, &mock);
        assert_eq!(state.detail_mode, DetailMode::FileList);
        assert!(state.diff_data.is_none());
        assert_eq!(state.focus, PanelFocus::Detail); // stays on detail
    }

    #[test]
    fn detail_back_from_file_list_returns_focus_to_graph() {
        let mock = MockBackend { graph: sample_graph() };
        let mut state = AppState::new(sample_graph());
        state.focus = PanelFocus::Detail;
        state.detail_mode = DetailMode::FileList;

        dispatch(&mut state, Action::DetailBack, &mock);
        assert_eq!(state.focus, PanelFocus::Graph);
    }

    #[test]
    fn detail_enter_with_no_files_is_noop() {
        let mock = MockBackend { graph: sample_graph() };
        let mut state = AppState::new(sample_graph());
        // sample_graph has files: vec![] so DetailEnter should be a no-op
        dispatch(&mut state, Action::DetailEnter, &mock);
        assert_eq!(state.detail_mode, DetailMode::FileList);
        assert!(state.diff_data.is_none());
    }
```

- [ ] **Step 6: Update dispatch to handle new actions**

Refactor `dispatch` to delegate. For now, handle all actions inline (panel modules come in Task 9):

```rust
pub fn dispatch(state: &mut AppState, action: Action, backend: &dyn RepoBackend) {
    match action {
        // Global
        Action::TabFocus | Action::BackTabFocus => {
            state.focus = match state.focus {
                PanelFocus::Graph => PanelFocus::Detail,
                PanelFocus::Detail => PanelFocus::Graph,
            };
        }
        Action::Quit => {
            state.should_quit = true;
        }
        Action::Refresh => {
            state.error = None;
            let prev_change_id = state.selected_change_id().map(String::from);
            match backend.load_graph() {
                Ok(new_graph) => {
                    state.graph = new_graph;
                    let nodes = state.graph.node_indices();
                    state.cursor = prev_change_id
                        .as_deref()
                        .and_then(|id| {
                            nodes.iter().find(|&&i| {
                                state.graph.lines[i].change_id.as_deref() == Some(id)
                            }).copied()
                        })
                        .or(state.graph.working_copy_index)
                        .or_else(|| nodes.first().copied())
                        .unwrap_or(0);
                    state.reset_detail();
                }
                Err(e) => {
                    state.error = Some(format!("Refresh failed: {e}"));
                }
            }
        }

        // Graph panel
        Action::MoveDown => {
            let nodes = state.graph.node_indices();
            if let Some(next) = nodes.iter().find(|&&i| i > state.cursor) {
                state.cursor = *next;
                state.reset_detail();
            }
        }
        Action::MoveUp => {
            let nodes = state.graph.node_indices();
            if let Some(prev) = nodes.iter().rev().find(|&&i| i < state.cursor) {
                state.cursor = *prev;
                state.reset_detail();
            }
        }
        Action::JumpToTop => {
            if let Some(&first) = state.graph.node_indices().first() {
                if state.cursor != first {
                    state.cursor = first;
                    state.reset_detail();
                }
            }
        }
        Action::JumpToBottom => {
            if let Some(&last) = state.graph.node_indices().last() {
                if state.cursor != last {
                    state.cursor = last;
                    state.reset_detail();
                }
            }
        }
        Action::JumpToWorkingCopy => {
            if let Some(wc) = state.graph.working_copy_index {
                if state.cursor != wc {
                    state.cursor = wc;
                    state.reset_detail();
                }
            }
        }

        // Detail panel
        Action::DetailMoveDown => {
            if let Some(detail) = state.selected_detail() {
                if state.detail_cursor + 1 < detail.files.len() {
                    state.detail_cursor += 1;
                }
            }
        }
        Action::DetailMoveUp => {
            if state.detail_cursor > 0 {
                state.detail_cursor -= 1;
            }
        }
        Action::DetailEnter => {
            if let (Some(change_id), Some(detail)) = (state.selected_change_id(), state.selected_detail()) {
                if let Some(file) = detail.files.get(state.detail_cursor) {
                    let path = file.path.clone();
                    let change_id = change_id.to_string();
                    match backend.file_diff(&change_id, &path) {
                        Ok(hunks) => {
                            state.diff_data = Some((path, hunks));
                            state.detail_mode = DetailMode::DiffView;
                            state.diff_scroll = 0;
                        }
                        Err(e) => {
                            state.error = Some(format!("Failed to load diff for {path}: {e}"));
                        }
                    }
                }
            }
        }
        Action::DetailBack => {
            match state.detail_mode {
                DetailMode::DiffView => {
                    state.detail_mode = DetailMode::FileList;
                    state.diff_data = None;
                    state.diff_scroll = 0;
                }
                DetailMode::FileList => {
                    state.focus = PanelFocus::Graph;
                }
            }
        }
        Action::DiffScrollDown => {
            if let Some((_, ref hunks)) = state.diff_data {
                let total: usize = hunks.iter().map(|h| 1 + h.lines.len()).sum();
                if state.diff_scroll + 1 < total {
                    state.diff_scroll += 1;
                }
            }
        }
        Action::DiffScrollUp => {
            state.diff_scroll = state.diff_scroll.saturating_sub(1);
        }
        Action::DiffNextHunk => {
            if let Some((_, ref hunks)) = state.diff_data {
                // Calculate line offsets for each hunk
                let mut offset = 0;
                for hunk in hunks {
                    let hunk_start = offset;
                    offset += 1 + hunk.lines.len(); // header + lines
                    if hunk_start > state.diff_scroll {
                        state.diff_scroll = hunk_start;
                        return;
                    }
                }
            }
        }
        Action::DiffPrevHunk => {
            if let Some((_, ref hunks)) = state.diff_data {
                let mut offsets: Vec<usize> = Vec::new();
                let mut offset = 0;
                for hunk in hunks {
                    offsets.push(offset);
                    offset += 1 + hunk.lines.len();
                }
                if let Some(&prev) = offsets.iter().rev().find(|&&o| o < state.diff_scroll) {
                    state.diff_scroll = prev;
                }
            }
        }
    }

    debug_assert!(
        state.graph.lines.get(state.cursor).map_or(true, |l| l.change_id.is_some()),
        "cursor must point to a node line"
    );
}
```

- [ ] **Step 7: Run all tests**

Run: `cargo test -p lajjzy-tui`
Expected: All existing + new tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/lajjzy-tui/src/app.rs
git commit -m "feat(tui): add panel focus, detail mode, new actions and dispatch logic"
```

---

### Task 7: Updated Input Routing

**Files:**
- Modify: `crates/lajjzy-tui/src/input.rs`

- [ ] **Step 1: Update map_event signature and implementation**

Replace the entire `map_event` function:

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{Action, DetailMode, PanelFocus};

/// Map a crossterm key event to an Action, considering panel focus and detail mode.
pub fn map_event(event: KeyEvent, focus: PanelFocus, detail_mode: DetailMode) -> Option<Action> {
    // Global keys — work regardless of focus
    match (event.code, event.modifiers) {
        (KeyCode::Char('q'), KeyModifiers::NONE)
        | (KeyCode::Char('c'), KeyModifiers::CONTROL) => return Some(Action::Quit),
        (KeyCode::Tab, _) => return Some(Action::TabFocus),
        (KeyCode::BackTab, _) => return Some(Action::BackTabFocus),
        (KeyCode::Char('R'), _) => return Some(Action::Refresh),
        (KeyCode::Char('@'), _) => return Some(Action::JumpToWorkingCopy),
        _ => {}
    }

    // Context-sensitive keys
    match focus {
        PanelFocus::Graph => match (event.code, event.modifiers) {
            (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => {
                Some(Action::MoveDown)
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => Some(Action::MoveUp),
            (KeyCode::Char('g'), KeyModifiers::NONE) => Some(Action::JumpToTop),
            (KeyCode::Char('G'), _) => Some(Action::JumpToBottom),
            _ => None,
        },
        PanelFocus::Detail => match detail_mode {
            DetailMode::FileList => match (event.code, event.modifiers) {
                (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => {
                    Some(Action::DetailMoveDown)
                }
                (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => {
                    Some(Action::DetailMoveUp)
                }
                (KeyCode::Enter, _) => Some(Action::DetailEnter),
                (KeyCode::Esc, _) => Some(Action::DetailBack),
                _ => None,
            },
            DetailMode::DiffView => match (event.code, event.modifiers) {
                (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => {
                    Some(Action::DiffScrollDown)
                }
                (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => {
                    Some(Action::DiffScrollUp)
                }
                (KeyCode::Char('n'), KeyModifiers::NONE) => Some(Action::DiffNextHunk),
                (KeyCode::Char('N'), _) => Some(Action::DiffPrevHunk),
                (KeyCode::Esc, _) => Some(Action::DetailBack),
                _ => None,
            },
        },
    }
}
```

- [ ] **Step 2: Update tests**

Replace the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn key_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    // Global keys
    #[test]
    fn quit_keys_work_in_any_focus() {
        for focus in [PanelFocus::Graph, PanelFocus::Detail] {
            assert_eq!(map_event(key(KeyCode::Char('q')), focus, DetailMode::FileList), Some(Action::Quit));
            assert_eq!(
                map_event(key_mod(KeyCode::Char('c'), KeyModifiers::CONTROL), focus, DetailMode::FileList),
                Some(Action::Quit),
            );
        }
    }

    #[test]
    fn tab_cycles_focus() {
        assert_eq!(map_event(key(KeyCode::Tab), PanelFocus::Graph, DetailMode::FileList), Some(Action::TabFocus));
        assert_eq!(map_event(key(KeyCode::BackTab), PanelFocus::Detail, DetailMode::FileList), Some(Action::BackTabFocus));
    }

    #[test]
    fn refresh_and_jump_working_copy_are_global() {
        assert_eq!(map_event(key(KeyCode::Char('R')), PanelFocus::Detail, DetailMode::DiffView), Some(Action::Refresh));
        assert_eq!(map_event(key(KeyCode::Char('@')), PanelFocus::Detail, DetailMode::FileList), Some(Action::JumpToWorkingCopy));
    }

    // Graph focus
    #[test]
    fn graph_navigation() {
        assert_eq!(map_event(key(KeyCode::Char('j')), PanelFocus::Graph, DetailMode::FileList), Some(Action::MoveDown));
        assert_eq!(map_event(key(KeyCode::Char('k')), PanelFocus::Graph, DetailMode::FileList), Some(Action::MoveUp));
        assert_eq!(map_event(key(KeyCode::Char('g')), PanelFocus::Graph, DetailMode::FileList), Some(Action::JumpToTop));
        assert_eq!(map_event(key(KeyCode::Char('G')), PanelFocus::Graph, DetailMode::FileList), Some(Action::JumpToBottom));
    }

    // Detail focus — file list
    #[test]
    fn detail_file_list_navigation() {
        assert_eq!(map_event(key(KeyCode::Char('j')), PanelFocus::Detail, DetailMode::FileList), Some(Action::DetailMoveDown));
        assert_eq!(map_event(key(KeyCode::Char('k')), PanelFocus::Detail, DetailMode::FileList), Some(Action::DetailMoveUp));
        assert_eq!(map_event(key(KeyCode::Enter), PanelFocus::Detail, DetailMode::FileList), Some(Action::DetailEnter));
        assert_eq!(map_event(key(KeyCode::Esc), PanelFocus::Detail, DetailMode::FileList), Some(Action::DetailBack));
    }

    // Detail focus — diff view
    #[test]
    fn detail_diff_view_navigation() {
        assert_eq!(map_event(key(KeyCode::Char('j')), PanelFocus::Detail, DetailMode::DiffView), Some(Action::DiffScrollDown));
        assert_eq!(map_event(key(KeyCode::Char('k')), PanelFocus::Detail, DetailMode::DiffView), Some(Action::DiffScrollUp));
        assert_eq!(map_event(key(KeyCode::Char('n')), PanelFocus::Detail, DetailMode::DiffView), Some(Action::DiffNextHunk));
        assert_eq!(map_event(key(KeyCode::Char('N')), PanelFocus::Detail, DetailMode::DiffView), Some(Action::DiffPrevHunk));
        assert_eq!(map_event(key(KeyCode::Esc), PanelFocus::Detail, DetailMode::DiffView), Some(Action::DetailBack));
    }

    #[test]
    fn same_key_different_action_by_context() {
        // j in graph = MoveDown, j in detail file list = DetailMoveDown, j in diff = DiffScrollDown
        assert_eq!(map_event(key(KeyCode::Char('j')), PanelFocus::Graph, DetailMode::FileList), Some(Action::MoveDown));
        assert_eq!(map_event(key(KeyCode::Char('j')), PanelFocus::Detail, DetailMode::FileList), Some(Action::DetailMoveDown));
        assert_eq!(map_event(key(KeyCode::Char('j')), PanelFocus::Detail, DetailMode::DiffView), Some(Action::DiffScrollDown));
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p lajjzy-tui input`
Expected: All input tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/lajjzy-tui/src/input.rs
git commit -m "feat(tui): context-sensitive input routing with focus and detail mode"
```

---

### Task 8: New Widgets — FileListWidget and DiffViewWidget

**Files:**
- Create: `crates/lajjzy-tui/src/widgets/file_list.rs`
- Create: `crates/lajjzy-tui/src/widgets/diff_view.rs`
- Modify: `crates/lajjzy-tui/src/widgets/mod.rs`

- [ ] **Step 1: Create FileListWidget**

`crates/lajjzy-tui/src/widgets/file_list.rs`:

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Widget;

use lajjzy_core::types::{FileChange, FileStatus};

pub struct FileListWidget<'a> {
    files: &'a [FileChange],
    cursor: usize,
    focused: bool,
}

impl<'a> FileListWidget<'a> {
    pub fn new(files: &'a [FileChange], cursor: usize, focused: bool) -> Self {
        Self {
            files,
            cursor,
            focused,
        }
    }

    fn status_color(status: FileStatus) -> Color {
        match status {
            FileStatus::Added => Color::Green,
            FileStatus::Modified => Color::Yellow,
            FileStatus::Deleted => Color::Red,
            FileStatus::Renamed => Color::Cyan,
        }
    }
}

impl Widget for FileListWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.files.is_empty() {
            let msg = Line::styled("(no files changed)", Style::default().fg(Color::DarkGray));
            buf.set_line(area.x, area.y, &msg, area.width);
            return;
        }

        let highlight = if self.focused {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        };

        for (i, file) in self.files.iter().enumerate() {
            if i >= area.height as usize {
                break;
            }

            let y = area.y + i as u16;
            let status_str = format!("{}", file.status);
            let line_text = format!("  {} {}", status_str, file.path);
            let color = Self::status_color(file.status);

            let style = if i == self.cursor {
                highlight.fg(color)
            } else {
                Style::default().fg(color)
            };

            let line = Line::styled(&line_text, style);
            buf.set_line(area.x, y, &line, area.width);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_files() -> Vec<FileChange> {
        vec![
            FileChange { path: "bar.txt".into(), status: FileStatus::Added },
            FileChange { path: "foo.txt".into(), status: FileStatus::Modified },
        ]
    }

    #[test]
    fn renders_file_entries() {
        let files = sample_files();
        let widget = FileListWidget::new(&files, 0, true);
        let area = Rect::new(0, 0, 40, 4);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..40).map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' ')).collect();
        assert!(line0.contains("A"));
        assert!(line0.contains("bar.txt"));
    }

    #[test]
    fn renders_empty_files() {
        let widget = FileListWidget::new(&[], 0, false);
        let area = Rect::new(0, 0, 40, 2);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..40).map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' ')).collect();
        assert!(line0.contains("no files"));
    }
}
```

- [ ] **Step 2: Create DiffViewWidget**

`crates/lajjzy-tui/src/widgets/diff_view.rs`:

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Widget;

use lajjzy_core::types::{DiffHunk, DiffLineKind};

pub struct DiffViewWidget<'a> {
    hunks: &'a [DiffHunk],
    scroll: usize,
}

impl<'a> DiffViewWidget<'a> {
    pub fn new(hunks: &'a [DiffHunk], scroll: usize) -> Self {
        Self { hunks, scroll }
    }

    /// Flatten hunks into a list of (kind, content) for rendering.
    fn flat_lines(&self) -> Vec<(DiffLineKind, &str)> {
        let mut lines = Vec::new();
        for hunk in self.hunks {
            lines.push((DiffLineKind::Header, hunk.header.as_str()));
            for dl in &hunk.lines {
                lines.push((dl.kind, &dl.content));
            }
        }
        lines
    }
}

impl Widget for DiffViewWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.hunks.is_empty() {
            let msg = Line::styled("(empty diff)", Style::default().fg(Color::DarkGray));
            buf.set_line(area.x, area.y, &msg, area.width);
            return;
        }

        let flat = self.flat_lines();
        let height = area.height as usize;

        for (row, idx) in (self.scroll..self.scroll + height).enumerate() {
            if idx >= flat.len() {
                break;
            }

            let (kind, content) = flat[idx];
            let prefix = match kind {
                DiffLineKind::Added => "+",
                DiffLineKind::Removed => "-",
                DiffLineKind::Context => " ",
                DiffLineKind::Header => "",
            };
            let text = if prefix.is_empty() {
                content.to_string()
            } else {
                format!("{prefix}{content}")
            };

            let style = match kind {
                DiffLineKind::Added => Style::default().fg(Color::Green),
                DiffLineKind::Removed => Style::default().fg(Color::Red),
                DiffLineKind::Context => Style::default(),
                DiffLineKind::Header => Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
            };

            let y = area.y + row as u16;
            let line = Line::styled(&text, style);
            buf.set_line(area.x, y, &line, area.width);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lajjzy_core::types::DiffLine;

    fn sample_hunks() -> Vec<DiffHunk> {
        vec![DiffHunk {
            header: "@@ -1,1 +1,1 @@".into(),
            lines: vec![
                DiffLine { kind: DiffLineKind::Removed, content: "hello".into() },
                DiffLine { kind: DiffLineKind::Added, content: "world".into() },
            ],
        }]
    }

    #[test]
    fn renders_diff_lines() {
        let hunks = sample_hunks();
        let widget = DiffViewWidget::new(&hunks, 0);
        let area = Rect::new(0, 0, 40, 5);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // First line should be the hunk header
        let line0: String = (0..40).map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' ')).collect();
        assert!(line0.contains("@@"));
    }

    #[test]
    fn renders_empty_diff() {
        let widget = DiffViewWidget::new(&[], 0);
        let area = Rect::new(0, 0, 40, 2);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..40).map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' ')).collect();
        assert!(line0.contains("empty diff"));
    }
}
```

- [ ] **Step 3: Update widgets/mod.rs**

```rust
pub mod diff_view;
pub mod file_list;
pub mod graph;
pub mod status_bar;
```

- [ ] **Step 4: Run widget tests**

Run: `cargo test -p lajjzy-tui widgets`
Expected: All widget tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/lajjzy-tui/src/widgets/
git commit -m "feat(tui): add FileListWidget and DiffViewWidget"
```

---

### Task 9: Panel Modules

**Files:**
- Create: `crates/lajjzy-tui/src/panels/mod.rs`
- Create: `crates/lajjzy-tui/src/panels/graph.rs`
- Create: `crates/lajjzy-tui/src/panels/detail.rs`
- Modify: `crates/lajjzy-tui/src/lib.rs`

The panel modules handle rendering only (for now). Action handling stays in `dispatch` in `app.rs` until the panel module refactor in a future task. This keeps the change incremental.

- [ ] **Step 1: Create panels/mod.rs**

```rust
pub mod detail;
pub mod graph;
```

- [ ] **Step 2: Create panels/graph.rs**

```rust
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders};
use ratatui::Frame;

use crate::app::{AppState, PanelFocus};
use crate::widgets::graph::GraphWidget;

pub fn render(frame: &mut Frame, state: &AppState, area: Rect) {
    let focused = state.focus == PanelFocus::Graph;
    let border_style = if focused {
        Style::default().fg(Color::Blue)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title("Changes");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let graph_widget = GraphWidget::new(&state.graph, state.cursor());
    frame.render_widget(graph_widget, inner);
}
```

- [ ] **Step 3: Create panels/detail.rs**

```rust
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders};
use ratatui::Frame;

use crate::app::{AppState, DetailMode, PanelFocus};
use crate::widgets::diff_view::DiffViewWidget;
use crate::widgets::file_list::FileListWidget;

pub fn render(frame: &mut Frame, state: &AppState, area: Rect) {
    let focused = state.focus == PanelFocus::Detail;
    let border_style = if focused {
        Style::default().fg(Color::Blue)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = match state.detail_mode {
        DetailMode::FileList => {
            if let Some(detail) = state.selected_detail() {
                let desc = if detail.description.is_empty() {
                    "(no description)"
                } else {
                    &detail.description
                };
                format!("Files — {desc}")
            } else {
                "Files".to_string()
            }
        }
        DetailMode::DiffView => {
            if let Some((ref path, _)) = state.diff_data {
                format!("Diff — {path}")
            } else {
                "Diff".to_string()
            }
        }
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(title);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    match state.detail_mode {
        DetailMode::FileList => {
            let files = state
                .selected_detail()
                .map(|d| d.files.as_slice())
                .unwrap_or(&[]);
            let widget = FileListWidget::new(files, state.detail_cursor(), focused);
            frame.render_widget(widget, inner);
        }
        DetailMode::DiffView => {
            if let Some((_, ref hunks)) = state.diff_data {
                let widget = DiffViewWidget::new(hunks, state.diff_scroll);
                frame.render_widget(widget, inner);
            }
        }
    }
}
```

- [ ] **Step 4: Update lib.rs**

Add `pub mod panels;` to `crates/lajjzy-tui/src/lib.rs`:

```rust
pub mod app;
pub mod input;
pub mod panels;
pub mod render;
pub mod widgets;
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo build -p lajjzy-tui`

- [ ] **Step 6: Commit**

```bash
git add crates/lajjzy-tui/src/panels/ crates/lajjzy-tui/src/lib.rs
git commit -m "feat(tui): add graph and detail panel modules"
```

---

### Task 10: Updated Render with Split Layout

**Files:**
- Modify: `crates/lajjzy-tui/src/render.rs`

- [ ] **Step 1: Replace render function**

```rust
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

use crate::app::AppState;
use crate::panels;
use crate::widgets::status_bar::StatusBarWidget;

const STATUS_BAR_HEIGHT: u16 = 2;

pub fn render(frame: &mut Frame, state: &AppState) {
    let outer = Layout::vertical([Constraint::Min(1), Constraint::Length(STATUS_BAR_HEIGHT)])
        .split(frame.area());

    let main =
        Layout::horizontal([Constraint::Ratio(1, 3), Constraint::Ratio(2, 3)]).split(outer[0]);

    panels::graph::render(frame, state, main[0]);
    panels::detail::render(frame, state, main[1]);

    let change_id = state.selected_change_id();
    let detail = state.selected_detail();
    let error = state.error.as_deref();
    let status_widget = StatusBarWidget::new(change_id, detail, error);
    frame.render_widget(status_widget, outer[1]);
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build -p lajjzy-tui`

- [ ] **Step 3: Commit**

```bash
git add crates/lajjzy-tui/src/render.rs
git commit -m "feat(tui): split layout 1/3 graph + 2/3 detail with panel borders"
```

---

### Task 11: Update main.rs

**Files:**
- Modify: `crates/lajjzy-cli/src/main.rs`

- [ ] **Step 1: Update map_event call to pass focus and detail_mode**

The `run_loop` function needs to pass `state.focus` and `state.detail_mode` to `map_event`:

```rust
fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    state: &mut AppState,
    backend: &JjCliBackend,
) -> Result<()> {
    loop {
        terminal.draw(|frame| render(frame, state))?;

        if let Event::Key(key_event) = event::read()? {
            if key_event.kind != KeyEventKind::Press {
                continue;
            }
            if let Some(action) = map_event(key_event, state.focus, state.detail_mode) {
                dispatch(state, action, backend);
            }
        }

        if state.should_quit {
            break;
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build -p lajjzy`

- [ ] **Step 3: Commit**

```bash
git add crates/lajjzy-cli/src/main.rs
git commit -m "feat(cli): pass panel focus and detail mode to input routing"
```

---

### Task 12: Final Integration and Cleanup

**Files:**
- Various (clippy fixes, formatting)

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests pass (existing 36 + new tests).

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: Clean. Fix any warnings.

- [ ] **Step 3: Run formatter**

Run: `cargo fmt`
Then: `cargo fmt --check`

- [ ] **Step 4: Commit any fixes**

```bash
git add -A
git commit -m "chore: fix clippy warnings and formatting"
```

- [ ] **Step 5: Update CLAUDE.md**

Add note about panel modules and M1a features. Add the new key bindings.

- [ ] **Step 6: Manual smoke test**

In a jj repo with file changes:
```bash
cargo run -p lajjzy
```

Verify:
- [ ] Split layout: graph on left (1/3), detail on right (2/3)
- [ ] File list shows for the selected change
- [ ] Moving graph cursor (`j`/`k`) updates file list
- [ ] `Tab` moves focus to detail pane (border changes)
- [ ] `j`/`k` in file list navigates files
- [ ] `Enter` on a file shows diff
- [ ] `Esc` returns from diff to file list
- [ ] `Esc` from file list returns focus to graph
- [ ] `@` jumps to working copy change
- [ ] `n`/`N` in diff view jumps between hunks
- [ ] `q` quits cleanly
- [ ] Status bar still shows change info
- [ ] Error handling: if diff fails, error shows in status bar

- [ ] **Step 7: Final commit**

```bash
git add -A
git commit -m "docs: update CLAUDE.md for M1a features"
```
