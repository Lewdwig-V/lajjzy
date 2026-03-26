# M6 — Forge Integration (GitHub) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show GitHub PR status in the graph and provide keybindings to open/create PRs, with graceful degradation when `gh` is not installed.

**Architecture:** A separate `ForgeBackend` trait (not on `RepoBackend`) handles `gh` CLI interaction. Types in `lajjzy-core/src/forge.rs`, implementation in `lajjzy-core/src/gh.rs`. The executor in `lajjzy-cli` holds both `RepoBackend` and `ForgeBackend`. The TUI imports forge types only, never the backend traits.

**Tech Stack:** Rust, `gh` CLI (JSON output), serde_json (for parsing `gh pr list` output)

**Spec:** `docs/superpowers/specs/2026-03-22-m6-forge-integration-design.md`

---

## File Map

### New files
- `crates/lajjzy-core/src/forge.rs` — `ForgeKind`, `PrInfo`, `PrState`, `ReviewStatus` types + `ForgeBackend` trait
- `crates/lajjzy-core/src/gh.rs` — `GhCliForge` implementation of `ForgeBackend`

### Modified files
- `crates/lajjzy-core/Cargo.toml` — Add `serde`, `serde_json` dependencies
- `crates/lajjzy-core/src/lib.rs` — Export `forge` and `gh` modules
- `crates/lajjzy-tui/src/action.rs` — New actions: `FetchForgeStatus`, `OpenOrCreatePr`, `ForgeStatusLoaded`, `PrViewUrl`, `PrCreateComplete`, `PrCreateFailed`
- `crates/lajjzy-tui/src/effect.rs` — New effects: `FetchForgeStatus`, `OpenPrInBrowser`, `CreatePr`
- `crates/lajjzy-tui/src/app.rs` — `forge`, `pr_status`, `pending_forge_fetch` fields; update `AppState::new` signature
- `crates/lajjzy-tui/src/dispatch.rs` — Dispatch handlers for all forge actions
- `crates/lajjzy-tui/src/input.rs` — `F` and `W` key mappings in graph context
- `crates/lajjzy-tui/src/widgets/graph.rs` — PR indicator rendering after bookmarks
- `crates/lajjzy-tui/src/widgets/help.rs` — Conditional forge help lines
- `crates/lajjzy-cli/src/main.rs` — Create `GhCliForge`, pass to `AppState`, executor arms for forge effects, `execute_effects` for `CreatePr` suspend

---

## Task 1: Forge Types and Backend Trait

**Files:**
- Create: `crates/lajjzy-core/src/forge.rs`
- Modify: `crates/lajjzy-core/src/lib.rs`

- [ ] **Step 1: Create `forge.rs` with types and trait**

Create `crates/lajjzy-core/src/forge.rs`:

```rust
use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForgeKind {
    GitHub,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrInfo {
    pub number: u32,
    pub title: String,
    pub state: PrState,
    pub review: ReviewStatus,
    pub head_ref: String,
    pub url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrState {
    Open,
    Merged,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewStatus {
    Approved,
    ChangesRequested,
    ReviewRequired,
    Unknown,
}

/// Abstraction over forge (GitHub/GitLab/Gerrit) access.
/// Separate from RepoBackend — forge operations use different CLI tools.
pub trait ForgeBackend: Send + Sync {
    /// Which forge CLI is available, if any.
    fn forge_kind(&self) -> Option<ForgeKind>;

    /// Fetch PR/MR status from the forge.
    /// Returns Ok(None) when no forge CLI is available.
    fn fetch_status(&self) -> Result<Option<Vec<PrInfo>>>;
}
```

- [ ] **Step 2: Export module from lib.rs**

In `crates/lajjzy-core/src/lib.rs` (create if it doesn't exist, or find the existing module declarations):

```rust
pub mod forge;
```

- [ ] **Step 3: Verify and commit**

Run: `cargo build --workspace`
Expected: Compiles (types defined, trait has no implementor yet).

```bash
git add -A
git commit -m "feat(core): add forge types and ForgeBackend trait

ForgeKind, PrInfo, PrState, ReviewStatus types.
Separate trait from RepoBackend — forge uses gh, not jj."
```

---

## Task 2: GhCliForge Implementation

**Files:**
- Create: `crates/lajjzy-core/src/gh.rs`
- Modify: `crates/lajjzy-core/Cargo.toml`
- Modify: `crates/lajjzy-core/src/lib.rs`

- [ ] **Step 1: Add serde_json dependency**

```bash
cargo add serde_json -p lajjzy-core
cargo add serde --features derive -p lajjzy-core
```

- [ ] **Step 2: Create `gh.rs` with GhCliForge**

Create `crates/lajjzy-core/src/gh.rs`:

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::forge::{ForgeBackend, ForgeKind, PrInfo, PrState, ReviewStatus};

pub struct GhCliForge {
    workspace_root: PathBuf,
    available: bool,
}

impl GhCliForge {
    pub fn new(workspace_root: &Path) -> Self {
        let available = Command::new("gh")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        Self {
            workspace_root: workspace_root.to_path_buf(),
            available,
        }
    }
}

impl ForgeBackend for GhCliForge {
    fn forge_kind(&self) -> Option<ForgeKind> {
        if self.available {
            Some(ForgeKind::GitHub)
        } else {
            None
        }
    }

    fn fetch_status(&self) -> Result<Option<Vec<PrInfo>>> {
        if !self.available {
            return Ok(None);
        }

        let output = Command::new("gh")
            .args([
                "pr", "list",
                "--state", "open",
                "--limit", "100",
                "--json", "number,title,state,headRefName,reviewDecision,url",
            ])
            .current_dir(&self.workspace_root)
            .output()
            .context("Failed to run gh pr list")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("gh pr list failed: {}", stderr.trim());
        }

        let stdout = String::from_utf8(output.stdout)
            .context("gh output was not valid UTF-8")?;

        let prs = parse_gh_pr_list(&stdout)?;
        Ok(Some(prs))
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPrJson {
    number: u32,
    title: String,
    state: String,
    head_ref_name: String,
    review_decision: String,
    #[serde(default)]
    url: String,
}

fn parse_gh_pr_list(json: &str) -> Result<Vec<PrInfo>> {
    let raw: Vec<GhPrJson> = serde_json::from_str(json)
        .context("Failed to parse gh pr list JSON")?;

    Ok(raw
        .into_iter()
        .map(|pr| PrInfo {
            number: pr.number,
            title: pr.title,
            state: match pr.state.as_str() {
                "OPEN" => PrState::Open,
                "MERGED" => PrState::Merged,
                "CLOSED" => PrState::Closed,
                _ => PrState::Open,
            },
            review: match pr.review_decision.as_str() {
                "APPROVED" => ReviewStatus::Approved,
                "CHANGES_REQUESTED" => ReviewStatus::ChangesRequested,
                "REVIEW_REQUIRED" => ReviewStatus::ReviewRequired,
                _ => ReviewStatus::Unknown,
            },
            head_ref: pr.head_ref_name,
            url: pr.url,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_open_pr_approved() {
        let json = r#"[{
            "number": 42,
            "title": "feat: add feature",
            "state": "OPEN",
            "headRefName": "feature-x",
            "reviewDecision": "APPROVED",
            "url": "https://github.com/owner/repo/pull/42"
        }]"#;
        let prs = parse_gh_pr_list(json).unwrap();
        assert_eq!(prs.len(), 1);
        assert_eq!(prs[0].number, 42);
        assert_eq!(prs[0].state, PrState::Open);
        assert_eq!(prs[0].review, ReviewStatus::Approved);
        assert_eq!(prs[0].head_ref, "feature-x");
    }

    #[test]
    fn parse_changes_requested() {
        let json = r#"[{
            "number": 15,
            "title": "fix: bug",
            "state": "OPEN",
            "headRefName": "fix-bug",
            "reviewDecision": "CHANGES_REQUESTED",
            "url": "https://github.com/owner/repo/pull/15"
        }]"#;
        let prs = parse_gh_pr_list(json).unwrap();
        assert_eq!(prs[0].review, ReviewStatus::ChangesRequested);
    }

    #[test]
    fn parse_empty_review_decision() {
        let json = r#"[{
            "number": 1,
            "title": "test",
            "state": "OPEN",
            "headRefName": "test",
            "reviewDecision": "",
            "url": ""
        }]"#;
        let prs = parse_gh_pr_list(json).unwrap();
        assert_eq!(prs[0].review, ReviewStatus::Unknown);
    }

    #[test]
    fn parse_empty_list() {
        let prs = parse_gh_pr_list("[]").unwrap();
        assert!(prs.is_empty());
    }

    #[test]
    fn parse_merged_pr() {
        let json = r#"[{
            "number": 10,
            "title": "merged",
            "state": "MERGED",
            "headRefName": "old",
            "reviewDecision": "APPROVED",
            "url": "https://github.com/owner/repo/pull/10"
        }]"#;
        let prs = parse_gh_pr_list(json).unwrap();
        assert_eq!(prs[0].state, PrState::Merged);
    }

    #[test]
    fn parse_malformed_json_returns_error() {
        assert!(parse_gh_pr_list("not json").is_err());
    }
}
```

- [ ] **Step 3: Export module**

Add to `crates/lajjzy-core/src/lib.rs`:

```rust
pub mod gh;
```

- [ ] **Step 4: Verify and commit**

Run: `cargo test --workspace`
Expected: All existing tests + 6 new JSON parsing tests pass.

```bash
git add -A
git commit -m "feat(core): implement GhCliForge with gh pr list parsing

Checks gh availability at construction time.
Parses gh pr list JSON into PrInfo with state/review mapping.
6 JSON parsing tests."
```

---

## Task 3: Actions, Effects, AppState

**Files:**
- Modify: `crates/lajjzy-tui/src/action.rs`
- Modify: `crates/lajjzy-tui/src/effect.rs`
- Modify: `crates/lajjzy-tui/src/app.rs`

- [ ] **Step 1: Add forge action variants**

In `crates/lajjzy-tui/src/action.rs`, add import and variants:

```rust
use lajjzy_core::forge::PrInfo;
```

Add to `Action` enum:

```rust
// Forge actions
FetchForgeStatus,
OpenOrCreatePr,
ForgeStatusLoaded(Result<Option<Vec<PrInfo>>, String>),
PrViewUrl { url: String },
PrCreateComplete,
PrCreateFailed { error: String },
```

- [ ] **Step 2: Add forge effect variants**

In `crates/lajjzy-tui/src/effect.rs`:

```rust
// Forge effects
FetchForgeStatus,
OpenPrInBrowser { bookmark: String, url: String },
CreatePr { bookmark: String },
```

- [ ] **Step 3: Update AppState**

In `crates/lajjzy-tui/src/app.rs`, add imports:

```rust
use std::collections::HashMap;
use lajjzy_core::forge::{ForgeKind, PrInfo};
```

Add fields to `AppState`:

```rust
pub forge: Option<ForgeKind>,
pub pr_status: HashMap<String, PrInfo>,
pub pending_forge_fetch: bool,
```

Update `AppState::new` to accept and store `forge`:

```rust
pub fn new(graph: GraphData, forge: Option<ForgeKind>) -> Self {
    // ... existing init ...
    Self {
        // ... existing fields ...
        forge,
        pr_status: HashMap::new(),
        pending_forge_fetch: false,
    }
}
```

- [ ] **Step 4: Fix all `AppState::new` call sites**

In `crates/lajjzy-cli/src/main.rs`, update:
```rust
let mut state = AppState::new(graph, None); // temporary — Task 6 wires up the real forge
```

In test files, update all `AppState::new(graph)` calls to `AppState::new(graph, None)`.

- [ ] **Step 5: Add stub dispatch arms and executor stubs**

Add a combined stub in dispatch so it compiles:
```rust
Action::FetchForgeStatus
| Action::OpenOrCreatePr
| Action::ForgeStatusLoaded(_)
| Action::PrViewUrl { .. }
| Action::PrCreateComplete
| Action::PrCreateFailed { .. } => {}
```

Add executor stubs for new effects in `main.rs` (similar pattern to previous milestones).

Add new effects to `next_graph_generation` zero-returning arm (forge effects don't load graphs).

- [ ] **Step 6: Verify and commit**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: All pass.

```bash
git add -A
git commit -m "feat(tui): add forge actions, effects, and AppState fields

FetchForgeStatus, OpenOrCreatePr, ForgeStatusLoaded, PrViewUrl,
PrCreateComplete, PrCreateFailed. AppState gains forge, pr_status,
pending_forge_fetch. Stub dispatch and executor arms."
```

---

## Task 4: Dispatch Logic

**Files:**
- Modify: `crates/lajjzy-tui/src/dispatch.rs`

- [ ] **Step 1: Replace forge stub with real dispatch handlers**

```rust
Action::FetchForgeStatus => {
    if state.forge.is_none() {
        state.error = Some("Install gh for GitHub integration".into());
        return vec![];
    }
    if state.pending_forge_fetch {
        return vec![];
    }
    state.pending_forge_fetch = true;
    return vec![Effect::FetchForgeStatus];
}
Action::OpenOrCreatePr => {
    if state.forge.is_none() {
        state.error = Some("Install gh for GitHub integration".into());
        return vec![];
    }
    let bookmarks = state.selected_detail()
        .map(|d| &d.bookmarks)
        .cloned()
        .unwrap_or_default();
    if bookmarks.is_empty() {
        state.error = Some("No bookmark on this change \u{2014} set one with B first".into());
        return vec![];
    }
    let bookmark = bookmarks[0].clone();
    if let Some(pr) = state.pr_status.get(&bookmark) {
        return vec![Effect::OpenPrInBrowser {
            bookmark,
            url: pr.url.clone(),
        }];
    }
    return vec![Effect::CreatePr { bookmark }];
}
Action::ForgeStatusLoaded(result) => {
    state.pending_forge_fetch = false;
    match result {
        Ok(Some(prs)) => {
            let count = prs.len();
            state.pr_status.clear();
            for pr in prs {
                state.pr_status.insert(pr.head_ref.clone(), pr);
            }
            state.status_message = Some(if count > 0 {
                format!("Loaded {count} PRs")
            } else {
                "No PRs found".into()
            });
        }
        Ok(None) => {}
        Err(e) => {
            state.error = Some(e);
        }
    }
}
Action::PrViewUrl { url } => {
    state.status_message = Some(url);
}
Action::PrCreateComplete => {
    return vec![Effect::FetchForgeStatus];
}
Action::PrCreateFailed { error } => {
    state.error = Some(error);
}
```

- [ ] **Step 2: Write dispatch tests**

```rust
#[test]
fn fetch_forge_status_emits_effect_when_available() {
    let mut state = AppState::new(sample_graph(), Some(ForgeKind::GitHub));
    let effects = dispatch(&mut state, Action::FetchForgeStatus);
    assert_eq!(effects.len(), 1);
    assert!(matches!(effects[0], Effect::FetchForgeStatus));
    assert!(state.pending_forge_fetch);
}

#[test]
fn fetch_forge_status_errors_when_unavailable() {
    let mut state = AppState::new(sample_graph(), None);
    let effects = dispatch(&mut state, Action::FetchForgeStatus);
    assert!(effects.is_empty());
    assert!(state.error.is_some());
}

#[test]
fn fetch_forge_status_debounces() {
    let mut state = AppState::new(sample_graph(), Some(ForgeKind::GitHub));
    state.pending_forge_fetch = true;
    let effects = dispatch(&mut state, Action::FetchForgeStatus);
    assert!(effects.is_empty());
}

#[test]
fn forge_status_loaded_populates_pr_status() {
    let mut state = AppState::new(sample_graph(), Some(ForgeKind::GitHub));
    state.pending_forge_fetch = true;
    let prs = vec![PrInfo {
        number: 42,
        title: "test".into(),
        state: PrState::Open,
        review: ReviewStatus::Approved,
        head_ref: "main".into(),
        url: "https://example.com/42".into(),
    }];
    dispatch(&mut state, Action::ForgeStatusLoaded(Ok(Some(prs))));
    assert!(!state.pending_forge_fetch);
    assert_eq!(state.pr_status.len(), 1);
    assert!(state.status_message.as_ref().unwrap().contains("1 PR"));
}

#[test]
fn forge_status_loaded_clears_pending_on_error() {
    let mut state = AppState::new(sample_graph(), Some(ForgeKind::GitHub));
    state.pending_forge_fetch = true;
    dispatch(&mut state, Action::ForgeStatusLoaded(Err("auth failed".into())));
    assert!(!state.pending_forge_fetch);
    assert!(state.error.is_some());
}

#[test]
fn open_or_create_pr_no_forge() {
    let mut state = AppState::new(sample_graph(), None);
    let effects = dispatch(&mut state, Action::OpenOrCreatePr);
    assert!(effects.is_empty());
    assert!(state.error.as_ref().unwrap().contains("gh"));
}

#[test]
fn open_or_create_pr_no_bookmark() {
    // sample_graph's first change has bookmarks: ["main"]
    // Use a change without bookmarks
    let mut state = AppState::new(sample_graph(), Some(ForgeKind::GitHub));
    // Move cursor to a change without bookmarks
    state.set_cursor_for_test(2); // "def456" has no bookmarks
    let effects = dispatch(&mut state, Action::OpenOrCreatePr);
    assert!(effects.is_empty());
    assert!(state.error.as_ref().unwrap().contains("bookmark"));
}

#[test]
fn pr_create_complete_triggers_fetch() {
    let mut state = AppState::new(sample_graph(), Some(ForgeKind::GitHub));
    let effects = dispatch(&mut state, Action::PrCreateComplete);
    assert!(matches!(effects[0], Effect::FetchForgeStatus));
}
```

- [ ] **Step 3: Verify and commit**

Run: `cargo test --workspace`
Expected: All pass.

```bash
git add -A
git commit -m "feat(tui): dispatch handlers for forge status and PR actions"
```

---

## Task 5: Input Handling + Graph Rendering

**Files:**
- Modify: `crates/lajjzy-tui/src/input.rs`
- Modify: `crates/lajjzy-tui/src/widgets/graph.rs`
- Modify: `crates/lajjzy-tui/src/widgets/help.rs`
- Modify: `crates/lajjzy-tui/src/modal.rs`

- [ ] **Step 1: Add key mappings**

In `crates/lajjzy-tui/src/input.rs`, graph panel:

```rust
(KeyCode::Char('F'), _) => Some(Action::FetchForgeStatus),
(KeyCode::Char('W'), _) => Some(Action::OpenOrCreatePr),
```

- [ ] **Step 2: Add PR indicators to graph widget**

In `crates/lajjzy-tui/src/widgets/graph.rs`, the graph widget needs access to `pr_status` to render indicators. Add `pr_status: &HashMap<String, PrInfo>` as a parameter to the widget.

After rendering bookmark names for each change, check if any bookmark matches a key in `pr_status`. For each match, append a colored indicator:

```rust
// For each bookmark on the change:
if let Some(pr) = pr_status.get(bookmark) {
    let (symbol, color) = match (pr.state, pr.review) {
        (PrState::Open, ReviewStatus::Approved) => ("✓", Color::Green),
        (PrState::Open, ReviewStatus::ChangesRequested) => ("✗", Color::Red),
        (PrState::Open, ReviewStatus::ReviewRequired) => ("●", Color::Yellow),
        (PrState::Open, ReviewStatus::Unknown) => ("●", Color::Yellow),
        (PrState::Merged, _) => ("✓", Color::DarkGray),
        (PrState::Closed, _) => ("✗", Color::DarkGray),
    };
    // Append: " #42 ✓approved" with appropriate color
}
```

Update the graph panel render call to pass `&state.pr_status`.

- [ ] **Step 3: Add conditional help text**

In `crates/lajjzy-tui/src/widgets/help.rs`, the `help_lines` function needs access to whether forge is available. Add forge lines conditionally:

```rust
HelpContext::Graph => {
    let mut lines = vec![
        // ... existing lines ...
    ];
    // Add forge lines only when forge is available
    // This requires passing forge availability to the help widget
}
```

The simplest approach: add `forge_available: bool` to `HelpWidget::new()` and conditionally append forge lines. Update `line_count` accordingly.

- [ ] **Step 4: Write input tests**

```rust
#[test]
fn forge_keys_in_graph() {
    assert_eq!(
        map_graph(key_mod(KeyCode::Char('F'), KeyModifiers::SHIFT)),
        Some(Action::FetchForgeStatus)
    );
    assert_eq!(
        map_graph(key_mod(KeyCode::Char('W'), KeyModifiers::SHIFT)),
        Some(Action::OpenOrCreatePr)
    );
}

#[test]
fn forge_keys_not_in_detail() {
    assert_eq!(map_file_list(key(KeyCode::Char('F'))), None);
    assert_eq!(map_file_list(key(KeyCode::Char('W'))), None);
}
```

- [ ] **Step 5: Verify and commit**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: All pass.

```bash
git add -A
git commit -m "feat: graph PR indicators, F/W keybindings, conditional help

PR status rendered after bookmarks with colored symbols.
F fetches forge status, W opens/creates PR.
Help text shown only when gh is available."
```

---

## Task 6: Executor Wiring

**Files:**
- Modify: `crates/lajjzy-cli/src/main.rs`

- [ ] **Step 1: Create GhCliForge and pass to AppState**

In `main()`:

```rust
use lajjzy_core::gh::GhCliForge;
use lajjzy_core::forge::ForgeBackend;

let forge = GhCliForge::new(&cwd);
let forge_kind = forge.forge_kind();
let forge = Arc::new(forge);
// ...
let mut state = AppState::new(graph, forge_kind);
```

Add `forge: Arc<GhCliForge>` to `EffectExecutor`.

- [ ] **Step 2: Wire up FetchForgeStatus**

```rust
Effect::FetchForgeStatus => {
    let forge = Arc::clone(&self.forge);
    let tx = self.tx.clone();
    thread::spawn(move || {
        let result = forge.fetch_status().map_err(|e| e.to_string());
        let _ = tx.send(Action::ForgeStatusLoaded(result));
    });
}
```

- [ ] **Step 3: Wire up OpenPrInBrowser**

```rust
Effect::OpenPrInBrowser { bookmark, url } => {
    let workspace_root = self.backend.workspace_root().to_path_buf();
    let tx = self.tx.clone();
    thread::spawn(move || {
        let _ = std::process::Command::new("gh")
            .args(["pr", "view", &bookmark, "--web"])
            .current_dir(&workspace_root)
            .output();
        let _ = tx.send(Action::PrViewUrl { url });
    });
}
```

- [ ] **Step 4: Intercept CreatePr in execute_effects**

Add to `execute_effects` alongside `SuspendForEditor` and `LaunchMergeTool`:

```rust
Effect::CreatePr { bookmark } => {
    ratatui::restore();
    let status = std::process::Command::new("gh")
        .args(["pr", "create", "--head", &bookmark])
        .current_dir(executor.backend.workspace_root())
        .status();
    *terminal = ratatui::init();
    match status {
        Ok(s) if s.success() => {
            let effects = dispatch(state, Action::PrCreateComplete);
            execute_effects(terminal, state, executor, effects);
        }
        Ok(s) => {
            let effects = dispatch(state, Action::PrCreateFailed {
                error: format!("gh pr create exited with {}", s.code().unwrap_or(-1)),
            });
            execute_effects(terminal, state, executor, effects);
        }
        Err(e) => {
            state.error = Some(format!("Failed to launch gh: {e}"));
        }
    }
}
```

- [ ] **Step 5: Update next_graph_generation**

Add `Effect::FetchForgeStatus`, `Effect::OpenPrInBrowser { .. }`, `Effect::CreatePr { .. }` to the zero-returning arm (none of these load graphs).

- [ ] **Step 6: Verify and commit**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: All pass.

```bash
git add -A
git commit -m "feat: wire up forge effects in executor

FetchForgeStatus on background thread.
OpenPrInBrowser fire-and-forget with URL fallback.
CreatePr with suspend/resume pattern."
```

---

## Task 7: README + Help Polish

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add forge section to README**

Add after the Bookmark Management section:

```markdown
### GitHub Integration

Requires `gh` CLI installed and authenticated (`gh auth login`). Features are hidden when `gh` is not available.

| Key | Action |
|-----|--------|
| `F` | Fetch PR status from GitHub |
| `W` | Open PR in browser (or create if none exists) |

After pressing `F`, PR status indicators appear next to bookmarks in the graph:
- `#42 ✓approved` (green)
- `#42 ●review-required` (yellow)
- `#42 ✗changes-requested` (red)
```

- [ ] **Step 2: Verify and commit**

```bash
git add README.md
git commit -m "docs: add GitHub integration section to README"
```

---

## Summary

| Task | Description | Depends On |
|------|-------------|------------|
| 1 | Forge types and ForgeBackend trait | — |
| 2 | GhCliForge implementation + JSON parsing | 1 |
| 3 | Actions, effects, AppState changes | 1 |
| 4 | Dispatch logic | 3 |
| 5 | Input + graph rendering + help | 3, 4 |
| 6 | Executor wiring | 2, 3, 5 |
| 7 | README | 6 |

Tasks 2 and 3 can be parallelized after Task 1. Task 4 depends on 3. Task 5 depends on 3+4. Task 6 wires everything together. Task 7 is documentation.
