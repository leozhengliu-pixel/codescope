# Parity gap report

_This document is the canonical repo-local parity gap report for `sourcebot-rewrite`._

This report tracks parity gaps against the 2026-04-18 full-parity roadmap. Task `03` is too broad for a single slice, so this first slice implements **task03a** only: the **backend/API** domain. Later slices should extend this same document with additional domains instead of creating competing gap reports.

## Status legend

- **Missing** — no meaningful rewrite implementation evidence exists yet for the parity-facing capability.
- **Partial** — some rewrite surface or behavior exists, but major parity requirements, acceptance coverage, durability, or operational behavior are still open.
- **Complete** — parity looks meaningfully closed based on current repo evidence and acceptance coverage.

## Backend/API domain

Grounding for this slice comes from the live API router in `crates/api/src/main.rs`, the acceptance inventory in `specs/acceptance/index.md`, the current acceptance specs under `specs/acceptance/`, the parity matrix in `specs/FEATURE_PARITY.md`, and the roadmap phases/tasks in `docs/plans/2026-04-18-sourcebot-full-parity-roadmap.md`.

| Capability area | Current rewrite evidence | Status | Highest-value next gap(s) |
| --- | --- | --- | --- |
| Service health and public config | `crates/api/src/main.rs` exposes `/healthz` and `/api/v1/config`; acceptance index already treats these as the current public/operator-facing baseline. | Partial | Add operator-focused acceptance coverage and production-grade config/observability parity called out by Phase 0 and later ops tasks. |
| Local bootstrap and session auth API baseline | Router exposes `/api/v1/auth/bootstrap`, `/api/v1/auth/login`, `/api/v1/auth/me`, `/api/v1/auth/logout`, and `/api/v1/auth/revoke`; `specs/acceptance/auth.md` covers first-run onboarding and local auth at a high level. | Partial | Close durable session/storage gaps, expand onboarding/auth acceptance detail, and finish broader auth/org/admin parity from roadmap Phase 6. |
| API key and auth-adjacent admin endpoints | Router exposes `/api/v1/auth/api-keys` and revoke, plus `/api/v1/auth/search-contexts`; parity matrix lists API keys and connection management separately, while acceptance auth/search coverage is still broad rather than endpoint-complete. | Partial | Finish full API key lifecycle semantics, durable storage, richer search-context behavior, and endpoint-level acceptance evidence. |
| Repository catalog, tree/blob, commit, and code-navigation APIs | Router exposes `/api/v1/repos`, repo detail, tree, blob, definitions, references, commits, commit detail, and commit diff endpoints; acceptance browse/code-nav specs define the intended black-box behavior. | Partial | Close branch/revision parity, multi-language code-nav correctness, pagination/diff hardening, and richer repo metadata/status behavior from Phases 2–4. |
| Search API | Router exposes `/api/v1/search`; `specs/acceptance/search.md` defines cross-repo, cross-branch, filter, snippet, and pagination expectations; parity matrix still marks search rows as needing audit. | Partial | Replace the current minimal search path with real indexing, richer filter grammar, cross-branch semantics, stable pagination, and streaming search API parity from Phase 3. |
| Ask/chat completion API | Router exposes `/api/v1/ask/completions`; `specs/acceptance/ask.md` expects citations, durable threads/history, rename/visibility, and scoped retrieval. | Partial | Add missing ask/chat endpoints and durable thread lifecycle support; harden citation contracts and richer ask semantics from Phase 5. |
| Review webhook and review-agent visibility/intake APIs | Router exposes authenticated review-webhook, delivery-attempt, and review-agent-run listing/detail endpoints plus public webhook event intake; acceptance index uses these as the current baseline for review automation visibility. | Partial | Finish durable worker orchestration, retries, scheduling, failure recovery, and dedicated worker/operator acceptance specs before claiming automation parity. |
| Audit, analytics, and OAuth client admin APIs | Router exposes `/api/v1/auth/audit-events`, `/api/v1/auth/analytics`, and `/api/v1/auth/oauth-clients`; parity matrix lists audit logs, analytics, and OAuth client/token flows as later-phase advanced features. | Partial | Implement full OAuth authorization/token/revocation flows, deepen audit/analytics behavior beyond endpoint presence, and add dedicated acceptance evidence. |
| Connection management and repository-status APIs | Acceptance index and parity matrix call out connection management and sync/index visibility, but the current router does not expose dedicated connection CRUD endpoints or the roadmap’s planned repository-status APIs. | Missing | Implement durable connection models plus authenticated connection CRUD and repository status endpoints from roadmap Tasks 13, 14, and 18. |

### Evidence notes

- The backend/API surface is broader than a pure MVP: the router already exposes public config, local auth, repo browse/search/commit/code-nav, ask completions, review-webhook visibility, audit, analytics, OAuth-client, and API-key endpoints.
- Even so, the evidence stays mostly at the **route-family** level today. The acceptance docs intentionally describe broad behavior, and `specs/FEATURE_PARITY.md` still leaves every row in a `Needs audit` state.
- The roadmap explicitly schedules major backend/API follow-up work after this slice: durable metadata foundations (Phase 1), connection CRUD and sync visibility (Phase 2), real search/indexing parity (Phase 3), browse/commit/code-nav hardening (Phase 4), ask/chat contract completion (Phase 5), and OAuth/auth/admin expansion (Phases 6–7).
- Because those downstream roadmap items remain open, this slice treats current backend/API parity as **Partial** unless the repo shows a clearly closed gap, which it does not yet.

## Remaining domains

Later slices should extend this document with:

- Worker
- Integrations
- Frontend
- Auth/admin
- Ops
