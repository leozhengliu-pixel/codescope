# Acceptance Spec Index: Full-Parity Surface Inventory

## Purpose
This index is the clean-room acceptance entrypoint for the full-parity roadmap. It turns the roadmap's broad parity goal into concrete black-box surface areas so later tasks can attach tests, UX work, persistence changes, and operator workflows to an explicit acceptance contract instead of local implementation convenience.

## Governing sources
- `docs/plans/2026-04-18-sourcebot-full-parity-roadmap.md`
- `docs/status/roadmap-state.yaml`
- `specs/FEATURE_PARITY.md`
- Current rewrite surface inventory from:
  - `crates/api/src/main.rs`
  - `web/src/App.tsx`
  - `crates/worker/src/main.rs`

## Inventory rules
1. Treat each row below as a parity-facing surface family, not an implementation detail.
2. Acceptance specs should stay black-box: describe inputs, expected behavior, permissions, edge cases, and operator-visible outcomes.
3. When a parity area already has an acceptance spec, later tasks should extend that spec instead of creating overlapping documents.
4. When a parity area has no acceptance spec yet, later roadmap slices must create one before broad implementation starts.

## Current acceptance-spec coverage

| Surface family | Acceptance spec | Status | Notes |
| --- | --- | --- | --- |
| Search | `specs/acceptance/search.md` | Present | Covers query/filter/result behavior, but parity matrix expansion still needs evidence placeholders. |
| Browse and source view | `specs/acceptance/browse.md` | Present | Covers tree/blob behavior plus the current repo-route revision control baseline; later parity tasks still need richer branch/revision UX and broader UI parity coverage. |
| Code navigation | `specs/acceptance/code-nav.md` | Present | Covers definitions/references behavior; later parity tasks will extend multi-language and UI parity details. |
| Ask and chat | `specs/acceptance/ask.md` | Present | Covers the shipped `#/ask` + `#/chat` baseline for repo-scoped asks, authenticated thread list/detail/reopen, inline citations, hash-restored active-thread continuity, and the dedicated `#/agents` review-agent visibility baseline; later parity tasks still need rename/delete/visibility plus richer agents-management/retry/orchestration coverage. |
| Auth and permissions | `specs/acceptance/auth.md` | Present | Covers local auth + permission boundaries; later parity tasks must expand onboarding/orgs/invites/API keys/OAuth details. |
| Integrations | `specs/acceptance/integrations.md` + `specs/acceptance/generic-local-git.md` | Expanded in task20a | Broad integrations coverage now has a dedicated generic/local Git acceptance home; later parity tasks must continue splitting provider-specific and webhook/operator flows. |
| Repository operations | `specs/acceptance/repository-operations.md` | Present in task18c | Locks the current sync-state and authenticated sync-job visibility contract while recording the remaining persisted-catalog, index-status, recovery, and admin-surface parity gaps. |
| Journey map and missing-spec prerequisites | `specs/acceptance/journeys.md` | Present in this slice | Maps indexed surfaces into user/admin/operator journeys and names the missing dedicated acceptance specs that must be created before broader implementation proceeds. |
| Frontend route/page parity index | `specs/acceptance/index.md` | Present in task01a1 | Surface inventory anchor for route/page/worker/operator evidence. |
| Settings navigation shell | `specs/acceptance/settings-navigation.md` | Present in this slice | Covers the shared `#/settings` landing page plus subsection-shell discoverability without over-claiming richer admin CRUD UX. |
| Worker execution parity | `specs/acceptance/worker-runtime.md` | Present in task80b | Covers the shipped one-shot, stub-oriented worker runtime baseline, runtime-baseline logging, no-work exits, and stub terminal outcomes while explicitly deferring real execution, retries, supervision, durable metadata, and production observability. |
| Operator workflows parity | `specs/acceptance/operator-runtime.md` | Present in task80a | Covers the shipped local runtime liveness/config baseline, shared `SOURCEBOT_DATA_DIR` path wiring, and `make api` + one-shot `make worker` bring-up while explicitly deferring migrations, readiness, supervision, durable metadata, and production-grade observability. |
| Operator maintenance parity | `specs/acceptance/operator-maintenance.md` | Present in task81b | Covers the shipped local runtime backup/restore helpers plus the local Postgres metadata backup/restore baseline and current SQLx migration/upgrade runbook while explicitly deferring full durable-surface parity and production deployment guarantees. |

## Surface inventory

### 1. Public and authenticated API surface families

| Surface family | Representative routes in rewrite | Parity intent | Current acceptance home |
| --- | --- | --- | --- |
| Health and public config | `/healthz`, `/api/v1/config` | Operator-visible service liveness and frontend bootstrap config parity | `specs/acceptance/operator-runtime.md` |
| Bootstrap and local session auth | `/api/v1/auth/bootstrap`, `/api/v1/auth/login`, `/api/v1/auth/me`, `/api/v1/auth/logout`, `/api/v1/auth/revoke` | First-run onboarding, login/session restoration, and local-admin auth parity | `specs/acceptance/auth.md` |
| API key management | `/api/v1/auth/api-keys`, `/api/v1/auth/api-keys/{api_key_id}/revoke` | Authenticated API credential lifecycle parity | `specs/acceptance/auth.md` (to be expanded) |
| Search contexts | `/api/v1/auth/search-contexts` | Saved search scope/context parity | `specs/acceptance/search.md` (to be expanded) |
| Audit and analytics | `/api/v1/auth/audit-events`, `/api/v1/auth/analytics` | Admin/operator visibility parity | `specs/acceptance/index.md` until dedicated operator/admin specs exist |
| Review webhooks and review-agent run visibility | `/api/v1/auth/review-webhooks`, `/api/v1/auth/review-webhooks/{webhook_id}`, `/api/v1/auth/review-webhooks/{webhook_id}/delivery-attempts`, `/api/v1/auth/review-webhooks/{webhook_id}/delivery-attempts/{attempt_id}`, `/api/v1/auth/review-webhooks/{webhook_id}/review-agent-runs`, `/api/v1/auth/review-webhooks/{webhook_id}/review-agent-runs/{run_id}`, `/api/v1/auth/review-webhook-delivery-attempts`, `/api/v1/auth/review-webhook-delivery-attempts/{attempt_id}`, `/api/v1/auth/review-agent-runs`, `/api/v1/auth/review-agent-runs/{run_id}`, `/api/v1/review-webhooks/{webhook_id}/events` | Review-agent and webhook automation parity across authenticated views plus public webhook intake | `specs/acceptance/index.md` until dedicated review-webhook / worker / operator specs exist |
| OAuth clients | `/api/v1/auth/oauth-clients` | OAuth client/token management parity | `specs/acceptance/index.md` until dedicated OAuth/admin specs exist |
| Repository catalog and detail | `/api/v1/repos`, `/api/v1/repos/{repo_id}` | Repo list/repo detail parity | `specs/acceptance/browse.md` plus `specs/acceptance/repository-operations.md` for sync/index visibility |
| Tree and blob retrieval | `/api/v1/repos/{repo_id}/tree`, `/api/v1/repos/{repo_id}/blob` | File explorer and source view parity | `specs/acceptance/browse.md` |
| Definitions and references | `/api/v1/repos/{repo_id}/definitions`, `/api/v1/repos/{repo_id}/references` | Code-navigation parity | `specs/acceptance/code-nav.md` |
| Commit history/detail/diff | `/api/v1/repos/{repo_id}/commits`, `/api/v1/repos/{repo_id}/commits/{commit_id}`, `/api/v1/repos/{repo_id}/commits/{commit_id}/diff` | Commit browsing and diff parity | `specs/acceptance/browse.md` until commit-specific expansion lands |
| Search | `/api/v1/search` | Search parity across accessible repositories | `specs/acceptance/search.md` |
| Ask completions and thread reads | `/api/v1/ask/completions`, `/api/v1/ask/threads`, `/api/v1/ask/threads/{thread_id}` | Ask/chat/citation parity | `specs/acceptance/ask.md` |

### 2. Frontend page and panel surface families

| Surface family | Rewrite evidence | Parity intent | Current acceptance home |
| --- | --- | --- | --- |
| Repository home/list page | `web/src/App.tsx` → `RepoListPage`, route `#/` | Repo inventory, sync-state visibility, and top-level navigation parity while the dedicated search flow moves to `#/search` | `specs/acceptance/browse.md` plus `specs/acceptance/repository-operations.md` for sync/index visibility |
| Dedicated search page | `web/src/App.tsx` → `SearchPage`, route `#/search` | User-facing route parity for API-backed code search without overloading the repository inventory page | `specs/acceptance/search.md` |
| Repository detail page shell | `web/src/App.tsx` → `RepoDetailPage`, route `#/repos/:repoId` | Repo-scoped metadata, browse/source, code-navigation, commit-view, and sync-state parity shell | `specs/acceptance/browse.md` plus `specs/acceptance/repository-operations.md` for sync/index visibility |
| Commit panel | `web/src/App.tsx` → `CommitsPanel` | Commit list/detail/diff UX parity | `specs/acceptance/browse.md` until commit/front-end expansion lands |
| Browse/source panel | `web/src/App.tsx` → `BrowsePanel` | Tree browsing, file rendering, symbol-click navigation parity | `specs/acceptance/browse.md` and `specs/acceptance/code-nav.md` |
| Ask/chat frontend | `web/src/App.tsx` → `AskPage` + `ChatPage` + `AgentsPage`, routes `#/ask`, `#/chat`, and `#/agents` | Dedicated ask/chat-route parity for repo-scoped prompts, authenticated repo-scoped thread history/reopen, inline citations, active-thread continuity, and a dedicated operator-visible review-agent route baseline while rename/delete/visibility and richer agents-management/retry/orchestration remain follow-up work | `specs/acceptance/ask.md` |
| Auth/admin/settings frontend | `web/src/App.tsx` now includes `#/auth`, `#/settings`, `#/settings/connections`, `#/settings/api-keys`, `#/settings/members`, `#/settings/access`, `#/settings/linked-accounts`, `#/settings/oauth-clients`, `#/settings/observability`, and `#/settings/review-automation` through a shared shell; `#/auth` now covers first-run onboarding, local login/session restoration, invite-redemption entry links, and truthful OAuth callback-status notices for provider redirects, while broader invite management, external-provider login/callback exchange, and richer account-management flows remain follow-up work | Onboarding, local login/session restoration, invite-redemption baseline, narrow OAuth-callback-status handling, and admin/settings discoverability parity | `specs/acceptance/auth.md` for the auth baseline plus `specs/acceptance/settings-navigation.md` for the shared settings shell |

### 3. Worker and background-execution surface families

| Surface family | Rewrite evidence | Parity intent | Current acceptance home |
| --- | --- | --- | --- |
| One-shot review-agent worker entrypoint | `crates/worker/src/main.rs` → `main`, `run_worker_tick` | Worker parity starts from a single-shot entrypoint that loads config, constructs the organization store, logs the resolved runtime baseline, invokes one worker tick, and logs either a terminal run or the no-work path | `specs/acceptance/worker-runtime.md` |
| Idle/no-work worker behavior | `crates/worker/src/main.rs` | Worker exposes an explicit no-work path by logging when no queued review-agent run or repository-sync job is available | `specs/acceptance/worker-runtime.md` |
| Config-driven stub execution outcomes | `crates/worker/src/main.rs` | Worker entrypoint selects review-agent and repository-sync stub outcomes before invoking the tick, and logs those resolved stub selections as part of the startup runtime baseline | `specs/acceptance/worker-runtime.md` |
| Retry/scheduling/resume loops | _Not implemented yet_ | Real automation parity for retries, polling, rescheduling, and worker orchestration | Future worker acceptance spec |

### 4. Operator/admin workflow surface families

| Surface family | Rewrite evidence | Parity intent | Current acceptance home |
| --- | --- | --- | --- |
| First-run bootstrap workflow | `/api/v1/auth/bootstrap` in `crates/api/src/main.rs` | Operator can initialize a fresh instance safely and deterministically | `specs/acceptance/auth.md` |
| Local-dev organization-state path wiring | `crates/worker/src/main.rs` loads `AppConfig::from_env()` and builds the organization store from `config.organization_state_path` | Dev/runtime state-path behavior, startup tolerance, shared `SOURCEBOT_DATA_DIR` baseline, and explicit-path override expectations | `specs/acceptance/operator-runtime.md` |
| Repo sync/index visibility | Repo `sync_state` exposed through API/UI surfaces in `crates/api/src/main.rs` and `web/src/App.tsx`, plus authenticated sync-job history in `/api/v1/auth/repository-sync-jobs` | Operator/user parity for sync readiness, failure visibility, and read-only sync-job history | `specs/acceptance/repository-operations.md` |
| Review webhook operational visibility | Authenticated review-webhook, delivery-attempt, and review-agent-run endpoints in `crates/api/src/main.rs` | Operator/admin inspection of automation state and failures | `specs/acceptance/index.md` until dedicated operator/review-webhook specs exist |
| Durable metadata / migrations / backup-restore | `scripts/backup_local_runtime_state.sh`, `scripts/restore_local_runtime_state.sh`, `scripts/backup_local_metadata_db.sh`, `scripts/restore_local_metadata_db.sh`, Makefile `runtime-backup`/`runtime-restore`/`metadata-backup`/`metadata-restore`, and the current local SQLx workflow in `README.md` | Local operator maintenance parity now covers file-backed runtime backup/restore plus a local Postgres metadata backup/restore baseline and the repo-local migration/upgrade runbook, while full durable-surface parity remains deferred | `specs/acceptance/operator-maintenance.md` |

## Immediate follow-up gaps exposed by this index
1. The acceptance corpus now has dedicated operator-runtime and worker-runtime specs; later slices should extend those documents instead of restating worker/operator behavior only in this index.
2. Frontend parity surfaces are still mostly implicit inside `web/src/App.tsx`; `specs/acceptance/journeys.md` now maps them into explicit user/admin/operator journeys and names the next missing spec documents.
3. `specs/FEATURE_PARITY.md` now has the evidence column needed for later parity auditing, but most rows are still only placeholders; later slices should keep replacing `_TBD_` rows with grounded evidence instead of treating the matrix as complete.
4. Review-agent, OAuth, analytics, and audit flows still need finer-grained dedicated acceptance specs before broader implementation proceeds; `specs/acceptance/operator-runtime.md` now covers the local runtime baseline, `specs/acceptance/operator-maintenance.md` covers the current local operator maintenance baseline, `specs/acceptance/worker-runtime.md` covers the current one-shot worker baseline, and `specs/acceptance/settings-navigation.md` covers the route-shell baseline, but richer admin workflows still need later split-out specs as the surface deepens.

## Related follow-up document
- `specs/acceptance/journeys.md` maps the indexed surfaces above into explicit user/admin/operator journeys and identifies the minimum dedicated missing acceptance-spec documents exposed by the current rewrite evidence.
