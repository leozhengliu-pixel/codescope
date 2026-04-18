# Parity gap report

_This document is the canonical repo-local parity gap report for `sourcebot-rewrite`._

This report tracks parity gaps against the 2026-04-18 full-parity roadmap. Task `03` is too broad for a single slice, so the completed slices now cover **task03a** (**backend/API**) and **task03b** (**worker**). Later slices should extend this same document with additional domains instead of creating competing gap reports.

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

## Worker domain

Grounding for this slice comes from the live worker entrypoint in `crates/worker/src/main.rs`, the worker library and focused smoke/unit coverage in `crates/worker/src/lib.rs` plus `crates/worker/tests/*.rs`, the acceptance inventory and journey map in `specs/acceptance/index.md` and `specs/acceptance/journeys.md`, the parity matrix in `specs/FEATURE_PARITY.md`, and the roadmap follow-up tasks in `docs/plans/2026-04-18-sourcebot-full-parity-roadmap.md`.

| Capability area | Current rewrite evidence | Status | Highest-value next gap(s) |
| --- | --- | --- | --- |
| One-shot worker entrypoint and state-path wiring | `crates/worker/src/main.rs` loads `AppConfig::from_env()`, builds the organization store from `config.organization_state_path`, converts the configured stub outcome, and runs exactly one `run_worker_tick(...)`. | Partial | Add the dedicated worker-runtime acceptance spec promised by the acceptance index/journey map, then expand beyond one-shot execution into durable orchestration and operator-visible runtime behavior. |
| Single-run claim/complete-fail execution path | `crates/worker/src/lib.rs` claims the next queued review-agent run, routes it through `execute_claimed_review_agent_run_stub`, and persists either Completed or Failed through the organization store. | Partial | Replace the stub-only execution boundary with real review-agent execution, durable queue orchestration, richer failure handling, and retry/backoff behavior from roadmap Phase 8. |
| Idle/no-work runtime behavior | `crates/worker/src/main.rs` logs `no queued review-agent run available`; `crates/worker/tests/no_queued_review_agent_run_idle_smoke.rs` proves successful exit for default-empty, missing-file, and existing-no-queued state without unintended persisted-state mutation. | Partial | Promote the current idle-path evidence into a dedicated black-box worker acceptance spec and extend it with operator-facing diagnostics, scheduling semantics, and runtime visibility instead of only smoke-level guarantees. |
| Config-driven stub outcome contract | `crates/worker/src/main.rs` reads `SOURCEBOT_STUB_REVIEW_AGENT_RUN_EXECUTION_OUTCOME`; `crates/worker/tests/explicit_completed_stub_outcome_smoke.rs` and `invalid_stub_outcome_smoke.rs` prove explicit `completed` support and fail-closed rejection of invalid configured outcomes. | Partial | Replace stub-outcome-only control with the real execution contract while preserving fail-closed config validation and expanding acceptance coverage beyond env-driven smoke tests. |
| Ordered repeated invocations over queued runs | `crates/worker/tests/explicit_completed_stub_outcome_smoke.rs` proves repeated real-binary invocations process only one oldest queued run at a time, then advance to the next-oldest remaining queued run on later invocations. | Partial | Add real queue scheduling, retry windows, and continuous/background orchestration so progress does not depend on manually rerunning the one-shot binary. |
| Retry, scheduling, and resume-loop orchestration | `specs/acceptance/index.md` and `specs/acceptance/journeys.md` both mark retry/scheduling/resume loops as not implemented and call for a dedicated `specs/acceptance/worker-runtime.md` follow-up before broad worker work proceeds. | Missing | Implement roadmap Tasks 63–65: real queue orchestration, real execution, durable retries/backoff, and the worker/operator acceptance docs needed to judge parity. |
| Worker/operator observability | The current worker surface is mostly logs from `crates/worker/src/main.rs` plus authenticated review-webhook/run inspection endpoints owned by the API layer; there is no dedicated worker runtime acceptance doc or operator status surface yet. | Missing | Add dedicated worker/runtime acceptance docs and later operator observability surfaces for queue depth, last-run, retry, failure, and health parity from roadmap Phases 8–9. |

### Evidence notes

- The worker surface is no longer purely hypothetical: the repo has a real `sourcebot-worker` binary, a one-shot `run_worker_tick` path, explicit Completed/Failed stub outcomes, oldest-queued claim semantics, repeated-invocation smoke coverage, and no-work idle-path coverage.
- However, the current evidence is still intentionally **stub-oriented**. The worker runs one tick, depends on a file-backed organization store path, and treats actual review execution, retries, scheduling, and background loops as explicitly deferred follow-up work.
- The acceptance corpus itself confirms that gap: `specs/acceptance/index.md` still marks worker execution parity as a missing dedicated acceptance spec, while `specs/acceptance/journeys.md` says the current worker evidence is enough only to justify creating `specs/acceptance/worker-runtime.md`, not to claim full worker automation parity.
- Because roadmap Phase 8 still reserves real orchestration, execution, and retry behavior for later tasks, this slice treats most current worker capability areas as **Partial** and the missing orchestration/observability layers as **Missing**.

## Integrations domain

Grounding for this slice comes from the integrations acceptance spec in `specs/acceptance/integrations.md`, the feature inventory rows in `specs/FEATURE_PARITY.md`, the live API router and authenticated/public integration-adjacent endpoints in `crates/api/src/main.rs`, the shared connection/OAuth/review-webhook models in `crates/models/src/lib.rs`, the repo/detail assembly in `crates/core/src/lib.rs`, the sample catalog/auth fixtures in `crates/api/src/storage.rs` and `crates/api/src/auth.rs`, the MCP crate in `crates/mcp/src/lib.rs`, the current frontend repo/detail connection display in `web/src/App.tsx`, and the roadmap follow-up tasks in `docs/plans/2026-04-18-sourcebot-full-parity-roadmap.md`.

| Capability area | Current rewrite evidence | Status | Highest-value next gap(s) |
| --- | --- | --- | --- |
| GitHub-oriented connection and review-webhook baseline | `crates/models/src/lib.rs` defines `ConnectionKind::GitHub`; repo/detail responses carry `connection` metadata; `crates/api/src/main.rs` exposes authenticated review-webhook CRUD/listing plus the public `/api/v1/review-webhooks/{webhook_id}/events` intake route; fixtures in `crates/api/src/auth.rs` exercise a `conn_github`-backed webhook/review-agent flow. | Partial | Finish real GitHub connection auth/configuration and repository enumeration/sync from roadmap Task 56, then complete provider webhook validation, event normalization, idempotency, and operator-visible failure handling from Task 62. |
| GitLab and generic/local Git connection modeling | `crates/models/src/lib.rs` already includes `ConnectionKind::GitLab` and `ConnectionKind::Local`, and repo/detail + frontend repo panels surface connection kind plus `sync_state`; acceptance integrations explicitly expects local/generic Git ingestion without a SaaS provider. | Partial | Add authenticated connection CRUD plus real local/generic Git and GitLab ingestion/enumeration flows, then prove per-connection/per-repository sync behavior with dedicated acceptance coverage from roadmap Tasks 13, 14, 18, and 57. |
| Gitea, Gerrit, Bitbucket, and Azure DevOps providers | `specs/FEATURE_PARITY.md` lists Gitea, Gerrit, Bitbucket, and Azure DevOps as required integration rows, but the current rewrite models/routes/tests do not expose provider-specific connection kinds, auth flows, or sync behavior for them. | Missing | Implement provider-specific connection models and sync/auth flows for roadmap Tasks 58–59 before claiming multi-host parity. |
| OIDC / SSO provider login and external account mapping | `specs/acceptance/integrations.md` expects OIDC/SSO login, but the live auth surface in `crates/api/src/main.rs` is still local bootstrap/login/session management plus OAuth client admin endpoints; repo-wide code search in this run found no `oidc`, `sso`, or `openid` implementation paths. | Missing | Land roadmap Task 60 and the acceptance/doc follow-ups needed for identity-provider metadata, login callbacks, and mapping external identities to local users/orgs. |
| MCP server retrieval surface | `crates/mcp/src/lib.rs` publishes an MCP manifest, retrieval tool definitions, and `execute_tool_call(...)` support for `list_repos`, `list_tree`, `read_file`, `glob`, and `grep`, giving the rewrite a real repository-aware MCP contract at the library layer. | Partial | Add the transport/runtime/auth wiring and explicit permission-scoped acceptance evidence required by roadmap Task 46 so MCP parity is proven beyond crate-local contracts. |
| Public REST API contract and machine-readable responses | `crates/api/src/main.rs` serves versioned `/api/v1/...` routes across config, repo browse/search, auth, OAuth-client, and review-webhook surfaces, while `web/src/App.tsx` consumes typed JSON repo/detail responses that include connection and sync metadata. | Partial | Expand endpoint completeness, integration-specific acceptance evidence, and durable connection/repository operations so the public API satisfies the broader connector/admin/operator contracts promised by the integrations acceptance spec. |

### Evidence notes

- The rewrite already has meaningful integration-adjacent scaffolding: shared connection models, repo/detail responses that surface connection metadata and `sync_state`, GitHub-backed review-webhook ingestion/inspection endpoints, a versioned public REST API, and a standalone MCP crate.
- Even so, the current evidence is still mostly **shared-contract** or **fixture-backed** rather than end-to-end provider parity. There is no authenticated connection-management surface yet, no real provider auth/credential exchange, no repo-enumeration/import workflow, and no broad sync/recovery behavior tied to specific external hosts.
- The gap is especially clear for identity and provider breadth: the current codebase has local auth plus OAuth-client admin records, but no OIDC/SSO login path, and no concrete implementation evidence for Gitea, Gerrit, Bitbucket, or Azure DevOps.
- Because roadmap Phase 7 and Phase 8 still reserve the real provider, identity, and webhook-hardening work for later tasks, this slice keeps all current integrations capability areas at **Partial** or **Missing** rather than over-claiming parity from shared models alone.

## Remaining domains

Later slices should extend this document with:

- Frontend
- Auth/admin
- Ops
