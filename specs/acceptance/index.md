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
| Browse and source view | `specs/acceptance/browse.md` | Present | Covers tree/blob behavior; later parity tasks still need richer branch/revision and UI parity coverage. |
| Code navigation | `specs/acceptance/code-nav.md` | Present | Covers definitions/references behavior; later parity tasks will extend multi-language and UI parity details. |
| Ask and chat | `specs/acceptance/ask.md` | Present | Covers ask behavior at a high level; later parity tasks still need thread lifecycle and citation/UI parity details. |
| Auth and permissions | `specs/acceptance/auth.md` | Present | Covers local auth + permission boundaries; later parity tasks must expand onboarding/orgs/invites/API keys/OAuth details. |
| Integrations | `specs/acceptance/integrations.md` | Present | Covers provider-facing behavior broadly; later parity tasks must split/expand per host/provider and webhook/operator flows. |
| Frontend route/page parity index | `specs/acceptance/index.md` | Present in this slice | New index created by task01a1 to anchor route/page/worker/operator inventory. |
| Worker execution parity | _Missing acceptance spec_ | Planned | Needs a dedicated black-box worker/operator acceptance spec in a later Task 1 slice. |
| Operator workflows parity | _Missing acceptance spec_ | Planned | Needs a dedicated black-box operator/admin/runtime acceptance spec in a later Task 1 slice. |

## Surface inventory

### 1. Public and authenticated API surface families

| Surface family | Representative routes in rewrite | Parity intent | Current acceptance home |
| --- | --- | --- | --- |
| Health and public config | `/healthz`, `/api/v1/config` | Operator-visible service liveness and frontend bootstrap config parity | `specs/acceptance/index.md` until operator spec exists |
| Bootstrap and local session auth | `/api/v1/auth/bootstrap`, `/api/v1/auth/login`, `/api/v1/auth/me`, `/api/v1/auth/logout`, `/api/v1/auth/revoke` | First-run onboarding, login/session restoration, and local-admin auth parity | `specs/acceptance/auth.md` |
| API key management | `/api/v1/auth/api-keys`, `/api/v1/auth/api-keys/{api_key_id}/revoke` | Authenticated API credential lifecycle parity | `specs/acceptance/auth.md` (to be expanded) |
| Search contexts | `/api/v1/auth/search-contexts` | Saved search scope/context parity | `specs/acceptance/search.md` (to be expanded) |
| Audit and analytics | `/api/v1/auth/audit-events`, `/api/v1/auth/analytics` | Admin/operator visibility parity | `specs/acceptance/index.md` until dedicated operator/admin specs exist |
| Review webhooks and review-agent run visibility | `/api/v1/auth/review-webhooks`, `/api/v1/auth/review-webhooks/{webhook_id}`, `/api/v1/auth/review-webhooks/{webhook_id}/delivery-attempts`, `/api/v1/auth/review-webhooks/{webhook_id}/delivery-attempts/{attempt_id}`, `/api/v1/auth/review-webhooks/{webhook_id}/review-agent-runs`, `/api/v1/auth/review-webhooks/{webhook_id}/review-agent-runs/{run_id}`, `/api/v1/auth/review-webhook-delivery-attempts`, `/api/v1/auth/review-webhook-delivery-attempts/{attempt_id}`, `/api/v1/auth/review-agent-runs`, `/api/v1/auth/review-agent-runs/{run_id}`, `/api/v1/review-webhooks/{webhook_id}/events` | Review-agent and webhook automation parity across authenticated views plus public webhook intake | `specs/acceptance/index.md` until dedicated review-webhook / worker / operator specs exist |
| OAuth clients | `/api/v1/auth/oauth-clients` | OAuth client/token management parity | `specs/acceptance/index.md` until dedicated OAuth/admin specs exist |
| Repository catalog and detail | `/api/v1/repos`, `/api/v1/repos/{repo_id}` | Repo list/repo detail parity | `specs/acceptance/browse.md` |
| Tree and blob retrieval | `/api/v1/repos/{repo_id}/tree`, `/api/v1/repos/{repo_id}/blob` | File explorer and source view parity | `specs/acceptance/browse.md` |
| Definitions and references | `/api/v1/repos/{repo_id}/definitions`, `/api/v1/repos/{repo_id}/references` | Code-navigation parity | `specs/acceptance/code-nav.md` |
| Commit history/detail/diff | `/api/v1/repos/{repo_id}/commits`, `/api/v1/repos/{repo_id}/commits/{commit_id}`, `/api/v1/repos/{repo_id}/commits/{commit_id}/diff` | Commit browsing and diff parity | `specs/acceptance/browse.md` until commit-specific expansion lands |
| Search | `/api/v1/search` | Search parity across accessible repositories | `specs/acceptance/search.md` |
| Ask completions | `/api/v1/ask/completions` | Ask/chat/citation parity | `specs/acceptance/ask.md` |

### 2. Frontend page and panel surface families

| Surface family | Rewrite evidence | Parity intent | Current acceptance home |
| --- | --- | --- | --- |
| Repository home/list page | `web/src/App.tsx` → `RepoListPage`, route `#/` | Repo inventory, sync-state visibility, search entry, and top-level navigation parity | `specs/acceptance/browse.md` plus future frontend parity slices |
| Repository detail page shell | `web/src/App.tsx` → `RepoDetailPage`, route `#/repos/:repoId` | Repo-scoped metadata, browse/source, code-navigation, and commit-view parity shell | `specs/acceptance/browse.md` |
| Commit panel | `web/src/App.tsx` → `CommitsPanel` | Commit list/detail/diff UX parity | `specs/acceptance/browse.md` until commit/front-end expansion lands |
| Browse/source panel | `web/src/App.tsx` → `BrowsePanel` | Tree browsing, file rendering, symbol-click navigation parity | `specs/acceptance/browse.md` and `specs/acceptance/code-nav.md` |
| Ask/chat frontend | _No dedicated page/component yet_ | Ask/chat thread history, citations, repo-scope controls, and chat UX parity | `specs/acceptance/ask.md` |
| Auth/admin/settings frontend | _No dedicated route shells yet_ | Onboarding, login, org/admin/settings discoverability parity | `specs/acceptance/auth.md` until dedicated UI specs land |

### 3. Worker and background-execution surface families

| Surface family | Rewrite evidence | Parity intent | Current acceptance home |
| --- | --- | --- | --- |
| One-shot review-agent worker entrypoint | `crates/worker/src/main.rs` → `main`, `run_worker_tick` | Worker parity starts from a single-shot entrypoint that loads config, constructs the organization store, invokes one worker tick, and logs either a terminal run or the no-work path | `specs/acceptance/index.md` until dedicated worker spec exists |
| Idle/no-work worker behavior | `crates/worker/src/main.rs` | Worker exposes an explicit no-work path by logging when no queued review-agent run is available | `specs/acceptance/index.md` until dedicated worker spec exists |
| Config-driven stub execution outcomes | `crates/worker/src/main.rs` | Worker entrypoint selects a configured stub outcome before invoking the tick, creating the acceptance anchor for later completed/failed worker parity slices | `specs/acceptance/index.md` until dedicated worker spec exists |
| Retry/scheduling/resume loops | _Not implemented yet_ | Real automation parity for retries, polling, rescheduling, and worker orchestration | Future worker acceptance spec |

### 4. Operator/admin workflow surface families

| Surface family | Rewrite evidence | Parity intent | Current acceptance home |
| --- | --- | --- | --- |
| First-run bootstrap workflow | `/api/v1/auth/bootstrap` in `crates/api/src/main.rs` | Operator can initialize a fresh instance safely and deterministically | `specs/acceptance/auth.md` |
| Local-dev organization-state path wiring | `crates/worker/src/main.rs` loads `AppConfig::from_env()` and builds the organization store from `config.organization_state_path` | Dev/runtime state-path behavior, startup tolerance, and migration-safe operator expectations | `specs/acceptance/index.md` until dedicated operator spec exists |
| Repo sync/index visibility | Repo `sync_state` exposed through API/UI surfaces in `crates/api/src/main.rs` and `web/src/App.tsx` | Operator/user parity for sync readiness and failure visibility | `specs/acceptance/browse.md` now; later dedicated sync/operator slices |
| Review webhook operational visibility | Authenticated review-webhook, delivery-attempt, and review-agent-run endpoints in `crates/api/src/main.rs` | Operator/admin inspection of automation state and failures | `specs/acceptance/index.md` until dedicated operator/review-webhook specs exist |
| Durable metadata / migrations / backup-restore | _Not implemented yet_ | Practical Sourcebot replacement requires production-grade persistence, migrations, backup, and restore parity | Future operator acceptance spec |

## Immediate follow-up gaps exposed by this index
1. The acceptance corpus has broad domain specs, but no dedicated worker/operator black-box specs yet.
2. Frontend parity surfaces are still mostly implicit inside `web/src/App.tsx`; later Task 1 slices should map them into explicit user journeys and missing UI shells.
3. `specs/FEATURE_PARITY.md` remains a checklist without per-row evidence placeholders; Task 2 should expand it after this index exists.
4. Review-agent, OAuth, analytics, audit, and settings/admin flows now have an index home but still need finer-grained acceptance expansion.

## Next recommended slice after this document
- Create the next Task 1 sub-slice that maps the indexed surfaces above into explicit user/admin/operator journeys and identifies which missing acceptance-spec documents must exist before implementation proceeds.
