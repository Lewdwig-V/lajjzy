# lajjzy

A keyboard-driven, lazygit-style TUI for [Jujutsu (jj)](https://github.com/jj-vcs/jj).

Built for jj's data model: immutable changes, automatic rebasing, first-class conflicts, and an operation log that makes every action reversible.

## Install

```bash
# From source (requires Rust 1.85+)
cargo install --path crates/lajjzy-cli

# Or build locally
cargo build --release
```

**Requirements:** `jj` CLI in PATH (tested with jj 0.39.0).

## Usage

```bash
# Run in any jj workspace
cd my-jj-repo
lajjzy
```

## Key Bindings

### Navigation

| Key | Action |
|-----|--------|
| `j` / `k` | Move between changes (graph) or files (detail) |
| `g` / `G` | Jump to top / bottom |
| `@` | Jump to working copy |
| `Tab` | Switch focus between graph and detail pane |
| `Enter` | Drill into file diff |
| `Esc` | Back / dismiss |

### Mutations

| Key | Action |
|-----|--------|
| `d` | Abandon selected change |
| `n` | New change (inserts after selected) |
| `e` | Edit description (inline editor) |
| `Ctrl-E` | Switch working copy to selected change |
| `S` | Squash into parent |
| `u` | Undo |
| `Ctrl-R` | Redo |
| `B` | Set bookmark on selected change |
| `P` | Git push |
| `f` | Git fetch |

### Modals

| Key | Action |
|-----|--------|
| `b` | Bookmark picker |
| `/` | Fuzzy find |
| `O` | Operation log |
| `?` | Help |

### Describe Editor

| Key | Action |
|-----|--------|
| `Ctrl-S` / `Ctrl-Enter` | Save description |
| `Escape` | Discard changes |
| `Shift-E` | Open in `$EDITOR` |

No confirmation dialogs — every mutation is reversible via `u` (undo).

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

- **Graph panel** (1/3): Interactive change DAG. Cursor moves between changes, not lines.
- **Detail panel** (2/3): File list, diff view, or describe editor depending on context.
- **Status bar**: Change metadata, error messages, operation progress.

## Development

```bash
cargo build                    # build all crates
cargo test                     # run all tests (requires jj in PATH)
cargo clippy -- -D warnings    # lint
cargo fmt --check              # format check
```

See `CLAUDE.md` for architectural constraints and crate structure.

## Roadmap

### Done

- **M0** — Read-only TUI with graph navigation
- **M1** — Detail pane with file list, diff view, overlays (op log, bookmark picker, fuzzy find, help)
- **M2** — Pure dispatch with effect executor, 10 mutations (abandon, squash, new, edit, describe, undo, redo, bookmark set/delete, push, fetch)

### Planned

- **M3a — Omnibar**: revset-first search/filter bar, replaces fuzzy-find
- **M3b — Rebase**: target picker modal, `jj rebase` with destination selection
- **M3c — Split & Partial Squash**: interactive hunk picker widget, the hardest UI in the app
- **M3d — Autocomplete**: for jj revset query language syntax in the Omnibar
- **M4 — Conflict Handling**: conflict file navigation, 3-way merge view, external merge tool launch
- **M5 — Forge Integration**: Gerrit, GitHub, GitLab — review status in graph, push-for-review
- **M6a — Polish**: configurable keymaps
- **M6b — Polish**: theming support; colour sets; nerd font support; noto emoji font support; statusline font support.
- **M6c — Polish**: basic mouse support
- **M6d — Polish**: release packaging; enable easy publishing to crates.io; `cargo install lajjzy`, `cargo binstall` support; Nix flake (jj community leans heavily on Nix) 
- **M6e — Polish**: collapsible command log pane which shows which jj commands we run on behalf of the user
- **M6f — Polish**: `jj move` hunks in hunk picker; other advanced rebasing workflows (if not too niche) 
- **M7 — Parallel Branches**: lane view for concurrent work (git-butler model)
- **M8 — Gerrit Depth**: patchset comparison, review actions, inline comments
- **M9 — GitHub/GitLab Stacked PRs**: Graphite-style stack-aware PR management

## License

MPL 2.0
