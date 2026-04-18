# Repository / Git / Search Fixture Layout

## Purpose

This document records the canonical clean-room layout for the repository, git-history,
and search-index fixture families used in `sourcebot-rewrite` through **task04b2b**
of the full-parity roadmap.

It now freezes:

- which live fixture builders own each corpus family after task04b2b,
- which shared paths/IDs/content shapes later parity slices must preserve, and
- which follow-up work remains intentionally local or deferred instead of adding new
  inline temp-directory or temp-git setup.

## Governing sources

- `docs/plans/2026-04-18-sourcebot-full-parity-roadmap.md`
- `docs/status/roadmap-state.yaml`
- `specs/fixtures-policy.md`
- `specs/CLEAN_ROOM_RULES.md`
- `specs/FEATURE_PARITY.md`
- `specs/acceptance/index.md`
- `docs/reports/2026-04-18-parity-gap-report.md`
- `crates/search/src/lib.rs`
- `crates/api/src/browse.rs`
- `crates/api/src/commits.rs`
- `crates/models/src/lib.rs`

## Scope boundary through task04b2b

Task04b was too broad for one execution unit, so the roadmap split it into:

- **task04b1** — define the canonical repository / git / search corpus layout and
  current builder ownership
- **task04b2a** — extract the shared canonical repo-tree root builder used by both
  browse and search temp corpora
- **task04b2b** — centralize the remaining search ignored/binary variants and
  browse symlink variants behind opt-in shared helpers

This document now reflects the canonical builder ownership and layout contract after
**task04b2b** while still leaving commit temp-git helpers untouched.

## Canonical fixture families

| Family | Current owner today | Canonical repo id(s) / labels | Canonical on-disk shape today | Why this shape matters |
| --- | --- | --- | --- | --- |
| Search temp corpus | `crates/test_support/repo_tree_fixture.rs` `CanonicalRepoTreeRoot::create(...)` for the shared temp root plus `crates/search/src/lib.rs` test-only `create_test_store()` for search-only extras | `repo_test` | temp root containing `src/main.rs`, `README.md`, `.git/HEAD`, `target/generated.txt`, `image.png`, `binary.dat` | Proves search returns source/doc hits while skipping `.git`, generated output, binary files, and over-size files |
| Browse temp corpus | `crates/test_support/repo_tree_fixture.rs` `CanonicalRepoTreeRoot::create(...)` for the shared temp root plus `crates/api/src/browse.rs` test-only `create_test_store()` for browse-only symlink variants | `repo_test` | temp root containing `README.md`, `src/main.rs`, `target/generated.rs`; some tests add `src/readme-link.rs` symlink | Proves tree/blob/glob/grep operate on a visible repo tree, including symlink and traversal edge cases |
| Commit seeded/special-case catalog corpus | `crates/api/src/commits.rs` `LocalCommitStore::seeded()` plus `crates/models/src/lib.rs` seeded repositories | `repo_sourcebot_rewrite`, `repo_demo_docs` | `repo_sourcebot_rewrite` is mapped to the live rewrite repo root; `repo_demo_docs` is a seeded catalog repo id that currently returns empty history via `EMPTY_HISTORY_REPO_IDS` | Separates real-history coverage from the explicit empty-history docs/demo case already exposed through the seeded catalog |
| Commit synthetic temp-git corpus | `crates/api/src/commits.rs` test helpers `create_temp_git_repo(...)`, `write_text_file(...)`, `git_in(...)` | caller-chosen ids like `repo_temp` | throwaway git repo initialized with `git init`, explicit file writes, and deterministic commits | Proves diff/history edge cases without checking in copied upstream repositories |

## Canonical layout contract by family

### 1. Search corpus contract

The current shared repo-tree root builder in `crates/test_support/repo_tree_fixture.rs`
plus the search-local `create_test_store()` wrapper in `crates/search/src/lib.rs`
are the canonical source for synthetic repo-search corpora after **task04b2b**.
Ignored and binary search-only variants are now added through opt-in shared fixture helpers.

Required layout contract:

- root directory created by `unique_temp_dir()`
- visible source file at `src/main.rs`
- visible documentation file at `README.md`
- ignored VCS file at `.git/HEAD`
- ignored generated/build output at `target/generated.txt`
- binary-ish files such as `image.png` and `binary.dat`
- repo mapping `repo_test -> <temp-root>`

Required behavior contract:

- source hits in `src/main.rs` remain searchable
- documentation hits in `README.md` remain searchable
- `.git/*` contents must not surface in results
- generated/build output under `target/` must not surface in results
- binary and oversize files must be excluded

### 2. Browse corpus contract

The current shared repo-tree root builder in `crates/test_support/repo_tree_fixture.rs`
plus the browse-local `create_test_store()` wrapper in `crates/api/src/browse.rs`
are the canonical source for tree/blob/glob/grep corpora after **task04b2b**.
Browse-only symlink variants are now added through opt-in shared fixture helpers.

Required layout contract:

- root directory created by `unique_temp_dir()`
- top-level `README.md`
- visible source file `src/main.rs`
- visible generated file `target/generated.rs`
- repo mapping `repo_test -> <temp-root>`
- optional in-test symlink `src/readme-link.rs` pointing at `README.md` for
  symlink parity checks

Required behavior contract:

- tree listings expose both `README.md` and `src/`
- glob results may include paths also visible in tree listings, including
  `target/generated.rs`
- blob reads return exact contents for visible files
- parent-directory traversal like `../etc` is rejected before filesystem access
- current grep symlink handling is constrained to the repo root; glob behavior must
  keep following the live browse contract and should only gain stricter root
  enforcement in a later implementation slice that updates this document

### 3. Commit corpus contract

The current commit fixtures intentionally use **two** corpus modes and both are
canonical until task04b2 or later parity work says otherwise.

#### 3a. Seeded real-repo + empty-history mode

Canonical owner: `LocalCommitStore::seeded()` in `crates/api/src/commits.rs`.

Required contract:

- seeded repo id `repo_sourcebot_rewrite` maps to the live
  `sourcebot-rewrite` workspace repo
- seeded repo id `repo_demo_docs` stays available as a seeded catalog entry whose
  commit-store behavior is the explicit empty-history special case in
  `EMPTY_HISTORY_REPO_IDS`
- list/detail assertions against real git history should use `repo_sourcebot_rewrite`
  and may compare against live `git rev-parse` / `git log` output from the rewrite repo
- `repo_demo_docs` is for the empty-history contract, not a second mapped on-disk repo root
- this mode is for real-history parity coverage plus the explicit empty-history behavior,
  not synthetic edge-case shaping

#### 3b. Synthetic temp-git mode

Canonical owners: `create_temp_git_repo(...)`, `write_text_file(...)`, and
`git_in(...)` in `crates/api/src/commits.rs` tests.

Required contract:

- initialize a fresh temp repo with `git init`
- author files through repo-owned helper writes, not checked-in fixture repos
- shape history through explicit commits in the test itself
- use this mode for type-change, rename, patch-normalization, and other focused
  git edge cases

## Shared builder ownership rules after task04b2b

After **task04b2b**, future parity work must follow these rules:

1. **Reuse `crates/test_support/repo_tree_fixture.rs` for the common temp root**
   (`README.md`, `src/main.rs`, `target/*`) before adding another temp-tree helper.
2. **Add search ignored/binary extras through the opt-in shared helper in**
   `crates/test_support/repo_tree_fixture.rs` **unless a later slice deliberately
   changes the canonical search corpus contract.**
3. **Add browse symlink variants through the opt-in shared helper in**
   `crates/test_support/repo_tree_fixture.rs` **unless a later slice deliberately
   changes the canonical browse corpus contract.**
4. **Use the seeded real rewrite repo only for real-history assertions.** Use the
   temp-git builders for synthetic commit edge cases.
5. **Keep repo ids stable** where existing tests already rely on them:
   - `repo_test` for search and browse temp stores
   - `repo_sourcebot_rewrite` / `repo_demo_docs` for seeded catalog/commit paths
6. **Preserve clean-room authorship.** Add new files/content as rewrite-authored
   literals written during tests, not copied fixture directories or upstream repo
   snapshots.

## Recommended follow-up for task04b2

Task04b2 is now split into:

1. **task04b2a** — extract the shared canonical repo-tree root builder used by both
   browse and search temp corpora
2. **task04b2b** — centralize the remaining search ignored/binary variants and
   browse symlink variants behind opt-in shared helpers if they still have more
   than one caller
3. keep the current canonical paths (`README.md`, `src/main.rs`, `target/*`,
   `.git/*`) and repo ids stable, and
4. leave commit temp-git helpers in `crates/api/src/commits.rs` unless a second
   caller actually needs them.

## What task04b1 intentionally does not claim

- It does **not** claim repo/git/search fixture builders are centralized today.
- It does **not** add a `tests/fixtures/` directory yet.
- It does **not** rename the current helper functions.
- It does **not** expand auth/webhook/frontend/provider fixture families; those stay
  for later task04 slices.
