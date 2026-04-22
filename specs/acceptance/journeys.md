# Acceptance Journey Map: User, Admin, and Operator Parity Follow-Ups

## Purpose
This document turns the surface inventory in `specs/acceptance/index.md` into concrete parity-facing journeys. It groups the currently indexed rewrite routes, page shells, worker entrypoints, and operator-visible surfaces into user, admin, and operator flows, names the current acceptance-spec home for each flow, and identifies the dedicated missing acceptance-spec documents that must exist before broader implementation work continues.

## Grounding and limits
- This is an acceptance-audit map, not a shipped-feature claim.
- Journeys below are grounded only in the current rewrite evidence already indexed from:
  - `crates/api/src/main.rs`
  - `web/src/App.tsx`
  - `crates/worker/src/main.rs`
  - `crates/worker/src/lib.rs`
- When a route or page shell exists without a dedicated acceptance spec or full UI flow, this document marks it as a follow-up instead of treating it as complete behavior.

## Journey map summary

| Journey family | Concrete journey | Indexed rewrite surfaces | Current acceptance-spec home | Required dedicated follow-up before broad implementation proceeds |
| --- | --- | --- | --- | --- |
| User | Search accessible code across visible repositories | `#/search`; `SearchPage`; `/api/v1/search`; repo list/detail shells as navigation anchors | `specs/acceptance/search.md` | None for the base search journey; later parity work can extend the existing spec |
| User | Open a repository, browse the tree, and read source | `#/`; `#/repos/:repoId`; `RepoListPage`; `RepoDetailPage`; `BrowsePanel`; `/api/v1/repos`; `/api/v1/repos/{repo_id}`; `/api/v1/repos/{repo_id}/tree`; `/api/v1/repos/{repo_id}/blob` | `specs/acceptance/browse.md` | None for the base browse journey; later parity work can extend the existing spec |
| User | Inspect commits and diffs for one repository | `CommitsPanel`; `/api/v1/repos/{repo_id}/commits`; `/api/v1/repos/{repo_id}/commits/{commit_id}`; `/api/v1/repos/{repo_id}/commits/{commit_id}/diff` | `specs/acceptance/browse.md` | None immediately; a dedicated commit spec may still be useful later if commit parity outgrows the current browse acceptance home |
| User | Navigate by symbol definitions and references from source views | `BrowsePanel`; `/api/v1/repos/{repo_id}/definitions`; `/api/v1/repos/{repo_id}/references` | `specs/acceptance/code-nav.md` | None for the base code-navigation journey; later parity work can extend the existing spec |
| User | Ask questions about code and receive cited answers | `/api/v1/ask/completions` | `specs/acceptance/ask.md` | None immediately; the existing ask acceptance home already covers thread lifecycle at a black-box level and can be split later if needed |
| User | Sign in, restore session, and sign out | `/api/v1/auth/login`; `/api/v1/auth/me`; `/api/v1/auth/logout`; `/api/v1/auth/revoke` | `specs/acceptance/auth.md` | None for the local-auth baseline; later parity work can extend the existing spec |
| Admin | Bootstrap the first local admin on a fresh instance | `/api/v1/auth/bootstrap` | `specs/acceptance/auth.md` | None immediately; the existing auth acceptance home already covers first-run onboarding and can be split later if the onboarding surface needs a dedicated spec |
| Admin | Create and revoke API keys | `/api/v1/auth/api-keys`; `/api/v1/auth/api-keys/{api_key_id}/revoke` | `specs/acceptance/auth.md` | None immediately; may remain an `auth.md` expansion until settings/admin docs split out |
| Admin | Manage saved search contexts for authenticated use | `/api/v1/auth/search-contexts` | `specs/acceptance/search.md` | None immediately; may remain a `search.md` expansion until a dedicated contexts spec is needed |
| Admin | Inspect audit and analytics surfaces | `/api/v1/auth/audit-events`; `/api/v1/auth/analytics` | `specs/acceptance/index.md` only | Create `specs/acceptance/admin-observability.md` before implementing richer audit/analytics/admin settings behavior |
| Admin | Manage OAuth clients | `/api/v1/auth/oauth-clients` | `specs/acceptance/index.md` only | Create `specs/acceptance/oauth-clients.md` before broader OAuth client/token implementation proceeds |
| Admin | Inspect review webhooks, delivery attempts, and review-agent runs from authenticated endpoints | `/api/v1/auth/review-webhooks`; `/api/v1/auth/review-webhooks/{webhook_id}`; `/api/v1/auth/review-webhooks/{webhook_id}/delivery-attempts`; `/api/v1/auth/review-webhooks/{webhook_id}/delivery-attempts/{attempt_id}`; `/api/v1/auth/review-webhooks/{webhook_id}/review-agent-runs`; `/api/v1/auth/review-webhooks/{webhook_id}/review-agent-runs/{run_id}`; `/api/v1/auth/review-webhook-delivery-attempts`; `/api/v1/auth/review-webhook-delivery-attempts/{attempt_id}`; `/api/v1/auth/review-agent-runs`; `/api/v1/auth/review-agent-runs/{run_id}` | `specs/acceptance/index.md` only | Create `specs/acceptance/review-automation.md` before implementation broadens across webhook admin UX, run history, retries, and operator diagnostics |
| Admin | Use auth/admin/settings navigation shells | `web/src/App.tsx` now includes `#/settings` plus shared-shell subsection routes for connections, API keys, OAuth clients, observability, and review automation; broader onboarding/login/admin route families are still absent | `specs/acceptance/settings-navigation.md` and `specs/acceptance/auth.md` | None for the current route-shell baseline; extend `specs/acceptance/settings-navigation.md` before broader settings/admin frontend implementation proceeds |
| Operator | Confirm service liveness and frontend bootstrap config | `/healthz`; `/api/v1/config` | `specs/acceptance/operator-runtime.md` | None for the current local liveness/config baseline; extend the existing spec before claiming broader deployment/runtime behavior |
| Operator | Run the one-shot worker tick against organization state path wiring | `crates/worker/src/main.rs` `main`; `AppConfig::from_env()`; `build_organization_store(config.organization_state_path.clone())`; `run_worker_tick(...)`; startup runtime-baseline log fields for resolved path and stub selections | `specs/acceptance/worker-runtime.md` | None for the current one-shot stub baseline; extend `specs/acceptance/worker-runtime.md` before broader worker orchestration work proceeds |
| Operator | Observe worker no-work and configured stub terminal outcomes | `StubReviewAgentRunExecutionOutcome`; `StubRepositorySyncJobExecutionOutcome`; worker logs for startup runtime baseline, terminal status, and `no queued review-agent run or repository sync job available`; `crates/worker/src/lib.rs` stub execution helpers | `specs/acceptance/worker-runtime.md` | None for the current stub/logging baseline; later slices should extend the same worker-runtime acceptance home instead of creating a second baseline doc |
| Operator | Run the local operator maintenance baseline | `scripts/backup_local_runtime_state.sh`; `scripts/restore_local_runtime_state.sh`; `Makefile` `runtime-backup`/`runtime-restore`; README local maintenance runbook | `specs/acceptance/operator-maintenance.md` | None for the current file-backed backup/restore plus local SQLx maintenance baseline; extend `specs/acceptance/operator-maintenance.md` before claiming durable metadata or production deployment parity |
| Operator | Accept public review-webhook events into the automation pipeline | `/api/v1/review-webhooks/{webhook_id}/events` plus the authenticated inspection endpoints above | `specs/acceptance/index.md` only | Covered by the same required `specs/acceptance/review-automation.md` follow-up |
| Operator | Track repo sync/index readiness exposed to users and admins | Repo `sync_state` surfaced through `/api/v1/repos`, `/api/v1/repos/{repo_id}`, the repo pages/panels in `web/src/App.tsx`, and authenticated sync-job history at `/api/v1/auth/repository-sync-jobs` | `specs/acceptance/repository-operations.md` | None for the dedicated acceptance home itself; later parity work should extend this spec for persisted index status, failure recovery, and operator controls |

## Journey details

### User journeys
1. **Search journey** already has a clear acceptance home in `specs/acceptance/search.md`, now anchored by both the dedicated `#/search` page shell and the current `/api/v1/search` route.
2. **Browse/source journey** already has a clear acceptance home in `specs/acceptance/browse.md`, anchored by the repo list/detail routes and `RepoListPage`/`RepoDetailPage`/`BrowsePanel` shells.
3. **Commit journey** is already covered at a black-box level inside `specs/acceptance/browse.md`; a dedicated commit spec may still become useful later if commit parity work outgrows that shared acceptance home.
4. **Code-navigation journey** already has a dedicated home in `specs/acceptance/code-nav.md`.
5. **Ask journey** already has a black-box acceptance home in `specs/acceptance/ask.md`, including thread lifecycle expectations, even though the current rewrite evidence still centers on `/api/v1/ask/completions`.
6. **Local auth journey** has a baseline home in `specs/acceptance/auth.md` for login/session/logout behavior.

### Admin journeys
1. **First-run onboarding** is currently housed in `specs/acceptance/auth.md`; that existing acceptance home is sufficient for now, and a dedicated onboarding doc would be an optional future split rather than a current prerequisite.
2. **API keys** and **search contexts** can continue as expansions of `auth.md` and `search.md` for now because those docs already exist and match the current route evidence.
3. **Audit/analytics**, **OAuth clients**, and **review automation visibility** still do not yet have dedicated acceptance homes. **Settings/admin navigation** now has a dedicated route-shell acceptance home in `specs/acceptance/settings-navigation.md`, but richer auth/admin flows still remain follow-up work.

### Operator journeys
1. **Runtime liveness/config** now has a dedicated acceptance home in `specs/acceptance/operator-runtime.md` for the current local `/healthz`, `/api/v1/config`, shared `SOURCEBOT_DATA_DIR`, and `make api`/`make worker` baseline without over-claiming broader deployment parity.
2. **Worker runtime** now has a dedicated home in `specs/acceptance/worker-runtime.md` for the one-shot entrypoint, startup runtime-baseline logging, stubbed terminal outcomes, and explicit no-work logging. That baseline still does not claim retries, scheduling loops, or full automation orchestration.
3. **Operator maintenance baseline** now has a dedicated acceptance home in `specs/acceptance/operator-maintenance.md` for the shipped file-backed runtime backup/restore helpers plus the current local SQLx migration and upgrade runbook, without over-claiming durable metadata backup/restore parity.
4. **Review webhook intake and run inspection** spans both operator and admin concerns; the current authenticated/public route set should be moved into a dedicated review-automation acceptance doc before broader implementation.
5. **Repository operations** now has a dedicated acceptance home in `specs/acceptance/repository-operations.md`; later sync/index parity work should extend that doc instead of overloading `browse.md`.

## Minimum missing acceptance-spec set exposed by the current journey audit
These are the smallest still-missing dedicated follow-up docs this journey map shows as prerequisites for broader parity implementation in the indexed gaps:

1. `specs/acceptance/admin-observability.md`
2. `specs/acceptance/oauth-clients.md`
3. `specs/acceptance/review-automation.md`

`specs/acceptance/worker-runtime.md` is now present; later worker slices should extend that existing acceptance home instead of reopening the gap elsewhere.

## Immediate usage rule for later slices
- Extend an existing domain spec when this document says an acceptance home already exists.
- Create the named dedicated follow-up spec before broad implementation in any journey currently anchored only in `specs/acceptance/index.md` or in an obviously overloaded domain doc.
- Do not treat missing UI shells or worker loops as shipped behavior just because a route, panel, or stub worker path already exists.