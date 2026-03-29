---
managed_file: crates/lajjzy-tui/src/widgets/bookmark_input.rs
intent: >
  Renders a bordered single-input overlay widget for naming or selecting a bookmark. Displays the current text input with a trailing cursor indicator on the first inner line. On the second inner line, renders a filtered list of matching completions (case-insensitive substring match against the input) drawn from a caller-supplied list, capped to fit available width. The widget is stateless and read-only: it renders what it receives and owns no input state.
intent-approved: false
intent-hash: 239be1f819d7
distilled-from:
  - path: crates/lajjzy-tui/src/widgets/bookmark_input.rs
    hash: 4d4f789f138d
non-goals:
  - Owning or mutating the input string in response to keystrokes
  - Tracking or highlighting a currently-selected completion entry
  - Persisting or committing the bookmark name to the backend
depends-on: []
---

## Purpose

`BookmarkInputWidget` is a pure rendering widget. Callers construct it with a current input string and a list of candidate bookmark names, then render it into a `Buffer`. The widget produces a yellow-bordered overlay with a title line describing the confirm/cancel keys, an input line showing the typed text and cursor, and (when space and matches exist) a completion hint line below the input.

## Behavior

1. **Border and title** — renders a full `Borders::ALL` block with a yellow border and the fixed title ` Set bookmark (Enter confirm | Esc cancel) `.
2. **Input line (inner row 0)** — renders `"Bookmark: "` in yellow, followed by the raw input string, followed by `"|"` in dark gray as a cursor indicator.
3. **Early exit on zero inner height** — if the inner area has no rows, rendering stops after the border; nothing else is written.
4. **Completion line suppressed when insufficient height** — if `inner.height < 2`, the completion line is not rendered.
5. **Completion line suppressed when completions list is empty** — if the caller passes an empty completions slice, the completion line is not rendered.
6. **Case-insensitive substring filtering** — the completions list is filtered by lowercasing both the candidate and the input and testing `contains`.
7. **Width-based cap** — at most `inner.width / 10 + 1` completions are shown, preventing overflow on narrow terminals.
8. **Completion line (inner row 1)** — matching completions are rendered in magenta, separated by two spaces. No separator is appended after the last item.
9. **No matches suppresses the completion line** — if filtering yields zero matches the line is not written even when height is sufficient.
10. **Render with empty input** — renders without panic when the input string is empty and the completions list is empty.

## Constraints

- The widget is `'a`-lifetime-bound to its input data; it holds only shared references and performs no allocation beyond the temporary `matches` and `spans` vecs during render.
- The completion cap formula `inner.width as usize / 10 + 1` guarantees at least one completion is shown regardless of width, but may still overflow if individual names are very long — the `buf.set_line` call clips to `inner.width` automatically.
- `inner.height == 0` is handled explicitly to avoid an out-of-bounds write into the buffer.

## Dependencies

- `ratatui` — `Buffer`, `Rect`, `Widget`, `Block`, `Borders`, `Style`, `Color`, `Line`, `Span`
