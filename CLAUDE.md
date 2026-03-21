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

- **Facade boundary:** `lajjzy-tui` never imports `RepoBackend`, `std::process::Command`, or jj-lib. The `Effect::SuspendForEditor` variant is *defined* in `lajjzy-tui` (as part of the `Effect` enum) but *executed* in `lajjzy-cli` — the `execute_effects` function in `main.rs` intercepts it before it reaches the executor and handles the terminal suspend/resume + `std::process::Command` launch there. No subprocess is ever spawned from `lajjzy-tui` code.
- **No panics on repo ops:** All `RepoBackend` methods return `Result`. Errors update `AppState.error`, never panic.
- **Dispatch purity:** `dispatch()` takes `(&mut AppState, Action)` and returns `Vec<Effect>`. It never calls backend methods or performs I/O.
- **Effect executor boundary:** Effects executed in `lajjzy-cli` only. `lajjzy-tui` defines the `Effect` enum but never executes effects.
- **Mutation gate:** At most one local mutation in flight, enforced by `AppState.pending_mutation`. Background ops (push/fetch) gated independently.
- **Interaction patterns:** Every mutation declares its slot (Instant, Mini-modal, Background). New patterns require design justification.
- **Working-copy gate for filesystem ops:** Any operation that reads or writes repo files on disk (file editing, merge tools, anything that touches the working tree) requires the target change to be `@`. If it is not, dispatch must emit `Effect::Edit` first to switch the working copy, then proceed. This is visible to the user — the `@` marker moves before the tool launches.

## Key Patterns

- Elm-style state machine: `fn dispatch(state: &mut AppState, action: Action) -> Vec<Effect>`
- Graph data loaded in bulk via `load_graph()` — one jj subprocess call, not per-keypress.
- Cursor skips connector lines; always lands on change nodes.
- Three concurrency lanes: local mutations, push, fetch — independent gates, no blocking between lanes.
- Dispatch is pure: all backend calls and I/O flow through the effect executor in `lajjzy-cli`.
