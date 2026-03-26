# M6 — Forge Integration Design (GitHub)

## Scope

GitHub PR status in the graph via `gh` CLI, push-for-review via `gh pr create`, runtime tool detection with graceful degradation. GitLab (`glab`) and Gerrit are future milestones using the same abstraction.

**What we build:**
- Runtime detection of `gh` CLI at startup
- On-demand PR status fetch (`F` key) via `gh pr list --json`
- PR indicators in the graph (number + review status, colored)
- Open/create PR (`W` key) — opens browser if PR exists, suspends for `gh pr create` if not
- Browser fallback: always show clickable URL in status bar alongside browser launch
- Graceful degradation: forge keys hidden from help when `gh` not found, error messages guide user

**What we don't build:** GitLab support, Gerrit support, automatic polling, CI status display, inline PR comments, PR merge actions.

## Data Model

### New types (`lajjzy-core/src/forge.rs`)

Forge types live in a separate module from jj types, keeping `types.rs` focused on repo data:

```rust
// lajjzy-core/src/forge.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForgeKind {
    GitHub,
    // Future: GitLab, Gerrit
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
```

### PR-to-change link

Bookmark name matches PR `headRefName`. A change with bookmark `feature-x` links to a PR whose `headRefName` is `feature-x`. This is the only stable link — commit SHAs drift with amend/rebase.

## Backend — Separate ForgeBackend

Forge operations are **not** on `RepoBackend`. `RepoBackend` is for jj repo access; forge operations shell out to `gh`, a different tool. A separate `ForgeBackend` trait keeps concerns isolated.

### ForgeBackend trait (`lajjzy-core/src/forge.rs`)

```rust
pub trait ForgeBackend: Send + Sync {
    /// Which forge CLI is available, if any.
    fn forge_kind(&self) -> Option<ForgeKind>;

    /// Fetch PR/MR status from the forge.
    /// Returns Ok(None) when no forge CLI is available (not an error).
    fn fetch_status(&self) -> Result<Option<Vec<PrInfo>>>;
}
```

### GhCliForge implementation (`lajjzy-core/src/gh.rs`)

```rust
pub struct GhCliForge {
    workspace_root: PathBuf,
    available: bool,
}
```

**Construction:** Checks `Command::new("gh").arg("--version").output()` at creation time. Caches result in `available`.

**`fetch_status()`:** When available, runs:

```bash
gh pr list --state open --limit 100 --json number,title,state,headRefName,reviewDecision,url
```

Uses `--state open --limit 100` instead of `--state all` to avoid consuming the default 30-item limit with stale closed/merged PRs. Open PRs are what users care about in the graph. All `gh` commands run with `current_dir` set to `workspace_root` so `gh` finds the correct git remote.

JSON parsing maps: `state` → `PrState`, `reviewDecision` → `ReviewStatus` (empty string → `Unknown`).

When not available: returns `Ok(None)`.

### Facade boundary

`lajjzy-tui` imports `lajjzy_core::forge::{ForgeKind, PrInfo, PrState, ReviewStatus}` (types only). It does NOT import `ForgeBackend` or `GhCliForge` — those are used only in `lajjzy-cli`. This matches the existing pattern where `lajjzy-tui` imports `lajjzy_core::types` but never `RepoBackend`.

`ForgeKind` is injected into `AppState` at construction time by `lajjzy-cli`:

```rust
// In lajjzy-cli/src/main.rs:
let forge = GhCliForge::new(&cwd);
let mut state = AppState::new(graph, forge.forge_kind());
```

`AppState::new` gains an `Option<ForgeKind>` parameter.

## AppState

```rust
pub forge: Option<ForgeKind>,
pub pr_status: HashMap<String, PrInfo>,  // keyed by head_ref (bookmark name)
pub pending_forge_fetch: bool,           // prevents duplicate fetches
```

`forge` is set once at startup. `pr_status` is populated by `FetchForgeStatus`. `pending_forge_fetch` is cleared by `ForgeStatusLoaded` (both success and error paths).

## Effects, Actions, Dispatch

### New effects

```rust
Effect::FetchForgeStatus,
Effect::OpenPrInBrowser { bookmark: String, url: String },
Effect::CreatePr { bookmark: String },
```

**Interaction patterns:**
- `FetchForgeStatus` — **Background** read-only, no mutation gate
- `OpenPrInBrowser` — **Instant** fire-and-forget (always shows URL in status bar)
- `CreatePr` — **Suspend** pattern (same as `SuspendForEditor`)

`OpenPrInBrowser` carries the `url` directly (extracted from `pr_status` in dispatch). The executor attempts `gh pr view --web` AND always sends the URL back for status bar display. No need to detect browser failure — the URL is shown regardless.

### New actions

```rust
Action::FetchForgeStatus,
Action::OpenOrCreatePr,
Action::ForgeStatusLoaded(Result<Option<Vec<PrInfo>>, String>),
Action::PrViewUrl { url: String },  // URL for status bar (always sent after OpenPrInBrowser)
Action::PrCreateComplete,           // resume from gh pr create
Action::PrCreateFailed { error: String },
```

### Dispatch logic

**`Action::FetchForgeStatus`:**
- If `state.forge` is `None`: set error "Install gh for GitHub integration"
- If `state.pending_forge_fetch`: return empty (debounce)
- Set `state.pending_forge_fetch = true`, emit `Effect::FetchForgeStatus`

**`Action::OpenOrCreatePr`:**
- If `state.forge` is `None`: set error "Install gh for GitHub integration"
- Get selected change's bookmarks. If empty: set error "No bookmark on this change — set one with B first"
- For bookmark selection with multiple bookmarks: use the first bookmark in the list (jj's bookmark order). This is deterministic — jj returns bookmarks in a consistent order.
- Look up bookmark in `state.pr_status`:
  - Found → emit `Effect::OpenPrInBrowser { bookmark, url: pr.url.clone() }`
  - Not found → emit `Effect::CreatePr { bookmark }`

**`Action::ForgeStatusLoaded`:**
- Always clear `state.pending_forge_fetch`
- `Ok(Some(prs))` → populate `state.pr_status` HashMap, set status "Loaded N PRs"
- `Ok(None)` → no-op
- `Err(e)` → set `state.error`

**`Action::PrViewUrl { url }`:**
- Set `state.status_message` to the URL (terminal-clickable via ctrl-click)

**`Action::PrCreateComplete`:**
- Emit `Effect::FetchForgeStatus` to pick up the new PR

**`Action::PrCreateFailed { error }`:**
- Set `state.error`

### Executor wiring

**`Effect::FetchForgeStatus`:** Spawned on background thread (same pattern as `LoadOpLog`):
```rust
Effect::FetchForgeStatus => {
    let result = forge.fetch_status().map_err(|e| e.to_string());
    let _ = tx.send(Action::ForgeStatusLoaded(result));
}
```

**`Effect::OpenPrInBrowser`:** Spawned on background thread. Attempts `gh pr view <bookmark> --web` (fire-and-forget), then sends URL for status bar:
```rust
Effect::OpenPrInBrowser { bookmark, url } => {
    // Best-effort browser launch — ignore result
    let _ = Command::new("gh")
        .args(["pr", "view", &bookmark, "--web"])
        .current_dir(&workspace_root)
        .output();
    let _ = tx.send(Action::PrViewUrl { url });
}
```

**`Effect::CreatePr`:** Intercepted in `execute_effects` on main thread (suspend pattern):
```rust
Effect::CreatePr { bookmark } => {
    ratatui::restore();
    let status = Command::new("gh")
        .args(["pr", "create", "--head", &bookmark])
        .current_dir(&workspace_root)
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

`gh pr create` without `--fill` launches an interactive prompt in the terminal. This matches the user's `gh` configuration — if they've set up web-based PR creation, `gh` will open the browser instead. We let `gh` decide the interaction mode.

## Rendering

### Graph widget

After bookmark names, append PR indicator for any bookmark matching a key in `pr_status`:

```
◉ ksqxwpml alice 2m ago  main #42 ✓approved
◉ ytoqrzxn bob 1h ago    feature-x #15 ●changes-requested
```

**Colors:**
- `✓approved` — green
- `●review-required` — yellow
- `✗changes-requested` — red
- `✓merged` — green, dim
- `✗closed` — dim

Indicators only render when `pr_status` is non-empty (after first `F` press). Before fetch, graph looks exactly as today.

**Multiple bookmarks:** For a change with multiple bookmarks, show PR indicators for ALL bookmarks that have matching PRs (not just the first). Each gets its own `#N status` indicator.

### Input (graph panel)

```rust
(KeyCode::Char('F'), _) => Some(Action::FetchForgeStatus),
(KeyCode::Char('W'), _) => Some(Action::OpenOrCreatePr),
```

### Help text

When `forge` is `Some`:
```
F         Fetch PR status (GitHub)
W         Open/create PR in browser
```

When `forge` is `None`: these lines are omitted from help.

### Status bar

No dedicated forge section. Messages flow through existing slots:
- After `F`: "Loaded 3 PRs" / "No PRs found"
- After `W`: "https://github.com/owner/repo/pull/42" (always shown, clickable)
- Forge keys without `gh`: "Install gh for GitHub integration"

## Error Handling

- `gh` not in PATH → `forge: None`, keybindings show guidance message, help hides forge keys
- `gh` not authenticated → `gh pr list` exits non-zero → `state.error` with gh's stderr
- No git remote → `gh pr list` returns empty list → empty `pr_status`, no indicators
- No bookmark on selected change → `W` shows "No bookmark — set one with B first"
- Bookmark has no PR → `W` triggers `CreatePr`
- `gh pr create` cancelled by user (Ctrl-C) → non-zero exit → resume TUI, set error (harmless)
- Multiple bookmarks → `W` uses first bookmark; graph shows all matched PR indicators

## Testing

- **Dispatch tests (pure):** `FetchForgeStatus` emits effect when forge available, error when not. `pending_forge_fetch` debounce. `OpenOrCreatePr` routes to browser vs create based on `pr_status`. No-bookmark error. Forge-unavailable error. `ForgeStatusLoaded` clears pending flag on both success and error.
- **JSON parsing tests:** Parse real `gh pr list` JSON into `Vec<PrInfo>`. All state/review combinations. Empty list. Malformed JSON.
- **Graph rendering tests:** PR indicator when `pr_status` matches bookmark. No indicator when empty. Multiple bookmarks with PRs.
- **Input tests:** `F` → `FetchForgeStatus`, `W` → `OpenOrCreatePr` in graph context. Neither in detail.
- **Integration tests:** Require `gh` in PATH — skip if not available.
