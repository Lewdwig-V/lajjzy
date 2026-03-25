---
managed_file: crates/lajjzy-core/src/forge.rs
version: 1
test_policy: "No tests — trait definition only, tested via implementors"
---

# ForgeBackend trait and PR types

## Purpose

Define the forge integration abstraction. Forge operations (PR status, reviews)
use different CLI tools than repo operations (gh vs jj), so they live in a
separate trait from `RepoBackend`.

## Dependencies

- `anyhow::Result` — all trait methods are fallible

## Types

### ForgeKind

```
pub enum ForgeKind { GitHub }
```

Derives: `Debug, Clone, Copy, PartialEq, Eq`

Currently GitHub-only. Gerrit and GitLab are out of scope.

### PrInfo

```
pub struct PrInfo {
    pub number: u32,
    pub title: String,
    pub state: PrState,
    pub review: ReviewStatus,
    pub head_ref: String,
    pub url: String,
}
```

Derives: `Debug, Clone, PartialEq`

### PrState

```
pub enum PrState { Open, Merged, Closed }
```

Derives: `Debug, Clone, Copy, PartialEq, Eq`

### ReviewStatus

```
pub enum ReviewStatus { Approved, ChangesRequested, ReviewRequired, Unknown }
```

Derives: `Debug, Clone, Copy, PartialEq, Eq`

## Trait

```
pub trait ForgeBackend: Send + Sync
```

| Method | Signature | Notes |
|--------|-----------|-------|
| `forge_kind` | `(&self) -> Option<ForgeKind>` | `None` when no forge CLI is available |
| `fetch_status` | `(&self) -> Result<Option<Vec<PrInfo>>>` | `Ok(None)` when no forge CLI is available |

## Doc comment

The trait-level doc comment must note: separate from `RepoBackend` because
forge operations use different CLI tools.
