# Sourcebot Full-Parity Implementation Roadmap

> **For Hermes:** Use `stateful-roadmap-autopilot`, `writing-plans`, `test-driven-development`, `requesting-code-review`, and `subagent-driven-development` to execute this roadmap one smallest-deliverable task at a time.

**Goal:** Turn CodeScope / `sourcebot-rewrite` from a completed clean-room MVP+ roadmap into a product that is functionally interchangeable with upstream Sourcebot across backend, worker, integrations, auth, admin, and frontend UX.

**Architecture:** Keep the clean-room Rust + React architecture, but finish all remaining parity layers intentionally deferred in the first roadmap: durable metadata persistence, real provider integrations, true indexing/search pipelines, rich frontend surfaces, background orchestration, and production-grade operational behavior. The project remains clean-room and may differ internally, but every user-visible and operator-visible behavior required for practical Sourcebot replacement must be implemented, verified, and documented.

**Tech Stack:** Rust, Axum, Tokio, SQLx, PostgreSQL, React, TypeScript, Vite, Tantivy, tree-sitter, git CLI/libgit2-compatible plumbing where appropriate, object storage, background workers, webhook processing, OAuth/OIDC, and test/smoke parity fixtures.

---

## Definition of 100% parity for this roadmap

This roadmap is complete only when all of the following are true:

1. `specs/FEATURE_PARITY.md` is fully checked off and matches shipped behavior.
2. Rewrite exposes equivalent practical capability for all major upstream user journeys:
   - repository onboarding and sync
   - search, browse, commit diff, code navigation
   - ask/chat with citations and thread lifecycle
   - organizations, members, invites, linked accounts, API keys
   - connection management, sync/index status, permissions/access
   - analytics, audit, OAuth client/token flows
   - review agent and webhook automation
3. Rewrite supports the key upstream operator workflows:
   - persistent metadata DB
   - background workers / retry / scheduling / visibility
   - repo sync/index refresh and failure recovery
   - configuration, observability, and upgrade-safe local deployment
4. Frontend parity is real, not API-only: all major upstream UI surfaces have rewrite equivalents.
5. Integrations required by `specs/FEATURE_PARITY.md` exist with working end-to-end coverage.
6. A final parity audit document maps every upstream feature area to implemented rewrite evidence.

---

## Execution rules for autopilot

- `docs/status/roadmap-state.yaml` remains the single source of truth.
- Execute exactly one smallest deliverable task per execution unit.
- Every completed task must end with: implementation, tests, docs, one review, commit, state update, and push.
- If a task is too large, split it immediately and finish only the first concrete slice.
- Prefer vertical slices that end in observable product behavior or locked contract coverage.
- If blocked on external credentials or unavailable infrastructure, record the blocker in state and continue with the next unblocked parity slice.

---

## Phase 0 — Parity contract freeze and audit baseline

### Task 1: Upstream feature inventory and acceptance baseline
Create a clean-room acceptance inventory that maps upstream feature surfaces, routes, screens, worker behaviors, and operator workflows into rewrite-owned black-box specs.

### Task 2: Parity matrix expansion
Expand `specs/FEATURE_PARITY.md` from bullet points into a full checklist with per-feature acceptance evidence placeholders.

### Task 3: Gap report by domain
Write a repo-local parity gap report covering backend, worker, integrations, frontend, auth/admin, and ops, with explicit “missing / partial / complete” status.

### Task 4: Fixtures and test corpus policy
Define the clean-room fixture strategy for repos, git history, search indexes, auth/org states, webhook payloads, and provider mocks so later parity tasks can reuse stable fixtures.

---

## Phase 1 — Durable metadata and state foundation

### Task 5: PostgreSQL schema bootstrap
Introduce the first SQLx-backed metadata schema for repositories, organizations, members, sessions, ask threads, review-agent runs, and delivery attempts.

### Task 6: Migration and local-dev workflow
Add migration commands, local bootstrapping docs, and deterministic dev/test DB setup.

### Task 7: Catalog store parity migration
Replace the seeded/in-memory catalog path with a real Postgres-backed catalog implementation while preserving existing API behavior.

### Task 8: Ask thread durable store
Move ask thread and message persistence from in-memory to durable SQL-backed storage.

### Task 9: Auth/session durable store
Move bootstrap/admin/session state from file-backed dev storage toward durable metadata storage while keeping a local-dev fallback strategy explicit.

### Task 10: Organization aggregate durable store
Persist org/member/invite/account/repo-permission state in durable storage instead of whole-document JSON files.

### Task 11: Review-agent durable store hardening
Move review-agent runs and delivery attempts to durable relational storage with idempotent writes and concurrency-safe state transitions.

### Task 12: Backward-compat and migration smoke coverage
Add migration / boot compatibility coverage proving rewrite can start with empty state and can safely migrate from dev-friendly local storage where supported.

---

## Phase 2 — Repository ingestion and connection management parity

### Task 13: Connection domain model
Add durable connection models for GitHub, GitLab, Gitea, Gerrit, Bitbucket, Azure DevOps, and generic/local Git.

### Task 14: Connection CRUD APIs
Implement authenticated APIs for creating, listing, updating, and deleting repository connections.

### Task 15: Connection settings UI
Build settings UI for viewing and editing configured connections.

### Task 16: Repository sync job model
Add durable sync-job records with status, timestamps, error surfaces, and operator-readable history.

### Task 17: Mirror/fetch worker parity baseline
Implement a real repo fetch/mirror worker path that updates branch/revision metadata instead of relying only on local preseeded repos.

### Task 18: Repository status API parity
Expose repo sync/index status endpoints equivalent to upstream operator visibility.

### Task 19: Repository status UI
Show sync/index health and last-run information in repo and settings surfaces.

### Task 20: Generic/local Git connection parity
Finish the generic/local Git host path so self-hosted local mirrors work as a first-class supported mode.

---

## Phase 3 — Search and indexing parity

### Task 21: Real indexing pipeline bootstrap
Replace the minimal filesystem text search path with a real incremental indexing pipeline.

### Task 22: Tantivy document schema and snippets
Define indexed search documents, snippet extraction, and hit rendering parity.

### Task 23: Multi-repo indexing orchestration
Support indexing across many repos and branches instead of the current single-seeded-repo bias.

### Task 24: Search filter grammar parity
Implement literal / regex / boolean / repo / language / path filters with stable request validation.

### Task 25: Cross-repo and cross-branch search parity
Allow search across multiple repos and revisions with equivalent result semantics.

### Task 26: Streaming search API parity
Add a stream-search endpoint and backend execution path equivalent to upstream’s richer search flow.

### Task 27: Search relevance and pagination hardening
Lock result ordering, pagination, snippet quality, and error handling with focused tests.

### Task 28: Search contexts backend parity
Persist reusable search scopes / contexts with owner and org scoping rules.

### Task 29: Search contexts UI parity
Build UI for creating, editing, selecting, and applying saved search contexts.

---

## Phase 4 — Browse, source view, commits, and code navigation parity

### Task 30: Syntax-highlighted source rendering parity
Upgrade file source rendering to production-grade syntax highlighting and large-file handling.

### Task 31: Rich tree navigation parity
Finish file explorer parity for nested tree interactions, breadcrumbs, and route-stable navigation.

### Task 32: Branch and revision selection parity
Allow browse/search/commit/source views to target non-default branches and specific revisions.

### Task 33: Commit API parity expansion
Close remaining commit history/detail/diff contract gaps with upstream behavior.

### Task 34: Commit UI parity expansion
Build the missing commit detail and diff UX polish, navigation, and state management.

### Task 35: Symbol extraction multi-language baseline
Extend code navigation beyond the current minimal Rust-only extraction path.

### Task 36: Definitions parity hardening
Close correctness gaps for symbol definition lookup across supported languages and revisions.

### Task 37: References parity hardening
Close correctness gaps for reference lookup across supported languages and revisions.

### Task 38: Code-navigation UI parity
Add richer navigation UX: clickable symbols, stable highlighting, line targeting, and result jumps.

---

## Phase 5 — Ask, chat, citations, and MCP parity

### Task 39: Real retrieval tool backend parity
Finish list-tree / read-file / grep / glob / repo-list tools against durable multi-repo context and auth constraints.

### Task 40: Citation contract parity
Lock citation model, snippet rendering, source anchors, and source-open interactions to parity behavior.

### Task 41: Ask completions durability and thread lifecycle
Finish durable thread history, thread resume, thread visibility, and thread metadata lifecycle.

### Task 42: Ask/chat API parity
Add the missing ask/chat endpoints and request/response semantics required for practical upstream interchangeability.

### Task 43: Chat thread management UI parity
Build thread list/history, rename, delete, and visibility controls in the frontend.

### Task 44: Ask experience UI parity
Build richer ask UX with citations, source previews, repo scope controls, and loading/error behavior.

### Task 45: Model/provider management parity
Expose model listing / configuration surfaces needed by frontend and operators.

### Task 46: MCP server parity
Implement and verify the MCP server capability expected by the parity matrix.

---

## Phase 6 — Auth, onboarding, orgs, and admin parity

### Task 47: First-run onboarding parity
Implement first-run bootstrap/onboarding UX and API behavior.

### Task 48: Local auth UX parity
Build login, logout, verification, and session restoration frontend flows.

### Task 49: Organization and membership domain parity
Finish org/member/role state so it supports all required invite and access workflows.

### Task 50: Invite flow parity
Implement invite creation, redemption, and membership acceptance UX + API flows.

### Task 51: Linked account domain parity
Add external-account linking models and durable state.

### Task 52: Linked account UX parity
Build linked-account management UI and supporting APIs.

### Task 53: API key parity
Implement full API key lifecycle backend and settings UI.

### Task 54: Access / permission sync parity
Implement durable repo-permission sync behavior and fail-closed enforcement across search, browse, ask, review-agent, and admin surfaces.

### Task 55: Admin/settings navigation parity
Build the missing settings navigation and section shells so admin/user flows are discoverable and coherent.

---

## Phase 7 — Provider and identity integration parity

### Task 56: GitHub integration parity
Finish repository connection, auth, sync, and webhook support for GitHub.

### Task 57: GitLab integration parity
Finish repository connection, auth, and sync support for GitLab.

### Task 58: Gitea and Gerrit integration parity
Finish repository connection and sync support for Gitea and Gerrit.

### Task 59: Bitbucket and Azure DevOps integration parity
Finish repository connection and sync support for Bitbucket and Azure DevOps.

### Task 60: OIDC / SSO provider parity
Implement identity-provider integration support required by the parity matrix.

### Task 61: OAuth client / token flow parity
Finish OAuth client registration, authorization, token issuance, and revocation behavior.

---

## Phase 8 — Review agent and webhook automation parity

### Task 62: Webhook verification and routing hardening
Finish provider webhook validation, event normalization, idempotency, and operator-visible failure modes.

### Task 63: Review-agent queue orchestration parity
Replace the current one-shot stub worker flow with real queued orchestration, retries, scheduling, and visibility.

### Task 64: Review-agent execution contract
Implement the real review-agent execution boundary instead of stub-only completion/failure behavior.

### Task 65: Delivery attempt retry and backoff parity
Add durable retry policy, attempt history, terminal failure handling, and operator visibility.

### Task 66: Review-agent API parity completion
Finish all remaining authenticated and global run/attempt management surfaces.

### Task 67: Review-agent frontend parity
Build agents / review-agent UI, run history, attempt visibility, and actionable operator feedback.

---

## Phase 9 — Analytics, audit, and operator observability parity

### Task 68: Audit event persistence parity
Persist and expose audit events with equivalent operator usefulness.

### Task 69: Audit UI / API parity
Build audit list/detail surfaces and any required filters/export behavior.

### Task 70: Analytics backend parity
Implement the analytics aggregation and API layer required by parity goals.

### Task 71: Analytics UI parity
Build analytics/settings UI for the supported metrics.

### Task 72: Background job observability parity
Expose worker/job health, last-run, retry, queue-depth, and failure surfaces for operators.

### Task 73: Config and runtime diagnostics parity
Add version/source/models/runtime diagnostics endpoints and operator-friendly status views.

---

## Phase 10 — Frontend route and product-shell parity

### Task 74: Route inventory parity shell
Create the missing route skeletons so all upstream-equivalent product areas exist in the rewrite router structure.

### Task 75: Search page parity shell
Split home-page search into a dedicated search route with richer UX composition.

### Task 76: Repos and browse UX polish parity
Finish repo list/detail/browse UX state persistence, empty states, and navigation polish.

### Task 77: Settings section parity completion
Complete members, connections, API keys, access, analytics, linked accounts, and license/settings screens.

### Task 78: Auth and onboarding route parity completion
Complete login, signup/onboarding, invite/redeem, and OAuth-related screens.

### Task 79: Chat / agents route parity completion
Complete dedicated chat, ask, and agents pages with consistent navigation and state restoration.

---

## Phase 11 — Production hardening and release parity

### Task 80: Deployment and config parity
Close the gap between dev-only setup and production-ready deployment/configuration behavior.

### Task 81: Backup / restore / migration operator docs
Document and test metadata backup, restore, migration, and upgrade workflows.

### Task 82: Performance and scale sanity pass
Run representative multi-repo scale checks and fix obvious bottlenecks that would block practical replacement usage.

### Task 83: Security hardening pass
Review secret handling, auth/session security, webhook validation, token storage, and permission boundaries.

### Task 84: End-to-end smoke matrix
Add full-stack smoke scenarios that cover the critical user/operator journeys across integrations, search, ask, auth, and review-agent.

### Task 85: Final parity audit and release checklist
Produce the final audit document mapping every item in `specs/FEATURE_PARITY.md` to code/tests/docs, and only then declare full parity complete.

---

## Recommended first execution slices after bootstrap

1. Split **Task 1** into a first smallest slice that inventories upstream route/page/worker surfaces into a rewrite-owned acceptance index.
2. Then split **Task 2** into one capability area at a time, starting with core search/browse/code-nav/checklist rows.
3. Do not begin broad implementation tasks in later phases until the parity acceptance inventory and gap report exist; otherwise the autopilot will drift into local optimizations instead of parity closure.

---

## Completion gate

This roadmap is not done when the document says so; it is done only when:
- `specs/FEATURE_PARITY.md` has no unchecked items,
- the final parity audit proves coverage area by area,
- the remaining deferred notes about stub behavior / missing UI / missing retries / missing provider integrations are gone,
- and rewrite can honestly be described as a practical Sourcebot replacement rather than a narrowed clean-room subset.
