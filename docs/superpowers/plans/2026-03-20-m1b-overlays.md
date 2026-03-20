# M1b Overlays and Graph Visual Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add modal overlays (op log, bookmark picker, fuzzy-find, help), compact the graph, and colorize graph output.

**Architecture:** Extend the existing three-crate workspace. Core layer gets `op_log()` on RepoBackend and `OpLogEntry` type. TUI layer gets a modal system (`Option<Modal>` on AppState), separate `map_modal_event()` for modal input routing, modal widgets, and colorized graph rendering. `nucleo-matcher` added for fuzzy-find.

**Tech Stack:** Rust (stable), ratatui 0.30, crossterm 0.29, nucleo-matcher, anyhow

**Spec:** `docs/superpowers/specs/2026-03-20-m1b-overlays-design.md`

---

## File Structure

```
crates/
├── lajjzy-core/src/
│   ├── types.rs                    # MODIFY: add OpLogEntry, add glyph_prefix to GraphLine
│   ├── backend.rs                  # MODIFY: add op_log() to RepoBackend
│   └── cli.rs                      # MODIFY: graph compaction (don't add file lines to GraphData.lines),
│                                   #         glyph_prefix extraction, op_log() impl
├── lajjzy-tui/
│   ├── Cargo.toml                  # MODIFY: add nucleo-matcher dependency
│   └── src/
│       ├── app.rs                  # MODIFY: add Modal, HelpContext enums, modal field on AppState,
│       │                           #         new Action variants, modal dispatch logic
│       ├── input.rs                # MODIFY: add map_modal_event(), add modal trigger keys to map_event()
│       ├── render.rs               # MODIFY: dim background when modal active, render modal overlay
│       ├── panels/
│       │   └── graph.rs            # MODIFY: colorized rendering using ChangeDetail + glyph_prefix
│       └── widgets/
│           ├── mod.rs              # MODIFY: add modal widget modules
│           ├── op_log.rs           # CREATE: OpLogWidget
│           ├── bookmark_picker.rs  # CREATE: BookmarkPickerWidget
│           ├── fuzzy_find.rs       # CREATE: FuzzyFindWidget
│           └── help.rs             # CREATE: HelpWidget
└── lajjzy-cli/src/
    └── main.rs                     # MODIFY: modal-aware input routing
```

---

### Task 1: Graph Compaction — Hide File Lines from Graph

**Files:**
- Modify: `crates/lajjzy-core/src/cli.rs`

In `parse_graph_output()`, when a continuation line matches `parse_file_line()`, parse it into the change's files but do NOT push it to `lines`. This makes the graph compact.

- [ ] **Step 1: Update parser — don't add file lines to GraphData.lines**

In `parse_graph_output()` (around line 150), change the file-line branch from:

```rust
        } else if let Some(file_change) = parse_file_line(raw_line) {
            if let Some(last_id) = &current_change_id {
                if let Some(detail) = details.get_mut(last_id) {
                    detail.files.push(file_change);
                }
            }
            lines.push(crate::types::GraphLine {
                raw: raw_line.to_string(),
                change_id: None,
            });
```

To:

```rust
        } else if let Some(file_change) = parse_file_line(raw_line) {
            if let Some(last_id) = &current_change_id {
                if let Some(detail) = details.get_mut(last_id) {
                    detail.files.push(file_change);
                }
            }
            // File lines are NOT added to GraphData.lines — they only live in ChangeDetail.files.
            // This keeps the graph compact (lazygit-style).
```

- [ ] **Step 2: Update tests**

`parse_graph_output_with_file_summary` — update assertions. Lines no longer include file entries:
- `graph.lines.len()` decreases (was counting file lines, now doesn't)
- File data still in `detail.files` (unchanged)
- `graph.node_indices()` still correct

`parse_graph_output_rename` — the `R {foo.txt => bar.txt}` line won't be in `lines`. Update to only assert on `detail.files`.

- [ ] **Step 3: Run tests**

Run: `cargo test -p lajjzy-core`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(core): compact graph by hiding file summary lines from GraphData.lines"
```

---

### Task 2: Add glyph_prefix to GraphLine

**Files:**
- Modify: `crates/lajjzy-core/src/types.rs`
- Modify: `crates/lajjzy-core/src/cli.rs`

Add a `glyph_prefix` field to `GraphLine` for colorized rendering.

- [ ] **Step 1: Add field to GraphLine**

In `types.rs`, update `GraphLine`:

```rust
pub struct GraphLine {
    /// The full display string (graph glyphs + text), delimiter stripped.
    pub raw: String,
    /// Graph glyph prefix (e.g., "@  ", "○  ", "│  "). Extracted during parsing.
    pub glyph_prefix: String,
    /// The change ID if this is a node line.
    pub change_id: Option<String>,
}
```

- [ ] **Step 2: Update all GraphLine construction sites**

In `cli.rs` `parse_graph_output()`, for node lines (the `\x1F` branch), extract the glyph prefix — everything in `display` before the first alphanumeric character (the start of the change ID):

```rust
let glyph_end = display
    .find(|c: char| c.is_alphanumeric())
    .unwrap_or(0);
let glyph_prefix = display[..glyph_end].to_string();
```

For connector lines (the `else` branch), `glyph_prefix` is the entire `raw` string:

```rust
lines.push(crate::types::GraphLine {
    raw: raw_line.to_string(),
    glyph_prefix: raw_line.to_string(),
    change_id: None,
});
```

Also update every `GraphLine { raw, change_id }` literal in test code to include `glyph_prefix`:
- `types.rs` test module `sample_graph()`
- `cli.rs` test assertions
- `app.rs` test module `sample_graph()` and `sample_graph_with_files()`
- `widgets/graph.rs` test module `simple_graph()`

For test fixtures, use `glyph_prefix: String::new()` unless the test specifically needs glyph rendering.

- [ ] **Step 3: Run all tests**

Run: `cargo test`
Expected: All 66 tests pass.

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(core): add glyph_prefix field to GraphLine for colorized rendering"
```

---

### Task 3: Graph Colorization

**Files:**
- Modify: `crates/lajjzy-tui/src/panels/graph.rs`
- Modify: `crates/lajjzy-tui/src/widgets/graph.rs`

Replace plain-text graph rendering with colored spans.

- [ ] **Step 1: Update GraphWidget to render colored node lines**

In `widgets/graph.rs`, update the `render` method. For each line:

If `line.change_id.is_some()`, look up the detail from the graph and render colored spans:
- Glyph prefix: dark gray (or green bold for working copy)
- Change ID: yellow
- Author: blue
- Timestamp: cyan
- Bookmarks: magenta in brackets

If `line.change_id.is_none()` (connector), render `raw` in dark gray.

The `GraphWidget` needs access to `GraphData` (it already has it via `graph: &'a GraphData`), `working_copy_index`, and the details map.

```rust
use ratatui::text::{Line, Span};

// In render(), replace the plain Line::raw rendering:
let display = if let Some(ref cid) = line.change_id {
    let is_wc = Some(line_idx) == self.graph.working_copy_index;
    let glyph_style = if is_wc {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let mut spans = vec![Span::styled(&line.glyph_prefix, glyph_style)];

    if let Some(detail) = self.graph.details.get(cid) {
        spans.push(Span::styled(cid, Style::default().fg(Color::Yellow)));
        spans.push(Span::raw("  "));
        spans.push(Span::styled(&detail.author, Style::default().fg(Color::Blue)));
        spans.push(Span::raw("  "));
        spans.push(Span::styled(&detail.timestamp, Style::default().fg(Color::Cyan)));
        if !detail.bookmarks.is_empty() {
            spans.push(Span::raw("  "));
            let bm = format!("[{}]", detail.bookmarks.join(", "));
            spans.push(Span::styled(bm, Style::default().fg(Color::Magenta)));
        }
    } else {
        // Fallback: render raw text
        spans.push(Span::raw(&line.raw[line.glyph_prefix.len()..]));
    }
    Line::from(spans)
} else {
    Line::styled(&line.raw, Style::default().fg(Color::DarkGray))
};
```

- [ ] **Step 2: Update panels/graph.rs if needed**

The panel just calls `GraphWidget::new()` and renders it. No changes needed unless the widget API changes.

- [ ] **Step 3: Update graph widget tests**

The existing `highlighted_lines_have_reversed_style` test checks modifier on buffer cells. It may need updating since colored spans change the base styles. Update assertions to verify the REVERSED modifier is still applied on top of the colored styles.

- [ ] **Step 4: Run tests**

Run: `cargo test -p lajjzy-tui`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(tui): colorize graph with semantic colors for change ID, author, timestamp, bookmarks"
```

---

### Task 4: OpLogEntry Type + RepoBackend Extension + op_log() Implementation

**Files:**
- Modify: `crates/lajjzy-core/src/types.rs`
- Modify: `crates/lajjzy-core/src/backend.rs`
- Modify: `crates/lajjzy-core/src/cli.rs`
- Modify: `crates/lajjzy-tui/src/app.rs` (MockBackend + FailingBackend)

- [ ] **Step 1: Add OpLogEntry type**

In `types.rs`, add:

```rust
/// An entry in the jj operation log.
#[derive(Debug, Clone)]
pub struct OpLogEntry {
    pub id: String,
    pub description: String,
    pub timestamp: String,
}
```

- [ ] **Step 2: Add op_log() to RepoBackend trait**

In `backend.rs`:

```rust
    /// Load the operation log.
    fn op_log(&self) -> Result<Vec<crate::types::OpLogEntry>>;
```

- [ ] **Step 3: Implement op_log() on JjCliBackend**

In `cli.rs`, add a parser and implementation. Add `parse_op_log_output()` function:

```rust
fn parse_op_log_output(output: &str) -> Result<Vec<crate::types::OpLogEntry>> {
    let mut entries = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(sep_pos) = line.find(UNIT_SEP) {
            let metadata = &line[sep_pos + UNIT_SEP.len_utf8()..];
            let fields: Vec<&str> = metadata.split(RECORD_SEP).collect();
            if fields.len() < 3 {
                bail!("Expected 3 op log fields, got {}: {:?}", fields.len(), fields);
            }
            entries.push(crate::types::OpLogEntry {
                id: fields[0].to_string(),
                description: fields[1].to_string(),
                timestamp: fields[2].to_string(),
            });
        }
    }
    Ok(entries)
}
```

In `impl RepoBackend for JjCliBackend`:

```rust
    fn op_log(&self) -> Result<Vec<crate::types::OpLogEntry>> {
        let template = concat!(
            "\"\\x1f\"",
            " ++ self.id().short(8)",
            " ++ \"\\x1e\" ++ description",
            " ++ \"\\x1e\" ++ self.time().start().ago()",
        );

        let output = Command::new("jj")
            .args(["op", "log", "--no-graph", "--color=never", "-T", template])
            .current_dir(&self.workspace_root)
            .output()
            .context("Failed to run `jj op log`")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("jj op log failed: {}", stderr.trim());
        }

        let stdout = String::from_utf8(output.stdout)
            .context("jj op log output was not valid UTF-8")?;

        parse_op_log_output(&stdout)
    }
```

**IMPORTANT:** The exact `jj op log` template syntax needs validation against real jj 0.39.0. The implementer should test the template manually first:
```bash
jj op log --no-graph --color=never -T 'self.id().short(8) ++ " " ++ description ++ " " ++ self.time().start().ago()'
```
and adjust the template if needed.

- [ ] **Step 4: Update MockBackend and FailingBackend in app.rs**

```rust
    // MockBackend:
    fn op_log(&self) -> Result<Vec<lajjzy_core::types::OpLogEntry>> {
        Ok(vec![])
    }

    // FailingBackend:
    fn op_log(&self) -> Result<Vec<lajjzy_core::types::OpLogEntry>> {
        anyhow::bail!("connection lost")
    }

    // DiffMockBackend:
    fn op_log(&self) -> Result<Vec<lajjzy_core::types::OpLogEntry>> {
        Ok(vec![])
    }
```

- [ ] **Step 5: Write tests**

In `cli.rs` tests:

```rust
    #[test]
    fn parse_op_log_output_basic() {
        let output = "\x1Fabc12345\x1Ecreate bookmark main\x1E2 hours ago\n\x1Fdef67890\x1Esnapshot working copy\x1E3 hours ago\n";
        let entries = parse_op_log_output(output).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "abc12345");
        assert!(entries[0].description.contains("bookmark"));
        assert_eq!(entries[1].id, "def67890");
    }

    #[test]
    fn parse_op_log_output_empty() {
        let entries = parse_op_log_output("").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn op_log_on_real_repo() {
        if !jj_available() {
            eprintln!("Skipping: jj not in PATH");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        Command::new("jj").args(["git", "init"]).current_dir(tmp.path()).status().unwrap();

        let backend = JjCliBackend::new(tmp.path()).unwrap();
        let entries = backend.op_log().unwrap();
        assert!(!entries.is_empty()); // at least the init operation
    }
```

- [ ] **Step 6: Run all tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 7: Commit**

```bash
git commit -m "feat(core): add OpLogEntry type and op_log() to RepoBackend"
```

---

### Task 5: Modal System Foundation

**Files:**
- Modify: `crates/lajjzy-tui/src/app.rs`
- Modify: `crates/lajjzy-tui/src/input.rs`
- Modify: `crates/lajjzy-tui/src/render.rs`
- Modify: `crates/lajjzy-cli/src/main.rs`

This is the central architectural task. It adds the modal enum, modal field on AppState, modal actions, `map_modal_event()`, and modal-aware rendering.

- [ ] **Step 1: Add Modal and HelpContext enums to app.rs**

Add after `DetailMode`:

```rust
#[derive(Debug, Clone)]
pub enum Modal {
    OpLog {
        entries: Vec<lajjzy_core::types::OpLogEntry>,
        cursor: usize,
        scroll: usize,
    },
    BookmarkPicker {
        bookmarks: Vec<(String, String)>,  // (bookmark_name, change_id)
        cursor: usize,
    },
    FuzzyFind {
        query: String,
        matches: Vec<usize>,  // graph line indices from node_indices
        cursor: usize,
    },
    Help {
        context: HelpContext,
        scroll: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpContext {
    Graph,
    DetailFileList,
    DetailDiffView,
}
```

- [ ] **Step 2: Add modal field to AppState and new Action variants**

Add to AppState:
```rust
    pub modal: Option<Modal>,
```

Initialize in `new()`:
```rust
    modal: None,
```

Add new Action variants:
```rust
pub enum Action {
    // ... existing variants ...
    ToggleOpLog,
    OpenBookmarks,
    OpenFuzzyFind,
    OpenHelp,
    ModalDismiss,
    ModalMoveUp,
    ModalMoveDown,
    ModalEnter,
    FuzzyInput(char),
    FuzzyBackspace,
}
```

- [ ] **Step 3: Add modal dispatch handling**

Add modal action handling to `dispatch()`. The modal trigger actions open the modal; the modal navigation actions modify the modal state.

Add these match arms to `dispatch()`:

```rust
        // Modal triggers
        Action::ToggleOpLog => {
            if matches!(state.modal, Some(Modal::OpLog { .. })) {
                state.modal = None;
            } else {
                match backend.op_log() {
                    Ok(entries) => {
                        state.modal = Some(Modal::OpLog { entries, cursor: 0, scroll: 0 });
                    }
                    Err(e) => {
                        state.error = Some(format!("Failed to load op log: {e}"));
                    }
                }
            }
        }
        Action::OpenBookmarks => {
            let mut bookmarks = Vec::new();
            for &idx in state.graph.node_indices() {
                if let Some(cid) = state.graph.lines[idx].change_id.as_ref() {
                    if let Some(detail) = state.graph.details.get(cid) {
                        for bm in &detail.bookmarks {
                            bookmarks.push((bm.clone(), cid.clone()));
                        }
                    }
                }
            }
            state.modal = Some(Modal::BookmarkPicker { bookmarks, cursor: 0 });
        }
        Action::OpenFuzzyFind => {
            let matches = state.graph.node_indices().to_vec();
            state.modal = Some(Modal::FuzzyFind {
                query: String::new(),
                matches,
                cursor: 0,
            });
        }
        Action::OpenHelp => {
            let context = match state.focus {
                PanelFocus::Graph => HelpContext::Graph,
                PanelFocus::Detail => match state.detail_mode {
                    DetailMode::FileList => HelpContext::DetailFileList,
                    DetailMode::DiffView => HelpContext::DetailDiffView,
                },
            };
            state.modal = Some(Modal::Help { context, scroll: 0 });
        }

        // Modal navigation
        Action::ModalDismiss => {
            state.modal = None;
        }
        Action::ModalMoveDown => {
            if let Some(ref mut modal) = state.modal {
                match modal {
                    Modal::OpLog { entries, cursor, .. } => {
                        if *cursor + 1 < entries.len() { *cursor += 1; }
                    }
                    Modal::BookmarkPicker { bookmarks, cursor } => {
                        if *cursor + 1 < bookmarks.len() { *cursor += 1; }
                    }
                    Modal::FuzzyFind { matches, cursor, .. } => {
                        if *cursor + 1 < matches.len() { *cursor += 1; }
                    }
                    Modal::Help { scroll, .. } => { *scroll += 1; }
                }
            }
        }
        Action::ModalMoveUp => {
            if let Some(ref mut modal) = state.modal {
                match modal {
                    Modal::OpLog { cursor, .. }
                    | Modal::BookmarkPicker { cursor, .. }
                    | Modal::FuzzyFind { cursor, .. } => {
                        *cursor = cursor.saturating_sub(1);
                    }
                    Modal::Help { scroll, .. } => {
                        *scroll = scroll.saturating_sub(1);
                    }
                }
            }
        }
        Action::ModalEnter => {
            // Take the modal to avoid borrow issues
            let modal = state.modal.take();
            match modal {
                Some(Modal::BookmarkPicker { bookmarks, cursor, .. }) => {
                    if let Some((_, change_id)) = bookmarks.get(cursor) {
                        // Find the node index with this change_id
                        if let Some(&idx) = state.graph.node_indices().iter().find(|&&i| {
                            state.graph.lines[i].change_id.as_deref() == Some(change_id)
                        }) {
                            state.cursor = idx;
                            state.reset_detail();
                        }
                    }
                    // modal already taken (dismissed)
                }
                Some(Modal::FuzzyFind { matches, cursor, .. }) => {
                    if let Some(&idx) = matches.get(cursor) {
                        state.cursor = idx;
                        state.reset_detail();
                    }
                    // modal already taken (dismissed)
                }
                other => {
                    // For OpLog/Help, Enter is a no-op — put the modal back
                    state.modal = other;
                }
            }
        }
        Action::FuzzyInput(c) => {
            if let Some(Modal::FuzzyFind { query, matches, cursor }) = &mut state.modal {
                query.push(c);
                *matches = fuzzy_match(query, &state.graph);
                *cursor = 0;
            }
        }
        Action::FuzzyBackspace => {
            if let Some(Modal::FuzzyFind { query, matches, cursor }) = &mut state.modal {
                query.pop();
                *matches = fuzzy_match(query, &state.graph);
                *cursor = 0;
            }
        }
```

Add the `fuzzy_match` helper function (see Task 8 for the full implementation with nucleo). For Task 5, use a placeholder:

```rust
fn fuzzy_match(_query: &str, graph: &GraphData) -> Vec<usize> {
    // Placeholder — replaced with nucleo in Task 8
    graph.node_indices().to_vec()
}
```

- [ ] **Step 4: Add map_modal_event() to input.rs**

```rust
pub fn map_modal_event(event: KeyEvent, modal: &crate::app::Modal) -> Option<Action> {
    use crate::app::Modal;

    // Common keys for ALL modals (early returns)
    match event.code {
        KeyCode::Esc => return Some(Action::ModalDismiss),
        KeyCode::Enter => return Some(Action::ModalEnter),
        KeyCode::Up => return Some(Action::ModalMoveUp),
        KeyCode::Down => return Some(Action::ModalMoveDown),
        _ => {}
    }

    let is_fuzzy = matches!(modal, Modal::FuzzyFind { .. });

    if is_fuzzy {
        // Text input mode — j/k/q are text, not navigation
        match event.code {
            KeyCode::Backspace => Some(Action::FuzzyBackspace),
            KeyCode::Char('n') if event.modifiers == KeyModifiers::CONTROL => {
                Some(Action::ModalMoveDown)
            }
            KeyCode::Char('p') if event.modifiers == KeyModifiers::CONTROL => {
                Some(Action::ModalMoveUp)
            }
            KeyCode::Char(c)
                if event.modifiers == KeyModifiers::NONE
                    || event.modifiers == KeyModifiers::SHIFT =>
            {
                Some(Action::FuzzyInput(c))
            }
            _ => None,
        }
    } else {
        // Non-text modal — j/k for navigation, q to dismiss, trigger key to toggle
        match (event.code, event.modifiers) {
            (KeyCode::Char('q'), KeyModifiers::NONE) => Some(Action::ModalDismiss),
            (KeyCode::Char('j'), KeyModifiers::NONE) => Some(Action::ModalMoveDown),
            (KeyCode::Char('k'), KeyModifiers::NONE) => Some(Action::ModalMoveUp),
            (KeyCode::Char('O'), _) if matches!(modal, Modal::OpLog { .. }) => {
                Some(Action::ModalDismiss)
            }
            (KeyCode::Char('?'), _) if matches!(modal, Modal::Help { .. }) => {
                Some(Action::ModalDismiss)
            }
            _ => None,
        }
    }
}
```

- [ ] **Step 5: Add modal trigger keys to map_event()**

In `map_event()`, add to the global keys section (when no modal is active — the caller checks this):

```rust
(KeyCode::Char('O'), _) => return Some(Action::ToggleOpLog),
(KeyCode::Char('b'), KeyModifiers::NONE) => return Some(Action::OpenBookmarks),
(KeyCode::Char('/'), _) => return Some(Action::OpenFuzzyFind),
(KeyCode::Char('?'), _) => return Some(Action::OpenHelp),
```

- [ ] **Step 6: Update main.rs for modal-aware input routing**

In `run_loop()`:

```rust
if let Some(action) = if state.modal.is_some() {
    map_modal_event(key_event, state.modal.as_ref().unwrap())
} else {
    map_event(key_event, state.focus, state.detail_mode)
} {
    dispatch(state, action, backend);
}
```

And add import for `map_modal_event`.

- [ ] **Step 7: Update render.rs for modal overlay**

When `state.modal.is_some()`, render the normal panels with a dim modifier, then render the modal overlay on top. For now, just add the dimming and a placeholder — individual modal widgets come in later tasks.

```rust
pub fn render(frame: &mut Frame, state: &AppState) {
    let outer = Layout::vertical([Constraint::Min(1), Constraint::Length(STATUS_BAR_HEIGHT)])
        .split(frame.area());

    let main =
        Layout::horizontal([Constraint::Ratio(1, 3), Constraint::Ratio(2, 3)]).split(outer[0]);

    panels::graph::render(frame, state, main[0]);
    panels::detail::render(frame, state, main[1]);

    // Status bar
    let change_id = state.selected_change_id();
    let detail = state.selected_detail();
    let error = state.error.as_deref();
    let status_widget = StatusBarWidget::new(change_id, detail, error);
    frame.render_widget(status_widget, outer[1]);

    // Modal overlay (rendered last, on top)
    if let Some(ref modal) = state.modal {
        // Dim background panels
        let dim = Style::default().add_modifier(Modifier::DIM);
        for y in outer[0].y..outer[0].y + outer[0].height {
            for x in outer[0].x..outer[0].x + outer[0].width {
                frame.buffer_mut()[(x, y)].set_style(dim);
            }
        }
        render_modal(frame, modal, outer[0]);
    }
}

fn render_modal(frame: &mut Frame, modal: &Modal, area: Rect) {
    // TODO: individual modal widgets added in tasks 6-9
    // For now, just render a centered block
    let modal_area = centered_rect(60, 80, area);
    frame.render_widget(ratatui::widgets::Clear, modal_area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .title("Modal");
    frame.render_widget(block, modal_area);
}

/// Create a centered rect within `area` using percentage width and height.
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
```

- [ ] **Step 8: Write tests for modal system**

In `app.rs` tests:

```rust
    #[test]
    fn toggle_op_log_opens_and_closes() {
        let mock = mock();
        let mut state = AppState::new(sample_graph());
        assert!(state.modal.is_none());

        dispatch(&mut state, Action::ToggleOpLog, &mock);
        assert!(matches!(state.modal, Some(Modal::OpLog { .. })));

        dispatch(&mut state, Action::ModalDismiss, &mock);
        assert!(state.modal.is_none());
    }

    #[test]
    fn q_in_modal_dismisses_not_quits() {
        let mock = mock();
        let mut state = AppState::new(sample_graph());
        dispatch(&mut state, Action::ToggleOpLog, &mock);

        dispatch(&mut state, Action::ModalDismiss, &mock);
        assert!(state.modal.is_none());
        assert!(!state.should_quit);
    }

    #[test]
    fn open_help_captures_context() {
        let mock = mock();
        let mut state = AppState::new(sample_graph());
        state.focus = PanelFocus::Detail;
        state.detail_mode = DetailMode::DiffView;

        dispatch(&mut state, Action::OpenHelp, &mock);
        if let Some(Modal::Help { context, .. }) = &state.modal {
            assert_eq!(*context, HelpContext::DetailDiffView);
        } else {
            panic!("Expected Help modal");
        }
    }
```

In `input.rs` tests:

```rust
    #[test]
    fn modal_trigger_keys() {
        assert_eq!(map_graph(key(KeyCode::Char('O'))), Some(Action::ToggleOpLog));
        assert_eq!(map_graph(key(KeyCode::Char('b'))), Some(Action::OpenBookmarks));
        assert_eq!(map_graph(key(KeyCode::Char('/'))), Some(Action::OpenFuzzyFind));
        assert_eq!(map_graph(key(KeyCode::Char('?'))), Some(Action::OpenHelp));
    }
```

- [ ] **Step 9: Run all tests**

Run: `cargo test`

- [ ] **Step 10: Commit**

```bash
git commit -m "feat(tui): add modal system foundation with dispatch, input routing, and overlay rendering"
```

---

### Task 6: Op Log Widget

**Files:**
- Create: `crates/lajjzy-tui/src/widgets/op_log.rs`
- Modify: `crates/lajjzy-tui/src/widgets/mod.rs`
- Modify: `crates/lajjzy-tui/src/render.rs`

- [ ] **Step 1: Create OpLogWidget**

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};

use lajjzy_core::types::OpLogEntry;

pub struct OpLogWidget<'a> {
    entries: &'a [OpLogEntry],
    cursor: usize,
    scroll: usize,
}

impl<'a> OpLogWidget<'a> {
    pub fn new(entries: &'a [OpLogEntry], cursor: usize, scroll: usize) -> Self {
        Self { entries, cursor, scroll }
    }
}

impl Widget for OpLogWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title("Operation Log");
        let inner = block.inner(area);
        block.render(area, buf);

        if self.entries.is_empty() {
            let msg = Line::styled("(no operations)", Style::default().fg(Color::DarkGray));
            buf.set_line(inner.x, inner.y, &msg, inner.width);
            return;
        }

        let highlight = Style::default().add_modifier(Modifier::REVERSED);
        let height = inner.height as usize;

        for (row, idx) in (self.scroll..self.scroll + height).enumerate() {
            if idx >= self.entries.len() {
                break;
            }
            let entry = &self.entries[idx];
            let spans = vec![
                Span::styled(&entry.id, Style::default().fg(Color::Yellow)),
                Span::raw("  "),
                Span::styled(&entry.timestamp, Style::default().fg(Color::Cyan)),
                Span::raw("  "),
                Span::raw(&entry.description),
            ];
            let line = Line::from(spans);
            let y = inner.y + row as u16;
            buf.set_line(inner.x, y, &line, inner.width);

            if idx == self.cursor {
                for x in inner.x..inner.x + inner.width {
                    buf[(x, y)].set_style(highlight);
                }
            }
        }
    }
}
```

- [ ] **Step 2: Update widgets/mod.rs**

Add `pub mod op_log;`

- [ ] **Step 3: Wire into render.rs**

Update `render_modal()` to render the op log widget when `Modal::OpLog`:

```rust
fn render_modal(frame: &mut Frame, modal: &Modal, area: Rect) {
    match modal {
        Modal::OpLog { entries, cursor, scroll } => {
            frame.render_widget(ratatui::widgets::Clear, area);
            let widget = crate::widgets::op_log::OpLogWidget::new(entries, *cursor, *scroll);
            frame.render_widget(widget, area);
        }
        _ => {
            // Other modals handled in later tasks
            let modal_area = centered_rect(60, 80, area);
            frame.render_widget(ratatui::widgets::Clear, modal_area);
            let block = Block::default().borders(Borders::ALL).title("Modal");
            frame.render_widget(block, modal_area);
        }
    }
}
```

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(tui): add OpLogWidget for operation log overlay"
```

---

### Task 7: Bookmark Picker Widget

**Files:**
- Create: `crates/lajjzy-tui/src/widgets/bookmark_picker.rs`
- Modify: `crates/lajjzy-tui/src/widgets/mod.rs`
- Modify: `crates/lajjzy-tui/src/render.rs`
- Modify: `crates/lajjzy-tui/src/app.rs` (bookmark collection + ModalEnter jump)

- [ ] **Step 1: Create BookmarkPickerWidget**

`crates/lajjzy-tui/src/widgets/bookmark_picker.rs`:

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};

pub struct BookmarkPickerWidget<'a> {
    bookmarks: &'a [(String, String)],  // (name, change_id)
    descriptions: &'a std::collections::HashMap<String, lajjzy_core::types::ChangeDetail>,
    cursor: usize,
}

impl<'a> BookmarkPickerWidget<'a> {
    pub fn new(
        bookmarks: &'a [(String, String)],
        descriptions: &'a std::collections::HashMap<String, lajjzy_core::types::ChangeDetail>,
        cursor: usize,
    ) -> Self {
        Self { bookmarks, descriptions, cursor }
    }
}

impl Widget for BookmarkPickerWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title("Bookmarks");
        let inner = block.inner(area);
        block.render(area, buf);

        if self.bookmarks.is_empty() {
            let msg = Line::styled("(no bookmarks)", Style::default().fg(Color::DarkGray));
            buf.set_line(inner.x, inner.y, &msg, inner.width);
            return;
        }

        let highlight = Style::default().add_modifier(Modifier::REVERSED);
        let height = inner.height as usize;

        for (row, idx) in (0..height).enumerate() {
            if idx >= self.bookmarks.len() {
                break;
            }
            let (name, cid) = &self.bookmarks[idx];
            let desc = self.descriptions.get(cid)
                .map(|d| d.description.as_str())
                .unwrap_or("");
            let spans = vec![
                Span::styled(name, Style::default().fg(Color::Magenta)),
                Span::raw("  "),
                Span::styled(desc, Style::default().fg(Color::DarkGray)),
            ];
            let line = Line::from(spans);
            let y = inner.y + row as u16;
            buf.set_line(inner.x, y, &line, inner.width);

            if idx == self.cursor {
                for x in inner.x..inner.x + inner.width {
                    buf[(x, y)].set_style(highlight);
                }
            }
        }
    }
}
```

- [ ] **Step 2: Implement bookmark collection in dispatch**

In the `OpenBookmarks` handler:

```rust
Action::OpenBookmarks => {
    let mut bookmarks = Vec::new();
    for &idx in state.graph.node_indices() {
        if let Some(cid) = state.graph.lines[idx].change_id.as_ref() {
            if let Some(detail) = state.graph.details.get(cid) {
                for bm in &detail.bookmarks {
                    bookmarks.push((bm.clone(), cid.clone()));
                }
            }
        }
    }
    state.modal = Some(Modal::BookmarkPicker { bookmarks, cursor: 0 });
}
```

- [ ] **Step 3: Implement ModalEnter for BookmarkPicker**

In `ModalEnter` handler, when modal is `BookmarkPicker`:
- Get the selected `(bookmark_name, change_id)` at `cursor`
- Find the change_id's position in `node_indices`
- Set `state.cursor` to that position, call `reset_detail()`
- Set `state.modal = None`

- [ ] **Step 4: Wire widget into render.rs**

- [ ] **Step 5: Write tests**

```rust
    #[test]
    fn bookmark_picker_collects_from_graph() {
        // Use a sample_graph where at least one change has bookmarks
        // ...
    }

    #[test]
    fn bookmark_picker_enter_jumps_cursor() {
        // Open picker, dispatch ModalEnter, verify cursor moved
    }
```

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(tui): add bookmark picker with cursor jump on select"
```

---

### Task 8: Fuzzy-Find Widget

**Files:**
- Modify: `crates/lajjzy-tui/Cargo.toml` (add `nucleo-matcher`)
- Create: `crates/lajjzy-tui/src/widgets/fuzzy_find.rs`
- Modify: `crates/lajjzy-tui/src/widgets/mod.rs`
- Modify: `crates/lajjzy-tui/src/render.rs`
- Modify: `crates/lajjzy-tui/src/app.rs` (fuzzy match logic)

- [ ] **Step 1: Add nucleo-matcher dependency**

```toml
nucleo-matcher = "0.3"
```

- [ ] **Step 2: Create FuzzyFindWidget**

Shows a text input line at the top (`/ query|`) and a results list below. Each result: `{change_id}  {author}  {description}`.

- [ ] **Step 3: Implement fuzzy match logic in dispatch**

For `OpenFuzzyFind`: initialize with all node indices as matches.

For `FuzzyInput(c)` and `FuzzyBackspace`: update query, re-run match using `nucleo_matcher::Matcher` and `Pattern`. Collect matching node indices sorted by score.

Key nucleo usage:

```rust
use nucleo_matcher::{Matcher, Config};
use nucleo_matcher::pattern::{Pattern, AtomKind, CaseMatching, Normalization};

fn fuzzy_match(query: &str, graph: &GraphData) -> Vec<usize> {
    if query.is_empty() {
        return graph.node_indices().to_vec();
    }
    let mut matcher = Matcher::new(Config::DEFAULT);
    // Use Pattern::new (not ::parse) to avoid interpreting ^ $ ! as special chars.
    // AtomKind::Fuzzy gives the expected fuzzy matching behavior.
    let pattern = Pattern::new(query, CaseMatching::Smart, Normalization::Smart, AtomKind::Fuzzy);

    let mut scored: Vec<(usize, u32)> = Vec::new();
    for &idx in graph.node_indices() {
        if let Some(cid) = graph.lines[idx].change_id.as_ref() {
            if let Some(detail) = graph.details.get(cid) {
                let haystack = format!("{cid} {author} {desc}",
                    author = detail.author, desc = detail.description);
                let mut buf: Vec<char> = Vec::new();
                if let Some(score) = pattern.score(
                    nucleo_matcher::Utf32Str::new(&haystack, &mut buf),
                    &mut matcher,
                ) {
                    scored.push((idx, score));
                }
            }
        }
    }
    scored.sort_by(|a, b| b.1.cmp(&a.1)); // highest score first
    scored.into_iter().map(|(idx, _)| idx).collect()
}
```

**Note:** The nucleo API may differ slightly across versions. The implementer should check the actual `nucleo-matcher` 0.3 API (use Context7 MCP if needed) and adapt. Key points: use `Pattern::new` with `AtomKind::Fuzzy` (not `Pattern::parse`) to avoid interpreting user input as regex-like patterns.

- [ ] **Step 4: Implement ModalEnter for FuzzyFind**

Same as BookmarkPicker: jump cursor to selected match, dismiss modal.

- [ ] **Step 5: Wire widget into render.rs**

- [ ] **Step 6: Write tests**

```rust
    #[test]
    fn fuzzy_find_empty_query_shows_all() {
        // Open fuzzy-find, verify matches contains all node indices
    }

    #[test]
    fn fuzzy_find_typing_filters() {
        // Open fuzzy-find, dispatch FuzzyInput('d'), verify matches filtered
    }

    #[test]
    fn fuzzy_find_enter_jumps_cursor() {
        // Open fuzzy-find, dispatch ModalEnter, verify cursor moved
    }

    #[test]
    fn fuzzy_find_backspace_removes_from_query() {
        // Type some chars, backspace, verify query shortened
    }
```

- [ ] **Step 7: Commit**

```bash
git commit -m "feat(tui): add fuzzy-find overlay with nucleo-matcher"
```

---

### Task 9: Help Widget

**Files:**
- Create: `crates/lajjzy-tui/src/widgets/help.rs`
- Modify: `crates/lajjzy-tui/src/widgets/mod.rs`
- Modify: `crates/lajjzy-tui/src/render.rs`

- [ ] **Step 1: Create HelpWidget**

Static content based on `HelpContext`. Two-column layout: key left, description right.

```rust
use crate::app::HelpContext;

pub struct HelpWidget {
    context: HelpContext,
    scroll: usize,
}

fn help_lines(context: HelpContext) -> Vec<(&'static str, &'static str)> {
    match context {
        HelpContext::Graph => vec![
            ("j/k", "Move between changes"),
            ("g/G", "Jump to top/bottom"),
            ("@", "Jump to working copy"),
            ("Tab", "Switch to detail pane"),
            ("R", "Refresh"),
            ("/", "Fuzzy-find"),
            ("b", "Bookmarks"),
            ("O", "Operation log"),
            ("?", "This help"),
            ("q", "Quit"),
        ],
        HelpContext::DetailFileList => vec![
            ("j/k", "Move between files"),
            ("Enter", "Open diff view"),
            ("Esc", "Return to graph"),
            ("Tab", "Switch to graph pane"),
        ],
        HelpContext::DetailDiffView => vec![
            ("j/k", "Scroll diff"),
            ("n/N", "Next/previous hunk"),
            ("Esc", "Return to file list"),
        ],
    }
}
```

- [ ] **Step 2: Wire into render.rs**

Centered overlay (~50% x ~60%).

- [ ] **Step 3: Write test for context-sensitive content**

```rust
    #[test]
    fn help_context_from_state() {
        // Verify Graph focus → HelpContext::Graph
        // Verify Detail + FileList → HelpContext::DetailFileList
        // Verify Detail + DiffView → HelpContext::DetailDiffView
    }
```

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(tui): add context-sensitive help overlay"
```

---

### Task 10: Final Integration and Cleanup

**Files:**
- Various (clippy, fmt, CLAUDE.md)

- [ ] **Step 1: Run full test suite**

Run: `cargo test`

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Fix any warnings.

- [ ] **Step 3: Run formatter**

Run: `cargo fmt`

- [ ] **Step 4: Update CLAUDE.md**

Add M1b features and modal keybindings. Note `op_log()` as another dispatch impurity.

- [ ] **Step 5: Manual smoke test**

In a jj repo:
```bash
cargo run -p lajjzy
```

Verify:
- [ ] Graph is compact (no file lines) and colorized
- [ ] Working copy change shows green `@`
- [ ] Bookmarks show in magenta
- [ ] `O` opens op log overlay, `j`/`k` navigates, `Esc` closes
- [ ] `b` opens bookmark picker, `Enter` jumps to change
- [ ] `/` opens fuzzy-find, typing filters, `Enter` selects
- [ ] `?` opens help showing context-appropriate keybindings
- [ ] `q` in any modal closes the modal, not the app
- [ ] `q` with no modal quits

- [ ] **Step 6: Commit**

```bash
git commit -m "chore: M1b integration cleanup and CLAUDE.md update"
```
