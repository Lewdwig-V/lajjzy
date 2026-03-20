# lajjzy

A keyboard-driven, lazygit-style TUI for Jujutsu (jj).

## Requirements

- Rust 1.85+ (edition 2024)
- `jj` CLI in PATH (for core integration tests and running the binary)

## Build & Run

```bash
cargo build                    # build all crates
cargo run -p lajjzy            # run the TUI binary
cargo test                     # run all tests (requires jj in PATH for core integration tests)
cargo clippy -- -D warnings    # lint
cargo fmt --check              # format check
```

## Crate Structure

- `lajjzy-core` — `RepoBackend` trait and `JjCliBackend` implementation. All jj interaction goes through this crate. The TUI layer never shells out to jj directly.
- `lajjzy-tui` — ratatui widgets, input handling, state machine (AppState + Action + dispatch). Depends on `lajjzy-core` only.
- `lajjzy-cli` — Binary entry point. Terminal setup, event loop, panic handler. Depends on `lajjzy-tui` and `lajjzy-core`.

## Architectural Constraints

- **Facade boundary:** `lajjzy-tui` accesses jj only through `lajjzy-core::RepoBackend`. It never uses `std::process::Command` or imports jj-lib.
- **No panics on repo ops:** All `RepoBackend` methods return `Result`. Errors update `AppState.error`, never panic.
- **Dispatch purity (aspirational):** For M0, dispatch takes `&dyn RepoBackend`. In M2+, repo calls move to an effect executor.

## Key Patterns

- Elm-style state machine: `fn dispatch(state: &mut AppState, action: Action, backend: &dyn RepoBackend)`
- Graph data loaded in bulk via `load_graph()` — one jj subprocess call, not per-keypress.
- Cursor skips connector lines; always lands on change nodes.
