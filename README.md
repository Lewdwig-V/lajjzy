# lajjzy

A keyboard-driven, lazygit-style TUI for [Jujutsu (jj)](https://github.com/jj-vcs/jj).

Built for jj's data model: immutable changes, automatic rebasing, first-class conflicts, and an operation log that makes every action reversible. Built on [Textual](https://textual.textualize.io/) with a reactive, keyboard-first UI.

[![CI](https://github.com/Lewdwig-V/lajjzy/actions/workflows/ci.yml/badge.svg)](https://github.com/Lewdwig-V/lajjzy/actions/workflows/ci.yml)

> **Status:** early — the Python/Textual implementation is **not yet at feature
> parity** with the original Rust prototype. See [Feature status & gaps](#feature-status--gaps).

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

Press `S` to squash the selected change into its parent. This is a whole-change squash (no hunk picker yet). The parent's description is kept.

## Feature status & gaps

The Python/Textual implementation does **not yet have feature parity** with the
original Rust prototype. The MVP covers the core read-and-mutate loop; the tables
below track what's shipped (✅), partial (🚧), and not yet ported (❌). The
[roadmap](#roadmap) orders the remaining work — and the Rust prototype lives in
git history (pre-`reboot/python-textual`) as the behavioural reference for ports.

### Navigation & views

| Feature | Status | Notes |
|---|:--:|---|
| Graph navigation (`j`/`k`/`g`/`G`) | ✅ | cursor lands only on change nodes |
| Refresh (`R`), quit (`q`) | ✅ | |
| Focus graph ↔ detail (`Tab`) | ✅ | reverse-focus (`Shift-Tab`) not bound |
| Jump to working copy (`@`) | ❌ | |
| Detail file list + diff drill-down | ✅ | |
| Diff hunk jump (`n`/`N`) | ❌ | Textual scrolls the pane; no hunk-to-hunk jumps |
| Help overlay (`?`) | ❌ | |
| Mouse support | ❌ | click-to-select, scroll, click-to-focus |

### Mutations

| Feature | Status | Notes |
|---|:--:|---|
| New (`n`), abandon (`d`) | ✅ | |
| Switch working copy / edit (`Ctrl-E`) | ✅ | |
| Describe (`e`) | 🚧 | opens `$EDITOR`; no inline editor (Rust used `tui-textarea`) |
| Squash into parent (`S`) | 🚧 | whole-change only; no interactive hunk selection |
| Rebase (`r`) / with descendants (`Ctrl-R`) | 🚧 | target-picking works; no fuzzy filter or source-dimming |
| **Undo (`u`) / redo** | ❌ | **highest-value gap** — jj makes every op reversible |
| Split (`s`) | ❌ | needs the hunk picker |
| Partial squash | ❌ | needs the hunk picker |
| Set / delete bookmark (`B` / `b`) | ❌ | |
| Git push (`P`) / fetch (`f`) | ❌ | also no background push/fetch worker lanes |
| Absorb (`a`), duplicate (`D`), revert (`x`) | ❌ | |

### Panels & integrations

| Feature | Status | Notes |
|---|:--:|---|
| Status bar (change info, errors) | ✅ | |
| Omnibar — revset search + completion | ❌ | functions, bookmarks, change-IDs |
| Hunk picker (split / partial squash) | ❌ | |
| Conflict view (base / left / right) | ❌ | |
| Bookmark picker / input | ❌ | |
| Operation log (browse / restore) | ❌ | |
| GitHub / forge integration | ❌ | PR status, open / create via `gh` |

### Architecture deltas from the Rust version

- **Concurrency lanes:** Rust ran three independent lanes (local mutation, push,
  fetch). Python currently has `mutation` (exclusive), `load`, and `diff`; the
  push and fetch lanes arrive with bookmark / forge work.
- **Describe editor:** Rust embedded a `tui-textarea` inline editor; Python hands
  the terminal to `$EDITOR` via suspend. An inline editor may return as a Textual
  `TextArea`.
- **Spec management:** the Rust tree was unslop-managed; the Python line is
  hand-written for now (unslop may return once the architecture settles).

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
uv sync                      # create the environment
uv run lajjzy                # run the TUI
uv run pytest                # run tests (jj in PATH required for integration tests)
uv run ruff check .          # lint
uv run ruff format .         # format (use --check in CI)
```

CI (`.github/workflows/ci.yml`) runs `ruff check`, `ruff format --check`, and
`pytest` on every pull request and on pushes to `main` (installing `jj` so the
integration tests actually run).

## Releasing

1. Update `version` in `pyproject.toml`
2. `uv build` — produces `dist/lajjzy-X.Y.Z-py3-none-any.whl` and `.tar.gz`
3. `uv publish` — uploads to PyPI (requires `UV_PUBLISH_TOKEN` or `--token`)
4. Verify: `uv tool install lajjzy`, `pipx install lajjzy`

## Roadmap

Roughly priority-ordered: first close the gaps that make lajjzy a viable daily
driver, then add depth, then explore what the Textual stack newly makes possible.
See [Feature status & gaps](#feature-status--gaps) for the full inventory.

### Shipped

Graph navigation, detail + diff drill-down, core mutations (`new`, `abandon`,
`edit`, `describe`, `squash`, `rebase` / `rebase --descendants`), reactive status
bar, and CI (ruff + pytest).

### Next — the daily-driver essentials

1. **Undo / redo** (`jj undo` / `jj redo`) — the single most-felt gap; jj's whole
   selling point is reversibility, and the TUI can't yet undo.
2. **Omnibar** — revset search & filter with completion (functions, bookmarks,
   change-IDs).
3. **Bookmark management** — set / delete / move, plus a picker to jump.
4. **Operation log** — browse `jj op log` and restore to a previous operation.
5. **Conflict view** — base / left / right resolution panes.
6. **Hunk picker** — interactive `split` and partial `squash` (upgrades the
   current whole-change squash).

### Then — depth & ergonomics

- **Mouse support** — click-to-select, scroll, click-to-focus.
- **Remaining mutations** — absorb, duplicate, revert; push / fetch on their own
  background worker lanes.
- **Inline describe editor** — a Textual `TextArea` alternative to `$EDITOR`.
- **Configurable keymaps.**
- **Theming** — Textual CSS themes, colour sets, nerd-font / emoji support.
- **Forge integration** — `gh`-backed PR status, open / create (and beyond GitHub).

### Exploration — what Textual unlocks

The move off ratatui makes a few things newly cheap:

- **Run in the browser** — `textual serve` can host the same app over the web with
  no separate UI code.
- **Blame / annotate** — a gutter that drills into the originating change.
- **Parallel-branch lane view** for concurrent work.
- **Stacked-PR management** (Graphite-style).

## License

MPL 2.0
