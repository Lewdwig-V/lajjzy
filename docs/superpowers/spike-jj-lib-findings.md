# jj-lib API Spike Findings

## Dependency Spec

```toml
[dependencies]
jj-lib = { version = "0.39.0", features = ["git"] }
```

Crate is `jj-lib` on crates.io (formerly `jujutsu-lib` pre-0.8). The `git` feature is needed for git-colocated repos. Requires Rust 1.89+ (edition 2024).

## Conflict Types

- **`MergedTreeValue`** = `Merge<Option<TreeValue>>` — represents a potentially conflicted tree entry
- **`MaterializedFileConflictValue`** — contains `contents: Merge<BString>` with actual byte content for each side
- **`Merge<T>`** — core data structure storing alternating adds/removes as `SmallVec<[T; 1]>`

Access path:
```
Commit → .tree() → MergedTree → .path_value(path) → MergedTreeValue
```

Content extraction:
```
materialize_tree_value(store, path, value, labels) → MaterializedFileConflictValue
  .contents: Merge<BString>  // byte content per side
```

Or lower-level:
```
extract_as_single_hunk(merge, store, path) → Merge<BString>
```

**Both are async** — need `pollster::block_on()` or tokio runtime.

## Side Extraction

For a 2-sided conflict (the common case):

```rust
let contents: Merge<BString> = /* materialized */;
let left  = contents.first();        // add₀ — first add
let base  = contents.get_remove(0);  // remove₀ — first remove
let right = contents.get_add(1);     // add₁ — second add
```

`Merge` methods:
- `.adds()` / `.removes()` — iterators over sides
- `.get_add(i)` / `.get_remove(i)` — indexed access
- `.num_sides()` — number of positive terms (2 for standard 3-way merge)
- `.is_resolved()` — true if no conflict

## Pairing Logic

Internal storage: `[add₀, remove₀, add₁, remove₁, ..., addₙ]` — odd number of values.

For `Merge::from_removes_adds(vec![base], vec![left, right])`:
- Internal: `[left, base, right]`
- `.first()` = left, `.get_remove(0)` = base, `.get_add(1)` = right

No library function for "give me (base, left, right)" — we extract manually via the indexed accessors. This is straightforward.

## N-Way Conflicts

No upper bound. `num_sides()` returns the count of add terms. Standard merge is 2-sided; octopus merges or repeated conflicts can produce 3+. In practice, 2-sided is ~99% of cases.

For M4: check `num_sides() > 2` and return error "Complex conflict — use external merge tool."

## Repo Handle Lifecycle

**Must reopen after CLI mutations.** `ReadonlyRepo` caches index and backend state that becomes stale when `jj` CLI modifies the operation log.

Design: `fn open_lib_repo(&self) -> Result<Arc<ReadonlyRepo>>` that opens fresh each call. NOT `OnceCell` — the cached handle would go stale after any CLI mutation.

Between CLI calls (pure read operations), a single handle is safe. But since our executor interleaves CLI mutations with reads, reopen is the safe pattern.

## Conflict Count in Templates

**No direct template keyword for conflicted file count.** The `conflict` keyword is boolean (does the commit have any conflicts).

### Options for getting count:

**A) Parse `jj resolve --list -r <rev>` output** — each line is a conflicted file. Count lines.
```bash
jj resolve --list -r <rev> 2>/dev/null | wc -l
```

**B) Keep `has_conflict: bool` and derive count from `FileStatus::Conflicted` files in the file list.**
Since we're adding `FileStatus::Conflicted` and parsing `C` from `jj log --summary`, we can count those files in `ChangeDetail.files`. This avoids a separate CLI call.

**C) Use jj-lib to iterate the tree** — too expensive for every change in the graph.

**Recommended: B.** The count is `detail.files.iter().filter(|f| f.status == Conflicted).count()`. No extra CLI call, no template change. Keep `has_conflict: bool` in the template (field 8), derive count from the file list in the widget.

## Design Adjustments from Findings

1. **`OnceCell` → per-call `open_lib_repo()`** — spec said `OnceCell<jj-lib::workspace::Workspace>`, reality requires reopening.
2. **Async API** — add `pollster` dependency for `block_on()` in sync context.
3. **Conflict count** — derive from file list instead of template change. Keep `has_conflict: bool`.
4. **`BString` → `String`** — content comes as `BString` (byte string); convert to `String` for UTF-8 (error on non-UTF-8 = binary conflict).
