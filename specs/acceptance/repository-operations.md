# Acceptance Spec: Repository Operations

## Purpose
This document creates the dedicated black-box acceptance home for repository sync/index visibility parity. It freezes the current rewrite contract around repository readiness metadata, authenticated sync-job history visibility, and the operator/admin parity gaps that still remain before Sourcegraph/Sourcebot-style repository operations are complete.

## Grounding and limits
- This is an acceptance contract for the currently evidenced repository-operations surface, not a claim that full sync/index parity is already shipped.
- Grounding for this document is limited to live rewrite evidence in:
  - `crates/api/src/main.rs`
  - `crates/api/src/storage.rs`
  - `web/src/App.tsx`
  - `specs/acceptance/index.md`
  - `specs/acceptance/journeys.md`
  - `specs/FEATURE_PARITY.md`
  - `docs/reports/2026-04-18-parity-gap-report.md`
  - `docs/plans/2026-04-18-sourcebot-full-parity-roadmap.md`
- When the current rewrite exposes only baseline sync metadata or read-only history, this spec records that truthfully and defers richer operator controls, retries, recovery, and durable catalog parity to later slices.

## Scope
- Public repository list and repository detail sync-state visibility
- Authenticated read-only repository sync-job history visibility
- Operator/admin-facing expectations for repository readiness, failure visibility, and deferred parity gaps

## Current rewrite evidence anchors
- `/api/v1/repos` returns repository summaries that already include `sync_state`.
- `/api/v1/repos/{repo_id}` returns repository detail that includes repository metadata plus `sync_state` and connection metadata.
- `#/` in `web/src/App.tsx` renders repository cards with a visible sync-state badge.
- `#/repos/:repoId` in `web/src/App.tsx` renders repository detail with a visible sync-state field.
- `/api/v1/auth/repository-sync-jobs` returns authenticated read-only sync-job history filtered to the caller's currently visible `(organization_id, repository_id)` bindings and sorted newest-first.
- The catalog-backed repo list/detail path still depends on `CatalogStore::{list_repositories,get_repository_detail}`; `PgCatalogStore` keeps both queries unimplemented, so persisted-catalog repository-status parity is not closed yet.

## Inputs
- Repository identifier for repo-detail visibility
- Authenticated local-session context for repository sync-job history
- Current repository catalog metadata including repository `sync_state`
- Persisted organization-state sync-job records (`queued`, `running`, `succeeded`, `failed`) where available

## Expected behavior
1. Repository list surfaces show each visible repository's current `sync_state` as user-visible readiness metadata.
2. Repository detail surfaces show repository metadata plus the same `sync_state` without requiring a separate operator-only page.
3. Authenticated repository sync-job history is available from a read-only endpoint that returns only jobs visible to the current user.
4. Sync-job history ordering is newest-first by queue time so operators/admins can inspect recent activity first.
5. Sync-job responses expose operator-relevant status metadata including job id, organization/repository ids, status, timestamps, and error text when present.
6. Hidden repositories and cross-org duplicate repository ids must not leak through sync-job history visibility.
7. If the rewrite is running on the current Postgres catalog skeleton, repository list/detail parity for sync/index visibility is not yet considered complete simply because the in-memory path works.

## Permission behavior
- Repository sync-job history requires an authenticated local session.
- Users only see sync jobs whose organization membership and repository visibility both allow access.
- Unauthorized or hidden repository operations must fail closed rather than leaking existence through sync-job history or repo metadata.

## Edge cases
- Duplicate repository ids across different organizations must still filter correctly by `(organization_id, repository_id)`.
- Repositories can expose a coarse `sync_state` even while richer index-status and last-run details remain absent.
- A configured `DATABASE_URL` does not itself prove repository-operations parity; the persisted catalog path must implement repo list/detail queries before status surfaces are considered durable.
- Sync-job history may show terminal failures while the repo list/detail UI still exposes only coarse readiness state.

## Black-box examples
- Opening `#/` shows repository cards with visible sync-state badges such as `ready`, `pending`, or `error`.
- Opening `#/repos/:repoId` shows the repository detail panel with a sync-state field and connection metadata.
- An authenticated user calling `/api/v1/auth/repository-sync-jobs` receives only the newest-first jobs for repositories they can currently access.
- A user who loses repository visibility no longer sees that repository's sync jobs in the authenticated history endpoint.

## Deferred parity gaps locked by this spec
This spec intentionally records the remaining gaps instead of over-claiming parity:

1. **Persisted catalog parity is still blocked.** `PgCatalogStore::list_repositories()` and `PgCatalogStore::get_repository_detail()` remain unimplemented, so repo-status visibility is not yet truthful on the durable catalog path.
2. **Index-status parity is still incomplete.** The rewrite does not yet provide a truthful persisted index-status API equivalent for repository operations; task18b remains blocked on the same catalog prerequisite.
3. **Frontend repository-operations parity is still shallow.** The current UI shows coarse sync-state badges/fields, but not last-run timestamps, queue depth, failure history, retry controls, or settings-level operator surfaces.
4. **Worker/recovery parity is still incomplete.** The current worker slices only prove stub sync-job transitions and read-only visibility; real fetch/mirror execution, retries, recovery, and rescheduling remain deferred.
5. **Settings/admin navigation parity is still incomplete.** The rewrite has a limited `#/settings/connections` settings shell, but it still lacks a broader auth/admin/settings route family that exposes repository operations as a first-class management surface.

## Acceptance-evidence anchors for future slices
- Use this spec as the acceptance home for roadmap Task 18 follow-ups and the related UI/admin follow-up work in Tasks 19 and 55.
- Extend this document, rather than `specs/acceptance/browse.md`, when future slices add persisted index status, sync history, failure recovery, retry controls, or repository-operations settings/admin surfaces.
