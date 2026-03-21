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

- **M3 — Stack Workflows**: rebase with target picker, split with interactive hunk picker, partial squash, revset bar (omnibar)
- **M4 — Conflict Handling**: conflict file navigation, 3-way merge view, external merge tool launch
- **M5 — Forge Integration**: Gerrit, GitHub, GitLab — review status in graph, push-for-review
- **M6 — Polish**: configurable keymap, theming, mouse support, packaging
- **M7 — Parallel Branches**: lane view for concurrent work (git-butler model)
- **M8 — Gerrit Depth**: patchset comparison, review actions, inline comments
- **M9 — GitHub/GitLab Stacked PRs**: Graphite-style stack-aware PR management

## License

MIT
