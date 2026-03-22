# jj-lib Write API Spike Findings

## Transaction API

```rust
// Start transaction (repo.rs:331)
let tx: Transaction = repo.start_transaction();  // consumes Arc<ReadonlyRepo>

// Access mutable repo
tx.repo_mut() -> &mut MutableRepo

// Commit (async — need pollster::block_on)
tx.commit("description").await -> Result<Arc<ReadonlyRepo>, TransactionCommitError>
```

Transaction lifecycle: `ReadonlyRepo::start_transaction()` → `Transaction` → mutate via `tx.repo_mut()` → `tx.commit()`.

Note: `start_transaction` takes `&Arc<Self>`, not settings. The `Transaction::new` constructor takes settings, but the repo method handles this internally.

## Absorb

**Available as library function.** Module: `jj_lib::absorb`.

```rust
// Main function
pub async fn absorb_hunks(
    repo: &mut MutableRepo,
    source: &AbsorbSource,
    selected_trees: HashMap<CommitId, MergedTreeBuilder>,
) -> BackendResult<AbsorbStats>

// Setup
pub struct AbsorbSource { commit, parents, parent_tree }
impl AbsorbSource {
    pub async fn from_commit(repo: &dyn Repo, commit: Commit) -> BackendResult<Self>
}

// To get selected_trees (which trees to absorb into):
pub async fn split_hunks_to_trees(
    repo: &dyn Repo,
    source: &AbsorbSource,
    destinations: &Arc<ResolvedRevsetExpression>,
    matcher: &dyn Matcher,
) -> Result<SelectedTrees, AbsorbError>

pub struct SelectedTrees {
    pub target_commits: HashMap<CommitId, MergedTreeBuilder>,
    pub skipped_paths: Vec<(RepoPathBuf, String)>,  // paths that couldn't be absorbed + reason
}

// Return value
pub struct AbsorbStats {
    pub rewritten_source: Option<Commit>,      // None if abandoned or no hunks moved
    pub rewritten_destinations: Vec<Commit>,    // commits that received hunks
    pub num_rebased: usize,                     // descendant commits rebased
}
```

**Flow:**
1. `AbsorbSource::from_commit(repo, commit)`
2. `split_hunks_to_trees(repo, source, destinations_revset, matcher)` → `SelectedTrees`
3. `absorb_hunks(mut_repo, source, selected_trees.target_commits)` → `AbsorbStats`
4. Rebase descendants: `mut_repo.transform_descendants(...)`
5. Status message from `AbsorbStats.rewritten_destinations.len()` + `SelectedTrees.skipped_paths.len()`

**Destination revset:** Need to construct a revset expression for "mutable ancestors of source". In jj-cli this is `mutable()` intersected with `ancestors(source)`.

## Duplicate

**Convenience function available.**

```rust
pub async fn duplicate_commits_onto_parents(
    mut_repo: &mut MutableRepo,
    target_commits: &[CommitId],
    target_descriptions: &HashMap<CommitId, String>,
) -> BackendResult<DuplicateCommitsStats>

pub struct DuplicateCommitsStats {
    pub duplicated_commits: IndexMap<CommitId, Commit>,  // old → new
    pub num_rebased: u32,
}
```

Simple: pass one commit ID, empty descriptions map (keep original), get back the new commit.

## Revert

**NOT a library function.** Implemented as a composition in jj-cli.

The algorithm:
1. For each revision to revert, compute the reverse diff
2. Create a new commit with that reverse diff applied to the target location
3. The core is: merge the target's tree with the reverse of the reverted commit's changes

Key primitives:
```rust
// Create new commit
mut_repo.new_commit(parent_ids, tree) -> CommitBuilder

// Merge trees (for computing reverse)
rewrite::merge_commit_trees(repo, commits) -> MergedTree

// CommitBuilder
builder.set_description(desc).write().await -> Commit
```

For `--onto @` (leaf child): parent_ids = [wc_commit_id], tree = merge(wc_tree, reverse_of_selected).

## Working-Copy Update

Two approaches:

**A) Via MutableRepo (within transaction):**
```rust
mut_repo.check_out(workspace_name, &new_commit).await  // for absorb that rewrites @
mut_repo.edit(workspace_name, &new_commit).await        // to switch working copy
```

**B) Via Workspace (after transaction):**
```rust
workspace.check_out(operation_id, old_tree, &commit).await
```

For absorb: if the source is `@`, absorb rewrites it. Use `mut_repo.check_out()` within the transaction to point the working copy at the rewritten source (or its replacement if abandoned).

For revert: creates a new child of `@`. The working copy doesn't need updating — `@` itself isn't rewritten, just gets a new child.

For duplicate: no working copy impact — the duplicate is on the original's parents, not on `@`.

## Design Adjustments

1. **Absorb is the most complex** — needs revset construction for destinations, `split_hunks_to_trees`, then `absorb_hunks`, then descendant rebasing. ~30 lines of jj-lib orchestration.
2. **Duplicate is trivial** — one function call.
3. **Revert requires manual composition** — compute reverse tree, create commit. ~15 lines.
4. **All async** — every jj-lib call is async. Use `pollster::block_on()` throughout.
5. **Working-copy update** — only needed for absorb when source is `@`. Use `mut_repo.check_out()` within transaction.
6. **`--onto` confirmed as leaf** — creates child without rebasing existing children.
