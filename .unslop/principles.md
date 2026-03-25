# Project Principles

<!-- Non-negotiable constraints for all generated code. Enforced during every generation cycle. -->

## Architecture
- Three-crate workspace: `lajjzy-core` (backend), `lajjzy-tui` (widgets/state/dispatch), `lajjzy-cli` (binary/effects)
- Facade boundary: `lajjzy-tui` never imports `RepoBackend`, `std::process::Command`, or jj-lib
- Effects defined in `lajjzy-tui`, executed in `lajjzy-cli` only
- Dispatch is pure: `fn dispatch(&mut AppState, Action) -> Vec<Effect>` — no I/O, no backend calls

## Error Handling
- All `RepoBackend` methods return `Result` — errors update `AppState.error`, never panic
- No silent failures: caught errors that are logged-and-ignored are critical failures
- Panic at the boundary: the decision to panic lies with the caller, not the library

## Concurrency
- Mutation gate: at most one local mutation in flight via `AppState.pending_mutation`
- Three independent lanes: local mutations, push, fetch — no blocking between lanes

## Style
- Rust edition 2024, MSRV 1.85
- `clippy::all` and `clippy::pedantic` at warn level
- Elm-style state machine pattern throughout
- Prefer lazygit UX patterns unless jj requires otherwise
