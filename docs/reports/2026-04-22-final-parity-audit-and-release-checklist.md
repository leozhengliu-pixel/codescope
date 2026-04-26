# Final parity audit and release checklist

## Purpose

This document closes roadmap Task 85 by mapping every current `specs/FEATURE_PARITY.md`
row to shipped rewrite evidence and to the blockers that still prevent an honest
"full parity" release claim.

## Scope and evidence policy

- This audit is grounded only in repo-local evidence already shipped in code, tests,
  acceptance specs, and the canonical gap report.
- A row is **Partial** when the rewrite has a real bounded baseline but still has
  explicit deferred gaps.
- A row is **Missing** when the repo has no truthful end-to-end baseline for that
  feature beyond shared models, placeholder routes, or future-facing docs.
- This audit does **not** upgrade any row to `Complete` unless the current repo
  evidence actually closes the practical parity contract.

## Release verdict

**Release verdict: NOT READY for a truthful full-parity Sourcebot replacement claim.**

The rewrite now has broad bounded baselines across search, browse, ask/chat,
auth/settings, review automation visibility, local runtime bring-up, and local
operator maintenance. But the live repo still has explicit blockers in durable
metadata/migrations, real provider integrations, end-to-end OIDC/SSO,
search/indexing depth, richer thread and admin lifecycles, worker scheduling/
retries, and production-grade operator/runtime parity.

## Audit summary by domain

### Core user features

| Feature | Status | Code / tests / docs evidence | Remaining blockers |
| --- | --- | --- | --- |
| Cross-repo and cross-branch code search | Partial | Code: `/api/v1/search` in `crates/api/src/main.rs` plus `#/search` in `web/src/App.tsx`; tests/docs: `specs/acceptance/search.md`, `web/src/App.test.tsx`, `docs/reports/2026-04-18-parity-gap-report.md` (backend/API + frontend search rows). | Still lacks real indexing pipeline, richer branch/index availability handling, and stronger pagination/relevance guarantees. |
| Regex, literal, boolean, repo/language/path filters | Partial | Code/docs: `specs/acceptance/search.md` already grounds regex/literal/boolean and repo/language/path filter expectations on top of `/api/v1/search`; audit basis remains the search rows in the gap report. | Filter grammar and validation are not yet fully proven to upstream parity, and real index-backed semantics remain open. |
| File explorer with tree browsing and syntax highlighting | Partial | Code: repo detail browse shell in `web/src/App.tsx`, tree/blob APIs in `crates/api/src/main.rs`; docs/tests: `specs/acceptance/browse.md`, `web/src/App.test.tsx`, gap-report browse/frontend rows. | Nested browse works and the backend now reports non-UTF-8 local/revisioned blobs as binary metadata instead of broken text, but production-grade syntax highlighting, frontend binary/download affordances, large-file handling, and broader browse UX parity remain deferred. |
| Repository page and repo list page | Partial | Code: `RepoListPage` and `RepoDetailPage` in `web/src/App.tsx`, `/api/v1/repos` and `/api/v1/repos/{repo_id}` in `crates/api/src/main.rs`; docs/tests: `specs/acceptance/browse.md`, `specs/acceptance/repository-operations.md`, `web/src/App.test.tsx`. | Current shell is useful but still shallow versus full multi-page parity, richer repo metadata, and durable catalog-backed parity. |
| File source view | Partial | Code: blob retrieval plus repo detail browse/source rendering in `crates/api/src/main.rs` and `web/src/App.tsx`; docs/tests: `specs/acceptance/browse.md`, `web/src/App.test.tsx`. | Safe source rendering exists and backend blob retrieval now distinguishes binary blobs with `is_binary`, but syntax-highlighting depth, frontend binary/download UX, large-file handling, and broader source-view polish remain open. |
| Commit list, commit detail, and diff view | Partial | Code: commit routes in `crates/api/src/main.rs`, `CommitsPanel` in `web/src/App.tsx`; docs/tests: `specs/acceptance/browse.md`, `web/src/App.test.tsx`, gap-report browse rows. | No broader commit-page UX, pagination/history navigation, or large-diff hardening yet. |
| Code navigation: definitions and references | Partial | Code: definitions/references endpoints in `crates/api/src/main.rs`; UI: browse panel symbol navigation in `web/src/App.tsx`; docs/tests: `specs/acceptance/code-nav.md`, `web/src/App.test.tsx`. | Works as a bounded baseline, but multi-language correctness, stale-index status, and richer UX parity remain deferred. |
| Ask the codebase with inline citations | Partial | Code: `/api/v1/ask/completions` and citation filtering in `crates/api/src/main.rs`; UI: `#/ask` route in `web/src/App.tsx`; docs/tests: `specs/acceptance/ask.md`, `web/src/App.test.tsx`, backend/API ask row in the gap report. | Repo-scoped ask baseline exists, but richer retrieval/model/runtime behavior and broader ask UX parity remain open. |
| Chat threads, history, rename, visibility, delete | Partial | Code: `/api/v1/ask/threads` plus `GET`/`PATCH`/`DELETE /api/v1/ask/threads/{thread_id}` in `crates/api/src/main.rs`; UI: `#/chat` in `web/src/App.tsx`; docs/tests: `specs/acceptance/ask.md`, `web/src/App.test.tsx`. | Thread list/detail/reopen, backend title/private-shared visibility updates, focused frontend rename/visibility controls, and a bounded caller-owned visible-thread delete lifecycle now exist, but archive controls, richer conversation/source-preview UX, streaming/progress states, and full chat parity remain open. |
| Search contexts / saved scopes | Partial | Code/tests: `/api/v1/auth/search-contexts` now supports create/list/delete and `/api/v1/search?context_id=...` applies caller-owned saved scopes in `crates/api/src/main.rs`; docs: `specs/acceptance/index.md` and `specs/acceptance/search.md`; gap report API-key/admin row. | Backend saved-context lifecycle and fail-closed search narrowing exist, but frontend context management UI, SQL-backed context durability, richer grammar, relevance tuning, and stable pagination parity remain open. |

### Admin and org features

| Feature | Status | Code / tests / docs evidence | Remaining blockers |
| --- | --- | --- | --- |
| First-run onboarding | Partial | Code: `/api/v1/auth/bootstrap` in `crates/api/src/main.rs`; UI: `#/auth` onboarding/login flow in `web/src/App.tsx`; docs/tests: `specs/acceptance/auth.md`, `web/src/App.test.tsx`, auth/admin gap-report baseline row. | Works only as a local baseline; durable metadata, richer onboarding UX, and broader account-management parity remain open. |
| Organizations, membership, invites, roles | Partial | Code/models: org/member/invite/role state in `crates/models/src/lib.rs` and auth handlers in `crates/api/src/main.rs`; UI/docs/tests: `#/settings/members`, minimal admin invite-create, invite-redeem flow, `specs/acceptance/auth.md`, auth/admin gap-report org row. | Admin-visible inventory, bounded file-backed pending-invite creation, and invite redemption exist, but durable PostgreSQL invite creation, email/resend/cancel, CRUD/admin management, and role lifecycle parity are still missing. |
| API keys | Partial | Code: `/api/v1/auth/api-keys` plus revoke in `crates/api/src/main.rs`; UI: `#/settings/api-keys` in `web/src/App.tsx`; docs/tests: `specs/acceptance/auth.md`, `specs/acceptance/settings-navigation.md`, `web/src/App.test.tsx`. | Minimal inventory/create/revoke baseline exists with one-time plaintext secret reveal and basic newline-delimited scope entry, but richer rotation, bulk management, advanced scoping UX, and full credential-management lifecycle parity are still incomplete. |
| Connection management | Partial | Code: authenticated connection CRUD in `crates/api/src/main.rs`; UI: `#/settings/connections` in `web/src/App.tsx`; docs/tests: `specs/acceptance/integrations.md`, `specs/acceptance/generic-local-git.md`, `web/src/App.test.tsx`. | Current shell is still limited and does not yet prove provider-specific auth, discovery, or durable catalog/runtime parity. |
| Sync state and indexing status | Partial | Code: repo `sync_state` and `/api/v1/auth/repository-sync-jobs`; UI/docs/tests: `specs/acceptance/repository-operations.md`, `web/src/App.tsx`, gap-report repository-operations rows. | Coarse sync visibility exists, but truthful persisted index-status and durable repository-operations parity remain blocked. |
| Linked external accounts | Partial | Code: `/api/v1/auth/linked-accounts` in `crates/api/src/main.rs`; UI: `#/settings/linked-accounts` in `web/src/App.tsx`; docs/tests: `specs/acceptance/auth.md`, `specs/acceptance/settings-navigation.md`, `web/src/App.test.tsx`. | Only local-identity visibility baseline exists; real external account linking and mapping remain missing. |
| Access / permission sync | Partial | Code: visibility enforcement in `crates/core/src/lib.rs` and protected API handlers in `crates/api/src/main.rs`; UI/docs: `#/settings/access`, `specs/acceptance/auth.md`, auth/admin gap-report access row. | Read-only visibility and fail-closed checks exist, but durable permission-sync lifecycle parity remains open. |

### Integrations

| Feature | Status | Code / tests / docs evidence | Remaining blockers |
| --- | --- | --- | --- |
| GitHub | Partial | Code/models: `ConnectionKind::GitHub`, review-webhook surfaces in `crates/api/src/main.rs`, fixture-backed GitHub webhook/review-agent flows; docs: `specs/acceptance/integrations.md`, integrations gap-report GitHub row. | Real GitHub auth/configuration, repo enumeration/import, and hardened runtime behavior remain open. |
| GitLab | Partial | Code/models: `ConnectionKind::GitLab` and shared connection CRUD; docs: `specs/acceptance/integrations.md`, integrations gap-report GitLab/generic-local row. | Provider-specific GitLab auth, discovery, and runtime parity are still missing. |
| Gitea | Missing | Evidence is limited to shared `ConnectionKind::Gitea` modeling in `crates/models/src/lib.rs`; integrations gap report marks provider-specific behavior absent. | No real auth, enumeration/import, or runtime parity. |
| Gerrit | Missing | Evidence is limited to shared `ConnectionKind::Gerrit` modeling in `crates/models/src/lib.rs`; integrations gap report marks provider-specific behavior absent. | No real auth, enumeration/import, or runtime parity. |
| Bitbucket | Missing | Evidence is limited to shared `ConnectionKind::Bitbucket` modeling in `crates/models/src/lib.rs`; integrations gap report marks provider-specific behavior absent. | No real auth, enumeration/import, or runtime parity. |
| Azure DevOps | Missing | Evidence is limited to shared `ConnectionKind::AzureDevOps` modeling in `crates/models/src/lib.rs`; integrations gap report marks provider-specific behavior absent. | No real auth, enumeration/import, or runtime parity. |
| Generic Git host / local Git | Partial | Code: generic/local connection CRUD and local import baseline in `crates/api/src/main.rs` plus repo detail connection metadata in `crates/core/src/lib.rs`; UI/docs/tests: `#/settings/connections`, `specs/acceptance/generic-local-git.md`, `web/src/App.test.tsx`. | One bounded local-import/settings baseline exists, but generic-host discovery, recursive import, and durable parity remain open. |
| OIDC / SSO providers | Missing | Docs: `specs/acceptance/integrations.md`, `specs/FEATURE_PARITY.md`, and integrations gap report all explicitly ground this as missing; `#/auth` only shows truthful callback-status notices. | No real external-provider login/callback exchange or external-account mapping. |
| MCP server | Partial | Code: `crates/mcp/src/lib.rs` ships the manifest and repository-aware retrieval tool definitions plus execution support; docs: `specs/acceptance/integrations.md`, `specs/FEATURE_PARITY.md`, integrations gap report. | Transport/runtime/auth wiring and end-to-end permission-scoped evidence remain open. |
| Public REST API | Partial | Code: versioned `/api/v1/...` routes across config, repos, search, auth, ask, OAuth clients, and review webhooks in `crates/api/src/main.rs`; docs: `specs/acceptance/integrations.md`, integrations gap report. | Many route families are still bounded baselines rather than full connector/admin/operator parity. |

### Later-phase advanced features

| Feature | Status | Code / tests / docs evidence | Remaining blockers |
| --- | --- | --- | --- |
| Audit logs | Partial | Code: `/api/v1/auth/audit-events` in `crates/api/src/main.rs`; UI: `#/settings/observability` in `web/src/App.tsx`; docs/tests: `specs/acceptance/auth.md`, `specs/acceptance/settings-navigation.md`, auth/admin + ops gap-report rows. | Read-only visibility exists, but richer filtering/export/operational parity is not closed. |
| Analytics | Partial | Code: `/api/v1/auth/analytics` in `crates/api/src/main.rs`; UI: `#/settings/observability`; docs/tests: `specs/acceptance/auth.md`, `specs/acceptance/settings-navigation.md`, auth/admin + ops gap-report rows. | Read-only visibility exists, but analytics depth and operator workflows remain incomplete. |
| OAuth client / token flows | Partial | Code: `/api/v1/auth/oauth-clients` inventory/create in `crates/api/src/main.rs`; UI: `#/settings/oauth-clients` in `web/src/App.tsx`; docs/tests: `specs/acceptance/auth.md`, `specs/acceptance/settings-navigation.md`, auth/admin gap-report rows. | Client inventory/create baseline exists, but authorization, token, revocation, and broader manage UX remain missing. |
| Review agent / webhook automation | Partial | Code: authenticated review-webhook, delivery-attempt, and review-agent-run APIs plus public webhook intake in `crates/api/src/main.rs`; UI: `#/settings/review-automation` and `#/agents` in `web/src/App.tsx`; tests/docs: `scripts/check_end_to_end_smoke_matrix.sh`, `specs/acceptance/ask.md`, `specs/acceptance/auth.md`, operator-runtime + gap-report rows. | Visibility and bounded smoke coverage exist, but durable orchestration, retries, scheduling, and richer operator controls remain open. |
| Enterprise entitlement controls | Missing | Present evidence is limited to roadmap and parity-matrix placeholders; the canonical gap report does not claim a shipped entitlement-control surface. | No truthful entitlement implementation or acceptance baseline exists yet. |

## Full-parity release checklist

- [x] `specs/FEATURE_PARITY.md` now has audited row statuses and evidence pointers instead of `Needs audit` / `_TBD_` placeholders.
- [x] This audit maps every current matrix row to code/tests/docs evidence plus remaining blockers.
- [ ] Durable metadata and migration parity are complete across catalog, auth, org, ask-thread, and review-agent state.
- [ ] Search/indexing parity is complete beyond the current bounded API-and-UI baseline.
- [ ] Browse/source/commit/code-nav parity is complete beyond the current bounded shell.
- [ ] Ask/chat parity is complete beyond the current thread rename/delete/visibility baseline, including archive/richer conversation lifecycle UX.
- [ ] Auth/admin parity is complete, including durable stores, richer org/invite management, API-key lifecycle, and external-provider identity flows.
- [ ] Provider parity is complete across GitHub, GitLab, Gitea, Gerrit, Bitbucket, Azure DevOps, and generic/local Git.
- [ ] MCP parity is complete with real runtime/auth wiring and explicit end-to-end evidence.
- [ ] Worker/runtime parity is complete with real scheduling, retries, supervision, and richer observability.
- [ ] Operator/runtime parity is complete beyond the current local stub-backed baseline.
- [ ] The rewrite can honestly be described as a practical Sourcebot replacement rather than a bounded clean-room subset.

## Conclusion

Task 85 can be closed as an honest audit-and-checklist closure because the repo now
has a final parity audit document and a fully grounded feature matrix. The audit's
truthful answer is that release parity is **not** complete yet, and the unchecked
items above are the blocking work that remains. Those blockers now continue under
`docs/plans/2026-04-22-sourcebot-follow-on-parity-roadmap.md` so future runs can
resume from a truthful follow-on roadmap instead of reopening the finished 85-task
audit roadmap.