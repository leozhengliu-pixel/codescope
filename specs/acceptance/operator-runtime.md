# Operator Runtime Acceptance

## Purpose
This acceptance spec defines the currently shipped operator-facing runtime baseline for local API and worker bring-up. It covers only black-box runtime liveness, bounded metadata readiness, public config visibility, authenticated worker status snapshot visibility, shared runtime-path wiring, and the documented local commands used to start the API plus the default one-tick/explicit bounded multi-tick worker.

## Scope
This document covers:
- `GET /healthz`
- `GET /readyz`
- `GET /api/v1/config`
- `GET /api/v1/auth/worker-status`
- `GET /api/v1/auth/repository-sync-jobs` sync-history visibility
- the shared `SOURCEBOT_DATA_DIR` runtime-path contract
- explicit per-file path overrides for bootstrap, local-session, and organization state
- local bring-up with `make api` and `make worker`
- optional `SOURCEBOT_WORKER_STATUS_PATH` preflight/fail-closed status snapshot writing

This document does **not** claim:
- complete metadata migration automation or durable metadata parity
- readiness checks beyond file-backed metadata readiness plus PostgreSQL SQLx-migration-inventory reachability
- supervised or continuously running workers
- durable worker scheduling, retries, or orchestration
- backup/restore workflows
- production-grade observability or deployment automation

## Environment contract
### Shared runtime data directory
When the operator sets `SOURCEBOT_DATA_DIR=/path/to/runtime`, the local runtime uses that directory as the default base for the current file-backed state:
- bootstrap state: `/path/to/runtime/bootstrap-state.json`
- local session state: `/path/to/runtime/local-sessions.json`
- organization state: `/path/to/runtime/organizations.json`

### Explicit path overrides
If the operator also sets any of these variables, the explicit value wins over `SOURCEBOT_DATA_DIR` for that specific file:
- `SOURCEBOT_BOOTSTRAP_STATE_PATH`
- `SOURCEBOT_LOCAL_SESSION_STATE_PATH`
- `SOURCEBOT_ORGANIZATION_STATE_PATH`

Operators may therefore keep one shared runtime directory by default while still overriding individual files when needed.

## Local runtime bring-up baseline
### API
Given a repo-local `.env` with `SOURCEBOT_DATA_DIR` and the other required environment values,
when the operator runs:
```bash
make api
```
then the API starts with the repo's `.env` contract and exposes the current local runtime baseline on the configured bind address.

### Worker
Given the same repo-local `.env`,
when the operator runs:
```bash
make worker
```
then the worker starts with the same environment contract, reads the organization-state path from the shared runtime baseline unless explicitly overridden, validates any configured `SOURCEBOT_WORKER_STATUS_PATH` before writing a supervisor-readable snapshot, processes one worker tick by default, and exits. Operators may explicitly set `SOURCEBOT_WORKER_MAX_TICKS=<positive integer up to 1000000>` and `SOURCEBOT_WORKER_IDLE_SLEEP_MS=<non-negative integer>` to run a bounded multi-tick loop before exit; oversized tick counts fail closed before execution, and an existing status-path directory, symlink target/parent, or existing file-valued parent fails closed before startup/status writes.

This worker baseline is intentionally limited to default one-tick local bring-up plus an explicit bounded multi-tick loop. It is not a supervised background service contract.

## Observable runtime behavior
### Liveness endpoint
Given the API is running,
when the operator requests `GET /healthz`,
then the service returns a successful liveness response suitable for confirming the process is up.

This endpoint is the current liveness baseline only. It does not claim dependency readiness, migration readiness, or downstream health verification.

### Readiness endpoint
Given the API is running without `DATABASE_URL`,
when the operator requests `GET /readyz`,
then the service returns `200` with `metadata_backend: "file"` and no database block, proving the file-backed local runtime baseline is available.

Given the API is running with `DATABASE_URL`,
when the operator requests `GET /readyz`,
then the service attempts a bounded PostgreSQL connection and reads the SQLx `_sqlx_migrations` inventory. It returns `200` with `metadata_backend: "postgres"`, `database.status: "ok"`, and an applied migration count when the migrated metadata database is reachable; otherwise it fails closed with `503`, `status: "degraded"`, and an error summary.

This endpoint is a bounded local readiness baseline for metadata reachability and migration-inventory visibility only. It does not yet claim richer dependency health, background-worker readiness, production observability, or upgrade orchestration.

### Public config endpoint
Given the API is running,
when the operator requests `GET /api/v1/config`,
then the service returns public runtime config needed by the frontend bootstrap surface while keeping secrets redacted to presence-only indicators rather than exposing raw secret values.

### Authenticated worker status endpoint
Given the API is running,
when an unauthenticated caller requests `GET /api/v1/auth/worker-status`,
then the service fails closed with `401` instead of exposing local runtime paths or status contents.

Given an authenticated local-session caller requests `GET /api/v1/auth/worker-status` and `SOURCEBOT_WORKER_STATUS_PATH` is unset or blank,
then the service returns `200` with `status: "not_configured"`, `configured: false`, and no snapshot.

Given an authenticated local-session caller requests `GET /api/v1/auth/worker-status` and `SOURCEBOT_WORKER_STATUS_PATH` points at a readable JSON worker snapshot,
then the service returns `200` with `status: "ok"`, `configured: true`, and the bounded JSON snapshot. Missing, invalid JSON, non-file, or oversized snapshots return a structured `status: "error"` response instead of panicking or reading unbounded data.

This endpoint is authenticated operator visibility for the local supervisor-readable worker status artifact only. The worker-side writer now fails closed when the configured status target contains control characters, is an existing directory, or has an existing parent that is a file, so the API is not asked to interpret silently misdirected status paths. It does not claim a production metrics API, durable worker metadata, a scheduler supervisor, retry orchestration, or broad observability parity.

### Authenticated repository-sync history
Given an authenticated local-session caller requests `GET /api/v1/auth/repository-sync-jobs`,
then each visible repository-sync job includes terminal metadata (`started_at`, `finished_at`, `error`, `synced_revision`, `synced_branch`, and `synced_content_file_count`) plus a derived `retryable` flag. The flag is `true` only for failed jobs visible to an organization admin when no queued or running job already exists for the same organization/repository/connection target, and `false` for non-admin callers, active jobs, and terminal jobs that the bounded manual retry API would currently reject.

This is operator-visible retry eligibility metadata for the sync-history API only. It does not claim automatic retry/backoff scheduling beyond the bounded worker behavior documented in the worker-runtime acceptance spec.

## Deferred follow-up areas
The following parity-facing operator concerns remain explicitly outside the shipped baseline documented here:
- complete database schema migration automation and durable metadata storage parity
- readiness probes beyond the bounded `/readyz` metadata-backend and SQLx migration-inventory check
- supervised worker lifecycle management
- durable worker metadata, retries, and resume loops
- production metrics, tracing, alerting, and other production-grade observability
- recovery, backup, restore, and upgrade-safe deployment procedures
