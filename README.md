# lajjzy

A keyboard-driven, lazygit-style TUI for [Jujutsu (jj)](https://github.com/jj-vcs/jj).

Built for jj's data model: immutable changes, automatic rebasing, first-class conflicts, and an operation log that makes every action reversible.

## Install

```bash
# Recommended: isolated tool install
uv tool install lajjzy
# or
pipx install lajjzy
```

**Requirements:** Python 3.11+ and the `jj` CLI in PATH (tested with jj 0.42.0).

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
│  ◉ ksqxwpml   │ Files changed:                │
│  ◉ ytoqrzxn   │   M src/lib.rs                │
│  ◉ zzzzzzzz   │   A src/new.rs                │
│               │                               │
├───────────────┴───────────────────────────────┤
│ Status bar: change info, errors, operations   │
└───────────────────────────────────────────────┘
```

- **Graph panel** (1/3 width): Interactive change DAG. Cursor moves between change nodes, skipping connector lines.
- **Detail panel** (2/3 width): File list → diff view drill-down for the selected change.
- **Status bar**: Change metadata, operation progress, error messages.

## Features

### Graph Navigation

Navigate the change DAG with vim-style keys. The cursor always lands on change nodes, never on connector lines between them.

| Key | Action |
|-----|--------|
| `j` / `↓` | Move to next change |
| `k` / `↑` | Move to previous change |
| `g` | Jump to top |
| `G` | Jump to bottom |
| `Tab` | Focus detail pane |
| `R` | Refresh graph from disk |
| `q` | Quit |

### Detail Pane

The detail pane shows the file list for the selected change. Navigate files with `j`/`k`, press `Enter` to view the diff, `Esc` to go back to the file list.

| Key | Action |
|-----|--------|
| `j` / `↓` | Next file |
| `k` / `↑` | Previous file |
| `Enter` | Open diff view for file |
| `Esc` | Back to file list |

### Mutations

All mutations are performed from the graph panel.

| Key | Action |
|-----|--------|
| `n` | New change (inserts after selected) |
| `d` | Abandon selected change |
| `e` | Describe (opens `$EDITOR` for long-form editing) |
| `Ctrl-E` | Switch working copy (`@`) to selected change |
| `S` | Squash selected change into its parent (whole change; no hunk picker yet) |
| `r` | Rebase change — navigate to destination, `Enter` to confirm, `Esc` to cancel |
| `Ctrl-R` | Rebase change with all descendants — same pick-destination flow |

**Concurrency:** Load and mutation operations run on independent worker groups. At most one mutation runs at a time (exclusive gate); graph loads run separately without blocking.

### Describe Editor

Press `e` to edit the description of the selected change. The description is opened in your `$EDITOR` via terminal suspend (the TUI hands the terminal to the editor and resumes when the editor exits). `$EDITOR` must be set in your environment.

### Rebase

Press `r` to rebase the selected change, or `Ctrl-R` to rebase with all descendants. The graph enters **target-picking mode**: navigate to the destination change and press `Enter` to confirm, or `Esc` to cancel.

### Squash

Press `S` to squash the selected change into its parent. This is a whole-change squash (no hunk picker in the MVP). The parent's description is kept.

> **Status:** Partial squash / hunk picker — planned, not yet in the Python reboot.

### Omnibar (Revset Search & Filter)

> **Status:** planned — not yet in the Python reboot.

### Split & Partial Squash (Hunk Picker)

> **Status:** planned — not yet in the Python reboot.

### Conflict View

> **Status:** planned — not yet in the Python reboot.

### Bookmark Management

> **Status:** planned — not yet in the Python reboot.

### Op Log

> **Status:** planned — not yet in the Python reboot.

Browse jj's operation history and restore to a previous operation.

### Mouse Support

> **Status:** planned — not yet in the Python reboot.

### GitHub Integration

> **Status:** planned — not yet in the Python reboot.

## Architecture

Three layers with a strict facade boundary:

- **`backend/`** — the only code that shells out to `jj`. Async functions
  (`asyncio.create_subprocess_exec`) returning typed dataclasses; pure parsers
  in `parse.py`. No Textual imports.
- **reactive UI** — Textual `reactive()` attributes on the `App` and widgets,
  with `watch_*`/`compute_*` for derived state. No central store object.
- **workers** — every jj call runs in a `@work` worker. Concurrency lanes are
  worker groups: `group="mutation"` (with `exclusive=True`, the
  single-mutation gate), `group="load"`, `group="diff"`.

There is no `dispatch`/`Effect` machine — actions invoke workers, workers write
reactive state, and the affected widgets re-render automatically.

## Development

```bash
uv sync                 # create the environment
uv run lajjzy           # run the TUI
uv run pytest           # run tests (jj in PATH required for integration tests)
uv run ruff check .     # lint
uv run ruff format .    # format
```

## Releasing

1. Update `version` in `pyproject.toml`
2. `uv build` — produces `dist/lajjzy-X.Y.Z-py3-none-any.whl` and `.tar.gz`
3. `uv publish` — uploads to PyPI (requires `UV_PUBLISH_TOKEN` or `--token`)
4. Verify: `uv tool install lajjzy`, `pipx install lajjzy`

## Roadmap

### Reboot R1 — MVP core (complete)

Graph + navigation, detail/diff panel, core mutations (`new`, `describe`, `edit`,
`abandon`, `squash`, `rebase`), status bar + error reactivity.

### Feature-port backlog (port from the Rust reference, incrementally)

- **P1 — Omnibar:** revset search + completion (functions, bookmarks, change IDs).
- **P2 — Hunk picker:** interactive split & partial squash.
- **P3 — Conflict view:** base/left/right resolution panes.
- **P4 — Bookmark UI:** picker + set/delete.
- **P5 — Op log:** browse + restore.
- **P6 — Mouse support:** lazygit-style click/scroll.
- **P7 — GitHub integration:** `gh`-backed PR status + open/create.

### Future features (post-parity)

- **F1 — Configurable keymaps**
- **F2 — Theming:** colour sets, nerd-font / emoji support.
- **F3 — Blame / annotate**
- **F4 — Parallel-branch lane view**
- **F5 — Stacked-PR management**

## License

MPL 2.0
