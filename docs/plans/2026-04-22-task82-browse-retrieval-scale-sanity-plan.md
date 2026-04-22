# Task 82 Browse Retrieval Scale Sanity Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Keep local browse/MCP retrieval usable across multiple repositories by preventing `glob`/`grep` from recursively traversing obvious build-artifact directories during repository scans.

**Architecture:** Reuse the existing local browse store and add one shared ignored-directory policy for recursive `glob` and `grep` walks. Prove the change with focused RED→GREEN tests in `crates/api/src/browse.rs`, then close the slice with one truthful acceptance/report update that explains the new practical scale guardrail without overclaiming broader indexing/performance parity.

**Tech Stack:** Rust, Axum crate-local tests, shared repo-tree fixtures, markdown acceptance/report docs.

---

### Task 1: Add failing browse-scale regressions

**Objective:** Lock the intended scale behavior before implementation: recursive browse `glob`/`grep` should skip `.git`, `target`, `node_modules`, and `dist` instead of surfacing build-artifact hits.

**Files:**
- Modify: `crates/api/src/browse.rs`
- Check reference: `crates/test_support/repo_tree_fixture.rs`

**Step 1: Write failing tests**

Add two focused tests near the existing `glob_paths(...)` / `grep(...)` coverage:

```rust
#[test]
fn glob_paths_skips_common_build_artifact_directories() {
    let store = create_test_store_with_common_ignored_dirs();

    let glob = store
        .glob_paths("repo_test", "**/*.rs")
        .unwrap()
        .unwrap();

    assert!(glob.paths.iter().any(|path| path == "src/main.rs"));
    assert!(glob.paths.iter().all(|path| !path.starts_with("target/")));
    assert!(glob.paths.iter().all(|path| !path.starts_with("node_modules/")));
    assert!(glob.paths.iter().all(|path| !path.starts_with("dist/")));
    assert!(glob.paths.iter().all(|path| !path.starts_with(".git/")));
}

#[test]
fn grep_skips_common_build_artifact_directories() {
    let store = create_test_store_with_common_ignored_dirs();

    let grep = store
        .grep("repo_test", "shared_scale_marker")
        .unwrap()
        .unwrap();

    assert!(grep.matches.iter().any(|entry| entry.path == "src/main.rs"));
    assert!(grep.matches.iter().all(|entry| !entry.path.starts_with("target/")));
    assert!(grep.matches.iter().all(|entry| !entry.path.starts_with("node_modules/")));
    assert!(grep.matches.iter().all(|entry| !entry.path.starts_with("dist/")));
    assert!(grep.matches.iter().all(|entry| !entry.path.starts_with(".git/")));
}
```

Use a small helper fixture that creates one visible source hit plus duplicate marker files under ignored directories.

**Step 2: Run tests to verify RED**

Run:

```bash
cargo test -p sourcebot-api glob_paths_skips_common_build_artifact_directories grep_skips_common_build_artifact_directories -- --nocapture
```

Expected: FAIL because current recursive browse traversal still walks ignored directories and returns those artifact hits.

**Step 3: Commit nothing yet**

Do not add implementation before the RED failure is observed.

---

### Task 2: Implement the minimal browse recursion guard

**Objective:** Add one shared ignored-directory helper and use it in recursive browse `glob`/`grep` walkers.

**Files:**
- Modify: `crates/api/src/browse.rs`

**Step 1: Add the minimal implementation**

Introduce one shared directory-skip constant/helper near the existing local browse helpers:

```rust
const SKIPPED_DIR_NAMES: &[&str] = &[".git", "target", "node_modules", "dist"];

fn should_skip_directory(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| SKIPPED_DIR_NAMES.iter().any(|skipped| name == *skipped))
}
```

Then gate the recursive walkers before descending:

```rust
if file_type.is_dir() {
    if should_skip_directory(&path) {
        continue;
    }

    self.collect_glob_matches(root, &path, matcher, matches)?;
    continue;
}
```

Apply the same guard in `collect_grep_matches(...)`.

**Step 2: Run the targeted tests to verify GREEN**

Run:

```bash
cargo test -p sourcebot-api glob_paths_skips_common_build_artifact_directories grep_skips_common_build_artifact_directories -- --nocapture
```

Expected: PASS.

**Step 3: Run a broader crate confidence pass**

Run:

```bash
cargo test -p sourcebot-api browse:: -- --nocapture
```

If the selector is too broad or invalid, fall back to the full crate test command used in prior roadmap slices:

```bash
cargo test -p sourcebot-api
```

Expected: PASS with no browse regressions.

---

### Task 3: Add one representative multi-repo scale sanity smoke and close docs

**Objective:** Prove the guardrail in a representative multi-repo scan and update the docs/report truthfully in the same slice.

**Files:**
- Modify: `crates/api/src/browse.rs`
- Modify: `specs/acceptance/integrations.md`
- Modify: `specs/repo-git-search-fixture-layout.md`
- Modify: `docs/reports/2026-04-18-parity-gap-report.md`

**Step 1: Add one multi-repo smoke-style test**

Add a focused test that creates two repo roots, seeds the same marker in visible source files plus ignored directories, then exercises the browse adapters across both repos:

```rust
#[tokio::test]
async fn browse_glob_and_grep_scale_sanity_skip_ignored_dirs_across_multiple_repositories() {
    // create repo_alpha and repo_beta temp roots
    // put shared marker in src/main.rs for both repos
    // put duplicate marker files under target/, node_modules/, dist/, and .git/
    // wire LocalBrowseStore::new(HashMap::from([...]))
    // assert each repo still returns the visible source hit
    // assert no ignored-directory path appears in glob or grep results
}
```

This is the representative Task 82 evidence that the local retrieval layer stays practical when more than one repo contains common build artifacts.

**Step 2: Verify RED/GREEN for the smoke**

Run just the new smoke after adding it, watch it fail if written before the helper is fully wired, then rerun after the implementation until it passes.

Recommended command:

```bash
cargo test -p sourcebot-api browse_glob_and_grep_scale_sanity_skip_ignored_dirs_across_multiple_repositories -- --nocapture
```

**Step 3: Update the docs truthfully**

Patch the docs to say exactly what is now true:
- `specs/acceptance/integrations.md`: the current local retrieval/MCP baseline skips obvious build-artifact directories during repo-scoped file discovery.
- `specs/repo-git-search-fixture-layout.md`: browse corpus expectations now distinguish visible source files from ignored artifact directories for recursive `glob`/`grep` behavior.
- `docs/reports/2026-04-18-parity-gap-report.md`: record Task 82 as a practical scale guardrail, not full indexing/performance parity.

**Step 4: Run doc-truth verification**

Run exact raw-content checks, for example:

```bash
python3 - <<'PY'
from pathlib import Path
checks = {
    'specs/acceptance/integrations.md': 'skips obvious build-artifact directories',
    'specs/repo-git-search-fixture-layout.md': 'recursive `glob`/`grep` behavior',
    'docs/reports/2026-04-18-parity-gap-report.md': 'practical scale guardrail',
}
for path, needle in checks.items():
    text = Path(path).read_text()
    assert needle in text, f"missing {needle!r} in {path}"
PY
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/api/src/browse.rs \
        specs/acceptance/integrations.md \
        specs/repo-git-search-fixture-layout.md \
        docs/reports/2026-04-18-parity-gap-report.md \
        docs/plans/2026-04-22-task82-browse-retrieval-scale-sanity-plan.md
git commit -m "feat: skip build-artifact dirs in browse retrieval scans"
```

---

### Task 4: Pre-commit verification and roadmap close-out

**Objective:** Finish the slice honestly with verification, review, state update, and push.

**Files:**
- Modify: `docs/status/roadmap-state.yaml`

**Step 1: Run the pre-commit verification pipeline**

Run:

```bash
git diff --cached --check
python3 <added-lines security scan over staged diff>
cargo test -p sourcebot-api browse_glob_and_grep_scale_sanity_skip_ignored_dirs_across_multiple_repositories -- --nocapture
cargo test -p sourcebot-api
```

Expected: PASS.

**Step 2: Request independent review**

Dispatch:
- spec review against this plan and the intended Task 82 scope
- code review against the staged diff

Expected: PASS / APPROVED with no blocking issues.

**Step 3: Update roadmap state**

Record:
- `last_completed_task`: the real Task 82 slice you shipped
- `current_task`: Task 83 security hardening pass
- history/tests/review/commit entries grounded in the actual commands and reviewers

**Step 4: Final close-out**

Run:

```bash
git status --short
git push
```

Expected: clean/truthful worktree and pushed commits.
