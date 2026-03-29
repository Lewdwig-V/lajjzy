---
managed-file: crates/lajjzy-core/src/gh.rs
intent: >
  Implements ForgeBackend for GitHub by shelling out to the gh CLI. On
  construction, probes gh --version to record availability. forge_kind()
  returns Some(GitHub) when available and None otherwise. fetch_status()
  invokes gh pr list --state open --limit 100 and parses the JSON response
  into a Vec<PrInfo>, mapping GitHub's OPEN/MERGED/CLOSED state strings and
  APPROVED/CHANGES_REQUESTED/REVIEW_REQUIRED review-decision strings to typed
  enum variants; null or unrecognised review decisions map to
  ReviewStatus::Unknown. Returns Ok(None) when gh is unavailable, Err when
  the subprocess fails or output is non-UTF-8 or malformed JSON.
intent-approved: false
intent-hash: 2bca086b3916
distilled-from:
  - path: crates/lajjzy-core/src/gh.rs
    hash: b19f7010a842
non-goals:
  - Does not authenticate or manage gh credentials — assumes the ambient gh session is already authenticated
  - Does not support forges other than GitHub (GitLab, Bitbucket, etc.)
  - Does not cache or rate-limit gh invocations — each fetch_status() call spawns a fresh subprocess
depends-on:
  - crates/lajjzy-core/src/forge.spec.md
---

## Purpose

`GhCliForge` is a `ForgeBackend` implementation that surfaces open-PR status
from GitHub by delegating to the `gh` CLI already installed in the user's
environment. Callers receive a typed `Vec<PrInfo>` without needing to know
anything about the GitHub API or authentication mechanism.

## Behavior

1. **Construction** — `GhCliForge::new(workspace_root)` runs `gh --version`
   and records whether the exit code is success. The struct stores the
   workspace root for later use as the working directory of subprocess calls.

2. **forge_kind()** — Returns `Some(ForgeKind::GitHub)` when `gh` was found
   on construction; returns `None` otherwise.

3. **fetch_status() — unavailable** — When `available` is false, returns
   `Ok(None)` immediately without spawning any subprocess.

4. **fetch_status() — subprocess** — Spawns `gh pr list --state open --limit
   100 --json number,title,state,headRefName,reviewDecision,url` with
   `current_dir` set to `workspace_root`.

5. **fetch_status() — subprocess failure** — If the `gh` process exits with a
   non-zero status, returns `Err` containing the trimmed stderr text.

6. **fetch_status() — JSON parsing** — Deserialises the stdout JSON array via
   `serde_json`. Returns `Err` on malformed JSON or non-UTF-8 bytes.

7. **State mapping** — `"MERGED"` → `PrState::Merged`; `"CLOSED"` →
   `PrState::Closed`; any other value (including `"OPEN"`) → `PrState::Open`.

8. **Review-decision mapping** — `"APPROVED"` → `ReviewStatus::Approved`;
   `"CHANGES_REQUESTED"` → `ReviewStatus::ChangesRequested`;
   `"REVIEW_REQUIRED"` → `ReviewStatus::ReviewRequired`; `null`, absent
   field, empty string, or any other value → `ReviewStatus::Unknown`.

9. **Empty list** — An empty JSON array `[]` parses successfully and returns
   `Ok(Some(vec![]))`.

## Constraints

- The `limit` argument is hard-coded to 100; `fetch_status()` never returns
  more than 100 `PrInfo` entries per call.
- `review_decision` is optional in the JSON (`#[serde(default)]` implied by
  `Option`); a missing key and a `null` value are both treated identically as
  `ReviewStatus::Unknown`.
- The `url` field defaults to an empty string when absent from the JSON
  payload (`#[serde(default)]`).
- No retry or timeout logic is applied to the subprocess.

## Dependencies

- **Runtime** — `gh` CLI binary must be present in `PATH`; absence is not an
  error but disables all functionality (`forge_kind()` → `None`,
  `fetch_status()` → `Ok(None)`).
- **Crate** — `anyhow` for error propagation; `serde` + `serde_json` for JSON
  deserialisation; `std::process::Command` for subprocess execution.
- **Trait** — `crates/lajjzy-core/src/forge.rs` defines `ForgeBackend`,
  `ForgeKind`, `PrInfo`, `PrState`, and `ReviewStatus`.
