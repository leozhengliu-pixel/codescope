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
- Authenticated repository sync-job history visibility plus bounded admin enqueue/retry controls for already visible repositories
- Visible-repository index-status counts for the bounded startup-built local in-memory search index
- Operator/admin-facing expectations for repository readiness, failure visibility, and deferred parity gaps

## Current rewrite evidence anchors
- `/api/v1/repos` returns repository summaries that already include `sync_state`.
- `/api/v1/repos/{repo_id}` returns repository detail that includes repository metadata plus `sync_state` and connection metadata.
- `#/` in `web/src/App.tsx` renders repository cards with a visible sync-state badge.
- `#/repos/:repoId` in `web/src/App.tsx` renders repository detail with a visible sync-state field plus a read-only index-status panel.
`/api/v1/auth/repository-sync-jobs` returns authenticated sync-job history filtered to the caller's currently visible `(organization_id, repository_id)` bindings and sorted newest-first; when `DATABASE_URL` is configured, that filtering uses PostgreSQL-backed organization membership and repository-permission metadata even though the sync-job records themselves still come from the configured organization-state store. The same route now accepts a bounded admin-only `POST` for an already visible repository, derives `connection_id` from the catalog detail instead of caller input, and appends a queued job without letting non-admin callers mutate the queue. `POST /api/v1/auth/repository-sync-jobs/{job_id}/retry` requeues admin-visible failed jobs by appending a fresh queued job while preserving the original terminal history row.
- `/api/v1/repos/{repo_id}/index-status` returns the search store's current startup-built local in-memory index status for visible repositories, including `indexed`, `indexed_empty`, or `error` state, indexed file count, indexed line count, skipped file count, and error text when startup indexing failed for that repo; if the startup search store has no status and the latest authorized local sync has a `search-index.json` artifact, the endpoint reports the artifact-backed counts before falling back to pre-artifact snapshot traversal. Hidden or unknown repositories fail closed with `404` through the same repository-visibility gate as browse/search.
- The catalog-backed repo list/detail path now has bounded PostgreSQL reads for `/api/v1/repos` and repository detail lookup when `DATABASE_URL` is configured, and the search store now exposes truthful in-memory index counts, but that does not yet close persisted index-status, retry, or full repository-operations parity.

## Inputs
- Repository identifier for repo-detail visibility
- Authenticated local-session context for repository sync-job history and bounded admin enqueue requests
- Current repository catalog metadata including repository `sync_state`
- Search store startup index metadata for configured local repository roots
- Persisted organization-state sync-job records (`queued`, `running`, `succeeded`, `failed`) where available, filtered by the caller's live auth metadata

## Expected behavior
1. Repository list surfaces show each visible repository's current `sync_state` as user-visible readiness metadata.
2. Repository detail surfaces show repository metadata, the same `sync_state`, and read-only index-status counts without requiring a separate operator-only page.
3. Authenticated repository sync-job history is available from an endpoint that returns only jobs visible to the current user and lets organization admins enqueue a queued sync job only for an already visible repository or append a queued retry for an already visible failed job.
4. Sync-job history ordering is newest-first by queue time so operators/admins can inspect recent activity first.
5. Sync-job responses expose operator-relevant status metadata including job id, organization/repository ids, status, timestamps, and error text when present.
6. Hidden repositories and cross-org duplicate repository ids must not leak through sync-job history visibility.
7. The repository index-status endpoint reports the bounded search-store baseline or, when the startup store has no status, the latest authorized local sync `search-index.json` artifact/pre-artifact snapshot fallback, not catalog `sync_state`: `indexed` for repositories with searchable files, `indexed_empty` for successfully scanned repositories with zero searchable files, or `error` with indexed file count, indexed line count, skipped file count, and optional error text.
8. If the rewrite is running on the current bounded Postgres catalog read path, repository list/detail parity for sync/index visibility is not yet considered complete simply because catalog summaries/details are durable; persisted index-status and operator controls remain follow-up work.

## Permission behavior
- Repository sync-job history requires an authenticated local session.
- Repository sync-job creation requires an authenticated organization admin, a non-empty organization/repository payload, a live repository visibility binding for that `(organization_id, repository_id)`, and an existing catalog repository detail from which the connection id is derived.
- Users only see sync jobs whose organization membership and repository visibility both allow access; with `DATABASE_URL`, those authorization bindings are read from PostgreSQL-backed auth metadata instead of trusting stale organization-state fixtures.
- Repository index-status reads use the same repository-visibility gate as browse/search and must return `404` for hidden, unauthorized, or unknown repositories.
- Unauthorized or hidden repository operations must fail closed rather than leaking existence through sync-job history or repo metadata.

## Edge cases
- Duplicate repository ids across different organizations must still filter correctly by `(organization_id, repository_id)`.
- Non-admin sync-job creation/retry attempts and non-failed retry targets fail closed without mutating persisted sync-job state.
- Repositories can expose a coarse `sync_state` alongside the bounded in-memory search index-status counts; those are separate surfaces and neither should be treated as the other's source of truth.
- A configured `DATABASE_URL`, bounded catalog reads, the startup-built in-memory index-status endpoint, the local sync artifact-backed requested-repo search/index-status, unscoped/search-context search fallback, snapshot tree/blob/code-navigation fallback, and local-Git commit list/detail/diff fallback scoped to the caller-authorized `(organization_id, repository_id)` boundary, and the manual failed-job retry endpoint do not by themselves prove repository-operations parity; persisted index-status, background reindexing, queue depth, automated retry/backoff, failure-recovery, and operator-control surfaces remain open.
- Sync-job history may show terminal failures while the repo list/detail UI still exposes only coarse readiness state.
- Startup indexing skips ignored directories, binary files, and oversize files and records counts, reports `indexed_empty` when the scan succeeds but produces zero searchable files, and does not rescan changed files until the API/search store is rebuilt.

## Black-box examples
- Opening `#/` shows repository cards with visible sync-state badges such as `ready`, `pending`, or `error`.
- Opening `#/repos/:repoId` shows the repository detail panel with a sync-state field, connection metadata, and an Index status panel with index state plus indexed-file, indexed-line, and skipped-file counts.
- An authenticated user calling `/api/v1/auth/repository-sync-jobs` receives only the newest-first jobs for repositories they can currently access.
- An authenticated organization admin can post `{"organization_id":"org_acme","repository_id":"repo_visible"}` to `/api/v1/auth/repository-sync-jobs` and receive a `queued` job whose `connection_id` comes from the visible repository's catalog detail; for a visible failed job, the admin can post to `/api/v1/auth/repository-sync-jobs/{job_id}/retry` and receive a fresh queued retry while the failed history row remains unchanged.
- A user who loses repository visibility no longer sees that repository's sync jobs in the authenticated history endpoint.
- An authenticated user calling `/api/v1/repos/{repo_id}/index-status` for a visible repository receives the current startup-built in-memory index state/counts, including `indexed_empty` for a successfully scanned repository with no searchable files, or the latest authorized local sync `search-index.json` artifact counts when the startup store has no status; the same call for a hidden repository returns `404`.

## Deferred parity gaps locked by this spec
This spec intentionally records the remaining gaps instead of over-claiming parity:

1. **Persisted repository-operations parity is still partial.** `PgCatalogStore::list_repositories()` and `PgCatalogStore::get_repository_detail()` now provide bounded durable catalog reads, the search store exposes startup-built in-memory index counts, requested-repo index-status/search can fall back to the latest authorized local sync artifact, requested-repo no-revision tree/blob/code-navigation reads can fall back to the latest authorized local sync snapshot, requested-repo commit list/detail/diff can fall back to the latest authorized local Git sync job, and admins can manually requeue visible failed jobs, but persisted index-status, richer last-run/failure detail, automated retry/backoff, and management semantics are still incomplete.
2. **Index-status parity is still bounded.** The rewrite now provides a truthful search-store-backed index-status API for the local startup-built in-memory index, but not a Tantivy-backed, persisted, background-updated, retry-aware, queue-depth-aware, or production-grade repository indexing surface.
3. **Frontend repository-operations parity is still shallow.** The current UI shows coarse sync-state badges/fields, repository-detail index-status counts, read-only history, and bounded commit-history previous/next pagination controls, but not last-run timestamps, queue depth, failure history management, retry controls, or settings-level operator surfaces.
4. **Worker/recovery parity is still incomplete.** The current worker slices only prove stub sync-job transitions and read-only visibility; real fetch/mirror execution, retries, recovery, and rescheduling remain deferred.
5. **Settings/admin navigation parity is still incomplete.** The rewrite has a limited `#/settings/connections` settings shell, but it still lacks a broader auth/admin/settings route family that exposes repository operations as a first-class management surface.

## Acceptance-evidence anchors for future slices
- Use this spec as the acceptance home for roadmap Task 18 follow-ups and the related UI/admin follow-up work in Tasks 19 and 55.
- Extend this document, rather than `specs/acceptance/browse.md`, when future slices add persisted index status, sync history, failure recovery, retry controls, or repository-operations settings/admin surfaces.
