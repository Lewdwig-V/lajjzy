# lajjzy

A keyboard-driven, lazygit-style TUI for [Jujutsu (jj)](https://github.com/jj-vcs/jj).

Built for jj's data model: immutable changes, automatic rebasing, first-class conflicts, and an operation log that makes every action reversible.

## Install

### Pre-built binaries (recommended)

```bash
# Via cargo-binstall (fastest)
cargo binstall lajjzy

# Via Homebrew (macOS)
brew tap lewdwig-v/lajjzy && brew install lajjzy

# Via Nix
nix run github:Lewdwig-V/lajjzy
# or persistent install:
nix profile install github:Lewdwig-V/lajjzy
```

### From source

```bash
# Via cargo (from crates.io)
cargo install lajjzy

# Or build locally (requires Rust 1.85+)
git clone https://github.com/Lewdwig-V/lajjzy
cd lajjzy
cargo build --release
```

**Requirements:** `jj` CLI in PATH (tested with jj 0.39.0).

## Usage

```bash
# Run in any jj workspace
cd my-jj-repo
lajjzy
```

## Layout

```
┌───────────────┬───────────────────────────────┐
│ Change Graph  │ Detail Pane                   │
│               │                               │
│ ◉ ksqxwpml   │ Files changed:                │
│ ◉ ytoqrzxn   │   M src/lib.rs                │
│ ◉ zzzzzzzz   │   A src/new.rs                │
│               │                               │
├───────────────┴───────────────────────────────┤
│ Status bar: change info, errors, operations   │
└───────────────────────────────────────────────┘
```

- **Graph panel** (1/3 width): Interactive change DAG. Cursor moves between change nodes, skipping connector lines.
- **Detail panel** (2/3 width): File list → diff view drill-down for the selected change.
- **Status bar**: Change metadata, active revset filter, operation progress, error messages.

## Features

### Graph Navigation

Navigate the change DAG with vim-style keys. The cursor always lands on change nodes, never on connector lines between them.

| Key | Action |
|-----|--------|
| `j` / `↓` | Move to next change |
| `k` / `↑` | Move to previous change |
| `g` | Jump to top |
| `G` | Jump to bottom |
| `@` | Jump to working copy |
| `Tab` / `Shift-Tab` | Switch focus between graph and detail pane |
| `R` | Refresh graph from disk |

### Detail Pane

The detail pane shows the file list for the selected change. Press `Enter` to drill into a file's diff, `Esc` to go back.

**File list:**

| Key | Action |
|-----|--------|
| `j` / `↓` | Next file |
| `k` / `↑` | Previous file |
| `Enter` | Open diff view for file |
| `Esc` | Back to graph focus |

**Diff view:**

| Key | Action |
|-----|--------|
| `j` / `↓` | Scroll down |
| `k` / `↑` | Scroll up |
| `n` | Jump to next hunk |
| `N` | Jump to previous hunk |
| `Esc` | Back to file list |

### Mutations

All mutations are performed from the graph panel. Every mutation is reversible via `u` (undo) — no confirmation dialogs.

| Key | Action |
|-----|--------|
| `d` | Abandon selected change |
| `n` | New change (inserts after selected) |
| `e` | Edit description (inline editor) |
| `Ctrl-E` | Switch working copy (`@`) to selected change |
| `s` | Split change (interactive hunk picker) |
| `S` | Partial squash into parent (interactive hunk picker) |
| `u` | Undo last operation |
| `Ctrl-Shift-R` | Redo |
| `r` | Rebase change onto a new destination |
| `Ctrl-R` | Rebase change with all descendants |
| `B` | Set bookmark on selected change |
| `P` | Git push |
| `f` | Git fetch |

**Concurrency:** Local mutations, push, and fetch run on three independent background lanes. You can push while a fetch is in-flight, or start a new mutation the moment the previous one completes. Each lane has its own gate — no operation blocks another lane.

### Describe Editor

An inline multi-line editor for change descriptions, powered by tui-textarea.

| Key | Action |
|-----|--------|
| `Ctrl-S` / `Ctrl-Enter` / `Alt-Enter` | Save description |
| `Escape` | Discard changes |
| `Shift-E` | Open in `$EDITOR` for long-form editing |

### Omnibar (Revset Search & Filter)

Press `/` to open the omnibar. Type any [jj revset expression](https://jj-vcs.github.io/jj/latest/revsets/) to filter the graph. The omnibar replaces the graph with matching changes when you press Enter.

| Key | Action |
|-----|--------|
| `/` | Open omnibar |
| Type text | Filter by revset expression |
| `↓` / `↑` / `Ctrl-N` / `Ctrl-P` | Navigate results |
| `Enter` | Submit revset query (filter graph) |
| `Tab` | Accept autocomplete suggestion |
| `Esc` | Dismiss omnibar |

**Autocomplete:** As you type, the omnibar offers prefix-matched suggestions for:
- **Revset functions** — all 24 built-in jj revset functions (e.g., `ancestors(`, `mine()`, `author(`)
- **Bookmarks** — all bookmark names in the repo
- **Change IDs** — short change IDs (after 2+ characters typed)

Nullary functions insert with both parens (`mine()`), functions with arguments insert with an open paren (`author(`). Tab accepts, Enter always submits the full query.

### Rebase

Press `r` to rebase the selected change, or `Ctrl-R` to rebase with all descendants. The graph enters **target-picking mode**: navigate to the destination change and press Enter to confirm, or Esc to cancel.

While picking a target, type to filter — an inline fuzzy filter narrows the visible changes. The source change and its descendants are dimmed to prevent invalid rebase targets.

### Split & Partial Squash

Press `s` to split a change, or `S` to partially squash into its parent. Both open the **hunk picker** — a single-column scrollable list showing all changed files and their hunks.

| Key | Action |
|-----|--------|
| `j` / `↓` | Next item |
| `k` / `↑` | Previous item |
| `J` | Jump to next file |
| `K` | Jump to previous file |
| `Space` | Toggle hunk selection |
| `a` | Select all hunks |
| `A` | Deselect all hunks |
| `Enter` | Confirm selection |
| `Esc` / `Ctrl-C` | Cancel |

Selected hunks are tinted cyan. File headers show selection counts (e.g., `[2/5]`).

### Modals

| Key | Action |
|-----|--------|
| `b` | Bookmark picker — jump to a bookmarked change, or `d` to delete a bookmark |
| `O` | Operation log — browse jj's operation history, Enter to restore |
| `?` | Context-sensitive help |

### Bookmark Management

| Key | Action |
|-----|--------|
| `b` | Open bookmark picker (navigate + jump, `d` to delete) |
| `B` | Set a new bookmark on the selected change (type name, Enter to confirm) |

## Architecture

Three crates with strict dependency boundaries:

- **`lajjzy-core`** — `RepoBackend` trait and `JjCliBackend` implementation. All jj CLI interaction goes through this crate.
- **`lajjzy-tui`** — ratatui widgets, input handling, pure Elm-style state machine (`AppState` + `Action` + `dispatch`). Never imports `RepoBackend` or spawns subprocesses.
- **`lajjzy-cli`** — Binary entry point. Terminal setup, event loop, effect executor. The only crate that performs I/O.

`dispatch()` is a pure function: `fn dispatch(state: &mut AppState, action: Action) -> Vec<Effect>`. All backend calls flow through the effect executor in `lajjzy-cli`.

## Development

```bash
cargo build                    # build all crates
cargo test                     # run all tests (requires jj in PATH)
cargo clippy -- -D warnings    # lint
cargo fmt --check              # format check
```

See `CLAUDE.md` for architectural constraints and crate structure.

## Releasing

1. Ensure `Cargo.lock` is up to date and committed
2. Update version in `Cargo.toml` (`[workspace.package] version`)
3. `cargo build --release` locally to verify
4. `git tag vX.Y.Z && git push --tags`
5. Wait for release workflow (builds binaries, creates GitHub Release, publishes to crates.io)
6. Copy sha256 sums from release notes into `homebrew-lajjzy` formula
7. Verify: `cargo install lajjzy`, `cargo binstall lajjzy`, `brew install lajjzy`, `nix run github:Lewdwig-V/lajjzy`

## Roadmap

- **M4 — Conflict Handling**: conflict file navigation, 3-way merge view, external merge tool launch
- **M5 — Release Packaging**: `cargo install lajjzy`, `cargo binstall`, Nix flake (jj community leans heavily on Nix)
- **M6 — Forge Integration**: Gerrit, GitHub, GitLab — review status in graph, push-for-review
- **M7 — More Mutations**: `jj absorb` (single keypress — auto-distribute fixup hunks to ancestor commits), `jj duplicate`, `jj backout`
- **M8a — Polish**: configurable keymaps
- **M8b — Polish**: theming support, colour sets, nerd font support, noto emoji, statusline fonts
- **M8c — Polish**: basic mouse support
- **M8d — Polish**: collapsible command log pane showing jj commands run on your behalf
- **M8e — Polish**: `jj move` hunks in hunk picker, advanced rebasing workflows
- **M8f — Polish**: context-aware revset completions (e.g., only authors inside `author()`)
- **M8g — Polish**: workspace name in status bar, workspace picker modal (list, switch, create)
- **M9 — Blame / Annotate**: file content with annotation gutter (change ID, author, date), drill into blame line to jump to originating change
- **M10 — Parallel Branches**: lane view for concurrent work (git-butler model)
- **M11 — Gerrit Depth**: patchset comparison, review actions, inline comments
- **M12 — GitHub/GitLab Stacked PRs**: Graphite-style stack-aware PR management

## License

MPL 2.0
