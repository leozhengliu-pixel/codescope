# Fixtures and Test Corpus Policy

## Purpose

This document is the canonical clean-room fixture policy for the full-parity roadmap.
It defines which fixture families `sourcebot-rewrite` is allowed to use, where the
current repo-owned fixture sources already live, and which reuse rules later parity
slices must follow before adding new fixture data.

Task `04` is broader than one execution unit, so **task04a** closes only the
policy + source-inventory slice. Later task04 follow-ups should extend the actual
fixture corpus without replacing this document.

## Governing sources

- `docs/plans/2026-04-18-sourcebot-full-parity-roadmap.md`
- `docs/status/roadmap-state.yaml`
- `specs/CLEAN_ROOM_RULES.md`
- `specs/FEATURE_PARITY.md`
- `specs/acceptance/index.md`
- `specs/acceptance/journeys.md`
- `docs/reports/2026-04-18-parity-gap-report.md`
- `specs/repo-git-search-fixture-layout.md`

## Clean-room rules for fixtures

1. **No upstream source or test reuse.** Fixture contents must not be copied from
   upstream source files, upstream tests, private schemas, or hidden prompts.
2. **Allowed raw inputs stay black-box/public.** When a later task needs a new
   corpus, derive it only from the allowed clean-room inputs listed in
   `specs/CLEAN_ROOM_RULES.md`: public docs/contracts, public screenshots/videos,
   public config schemas, black-box behavior, and rewrite-owned acceptance docs.
3. **Prefer repo-owned synthetic data over captured vendor payloads.** A fixture is
   acceptable when this repo authors the fields/values itself to satisfy a public
   contract; a fixture is not acceptable when it is a copied opaque blob from an
   upstream repository or a private production export.
4. **One canonical owner per fixture family.** Before adding new inline JSON or test
   data, reuse the current owner helper/file for that family or deliberately create
   the next canonical fixture home in a follow-up task.
5. **Record provenance in code or doc comments.** New fixture families added after
   task04a should state which public contract / acceptance spec / repo-owned model
   they are grounded in.

## Current canonical fixture/source inventory

| Fixture family | Current canonical source(s) | Current shape today | Clean-room status | Reuse rule for later slices |
| --- | --- | --- | --- | --- |
| Catalog seed data for repo and connection metadata | `crates/models/src/lib.rs` (`seed_connections()`, `seed_repositories()`); consumed by `crates/api/src/storage.rs` `InMemoryCatalogStore::seeded()` | Repo-owned static Rust structs for local/default repository + connection inventory | Allowed: fully rewrite-authored seed data | Reuse these shared seed helpers for repo/connection metadata before inventing new inline catalog JSON |
| Search corpus for API search behavior | `crates/search/src/lib.rs` (`LocalSearchStore::seeded()`, test-only `create_test_store()`) | Mix of seeded live-workspace repo mapping plus synthetic temp-dir files for focused search tests | Allowed when sourced from local rewrite repo or rewrite-authored temp files | Later parity search/index tasks should create a dedicated corpus home only when the current seeded/temp corpus stops expressing the needed behavior |
| Browse / blob / glob / grep repo trees | `crates/api/src/browse.rs` (`LocalBrowseStore::seeded()`, test-only `create_test_store()`) | Rewrite-owned temp directories, files, and symlink edge cases generated in tests | Allowed: generated entirely inside this repo/test runtime | Prefer helper-driven temp tree generation over copied fixture directories; add new edge cases to the shared browse test helpers first |
| Commit history / diff corpus | `crates/api/src/commits.rs` (`LocalCommitStore::seeded()`, test-only git repo builders around `git init`) | Seeded commit access against the local rewrite repo plus synthetic temp git repositories for diff/history tests | Allowed when derived from the rewrite repo or generated in temporary repos during tests | Later commit-parity work should extend the temp git repo builders instead of checking in copied upstream repos |
| Auth/bootstrap/session state fixtures | `crates/api/src/auth.rs` file-store tests; `crates/api/src/main.rs` helpers like `write_organization_state_fixture(...)` and local-session/bootstrap path helpers | Rewrite-owned JSON state written to temp files, plus shared model instances for accounts/orgs/api keys/OAuth clients | Allowed: rewrite-authored persisted state shapes grounded in repo-owned models and acceptance contracts | Reuse the shared state-writing helpers for org/auth/API-key/OAuth/review state before adding new hand-written JSON blobs |
| Organization / permission / review-automation state fixtures | `crates/api/src/main.rs` test helpers; `crates/models/src/lib.rs` `OrganizationState` and related models | Rich synthetic `OrganizationState` values persisted through temp file paths for API/auth/review tests | Allowed: data is authored in-repo from shared models, not copied from upstream | Later parity slices should centralize repeated state builders rather than scattering more ad hoc per-test structs |
| Worker runtime smoke-state corpus | `crates/worker/tests/no_queued_review_agent_run_idle_smoke.rs`; `crates/worker/tests/explicit_completed_stub_outcome_smoke.rs`; `crates/worker/tests/invalid_stub_outcome_smoke.rs` | Temp file-backed `OrganizationState` fixtures exercised through the real `sourcebot-worker` binary via env-configured paths | Allowed: generated from rewrite-owned models and executed through rewrite-owned binaries | Keep worker smoke fixtures file-backed and real-binary-driven; do not replace them with captured external worker traces |
| Ask/provider mock behavior | `crates/core/src/lib.rs` (`LlmProviderConfig::stub`, `StubLlmProvider`); ask tests in `crates/api/src/main.rs` | Rewrite-owned stub provider returns deterministic synthetic answers for visible repo scopes | Allowed: fully synthetic provider stub, no vendor transcript reuse | Future provider parity should add owned stub/fake providers per contract, not checked-in vendor responses or hidden prompts |
| Webhook event request/response shapes | `crates/api/src/main.rs` request/response structs and review-webhook tests; authenticated review state fixtures in `crates/api/src/auth.rs` | Inline rewrite-authored request bodies plus persisted delivery-attempt/review-run state fixtures | Allowed when fields are derived from public webhook/API contracts and rewritten into repo-owned test data | Later task04 slices should extract any repeated webhook event payload family into a shared fixture helper/file rather than duplicating inline JSON across tests |
| Frontend API response fixtures | `web/src/App.test.tsx` via `vi.spyOn(globalThis, 'fetch')` and inline `jsonResponse(...)` payloads | In-test rewrite-authored JSON responses for repo list/detail, browse, commits, definitions/references, and search flows | Allowed: synthetic UI-facing API payloads authored in the repo | Reuse shared response helpers where possible; if the frontend grows beyond inline fetch mocks, introduce a dedicated fixture module rather than copying payload literals between tests |

## Fixture-family policy by parity area

### 1. Repository / git / search corpora
- Prefer **generated** repositories, temp directories, and temp git histories over
  checked-in repo snapshots.
- It is acceptable to use the local `sourcebot-rewrite` repository itself as seeded
  read-only corpus where tests already do so today.
- If a later parity slice needs multiple stable repos/languages/branches, create a
  repo-owned synthetic corpus under a dedicated fixture home instead of importing an
  upstream fixture repository.

### 2. Auth / org / permission state
- Treat `OrganizationState`, bootstrap state, and local-session state as the
  canonical persisted fixture boundary for auth/admin/worker/review tests.
- Prefer helper-written non-empty state fixtures over raw string literals so later
  schema changes remain centralized.
- When a new persisted field is introduced, extend the shared state helpers and add
  at least one non-empty round-trip assertion rather than updating only compile-time
  fixtures.

### 3. Webhook payloads and review automation
- Public webhook contracts may be represented with rewrite-authored payloads that
  mirror public field names and fail-closed behavior.
- Do **not** paste vendor examples wholesale if they contain copied text or opaque
  unused fields that are not needed for the acceptance contract.
- If multiple tests begin sharing the same event family, promote them into a shared
  fixture helper or dedicated fixture file in a later task04 slice.

### 4. Provider mocks and stubs
- Provider mocks must stay **behavioral and synthetic**.
- Acceptable examples: stub LLM provider responses, fake connection metadata,
  deterministic fetch mocks, synthetic OAuth/client records, synthetic worker
  outcomes.
- Unacceptable examples: copied proprietary prompts, captured provider responses,
  exported production secrets, or replay files taken from private systems.

### 5. Frontend parity fixtures
- Current frontend tests are allowed to stay inline because the UI surface is still
  relatively small and single-shell.
- Once multiple screens reuse the same response families, move them into dedicated
  fixture builders/modules so API contracts stay consistent across tests.
- Frontend fixtures should describe user-visible states, not implementation-only
  internals.

## Canonical fixture-home rules for future task04 slices

Until later task04 follow-ups establish dedicated fixture directories, use these
owners as the canonical homes:

- **Shared domain/catalog seeds:** `crates/models/src/lib.rs`
- **Catalog store wiring:** `crates/api/src/storage.rs`
- **Browse temp corpora:** `crates/api/src/browse.rs`
- **Commit/git temp corpora:** `crates/api/src/commits.rs`
- **Search temp corpora:** `crates/search/src/lib.rs`
- **Auth/org/review persisted state helpers:** `crates/api/src/main.rs` and `crates/api/src/auth.rs`
- **Worker smoke corpora:** worker integration smoke tests under `crates/worker/tests/`
- **Frontend API response mocks:** `web/src/App.test.tsx`
- **Provider stub behavior:** `crates/core/src/lib.rs`

A later task may replace one of these with a better dedicated fixture module or a
repo-owned tests/fixtures/ home, but only by updating this policy/inventory first.

## What task04a intentionally does not claim

Task04a is **policy and inventory only**. It does not claim that fixture reuse is
fully centralized today, nor that every future parity family already has an ideal
fixture module. The current repo still has ad hoc inline payloads and helper-local
state builders. That is acceptable for now because this slice only establishes the
clean-room rules and the current canonical source inventory those later cleanup
slices must build from.

## Recommended next task04 decomposition

- `task04b1` — define the canonical repository / git / search corpus layout and current builder ownership
- `task04b2` — extract shared repo / git / search fixture builders while preserving the canonical layout contract
- `task04c` — centralize auth/org/review state fixture builders
- `task04d` — extract shared webhook payload/event fixture helpers
- `task04e` — centralize frontend/provider mock fixtures and contract builders
