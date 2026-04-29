# CodeScope

CodeScope is a self-hosted code search, code navigation, repository browsing, and codebase Q&A application built as a clean-room implementation with a Rust backend and React frontend.

## Project goals
- New implementation, not a fork.
- New architecture and new primary backend language.
- Broad feature parity with Sourcebot behavior and public contract surface.
- Permissive open-source licensing.
- Lower runtime complexity and better efficiency for self-hosting.

## Proposed stack
- Backend: Rust + Axum + Tokio
- Frontend: React + TypeScript + Vite
- Metadata DB: PostgreSQL
- Search/indexing: Rust service targeting Tantivy + regex-automata + tree-sitter; the current bounded local baseline builds an in-memory line index at API startup for configured local repository roots, exposes explicit `mode=boolean|literal|regex` search modes through the API and `#/search` UI, matches local/index-artifact boolean searches case-insensitively with all-term semantics for space-separated query words, quoted-phrase components, a bounded unquoted `OR` operator between term groups, and minimal unquoted `lang:`/`path:` filters, and returns deterministic repo/path/line ordering for identical indexed content.
- Object storage: S3-compatible

## Clean-room rule
This repository must not copy upstream Sourcebot code, prompts, tests, schema internals, UI assets, or prose. Implementation must follow the clean spec in `specs/` and the plan in `docs/plans/`.

## Initial documents
- `docs/plans/2026-04-14-sourcebot-rewrite-plan.md`
- `specs/FEATURE_PARITY.md`
- `specs/CLEAN_ROOM_RULES.md`

## Local metadata DB bootstrap
1. Copy the example env file for the deterministic local Postgres defaults:
   ```bash
   cp .env.example .env
   ```
2. Start the local Postgres service:
   ```bash
   make dev-up
   ```
3. Run the SQLx metadata-schema migrations:
   ```bash
   make sqlx-migrate
   ```
4. Reset the dedicated deterministic local test database when a local test run needs a clean metadata schema:
   ```bash
   make sqlx-test-reset
   ```
5. Run the focused metadata-schema test wrapper:
   ```bash
   make sqlx-test
   ```
6. Or run the single local metadata bootstrap wrapper:
   ```bash
   make metadata-dev-bootstrap
   ```
7. `make` auto-loads `.env`, so `.env.example` stays the runnable local metadata DB contract for both the local-only `sourcebot` bootstrap database and the dedicated `sourcebot_test` test database.
8. `make sqlx-test-reset` uses `TEST_DATABASE_URL` plus the repo-local `.sqlx-cli` install root to drop, recreate, and re-migrate the deterministic local test database.
9. `make sqlx-test` wraps the deterministic reset plus the focused `sourcebot-api` metadata storage, PostgreSQL-backed catalog list/detail/local-import handoff plus local-import sync-job enqueue, PostgreSQL-backed bootstrap-admin, durable local-session, PostgreSQL-backed local-account/membership/invite auth, PostgreSQL-backed repository-permission filtering for authenticated sync-job history, PostgreSQL-backed repository-sync-job lifecycle storage, PostgreSQL-backed review-agent-run and delivery-attempt lifecycle storage, PostgreSQL-backed ask-thread/message storage, and durable API-key/OAuth-client metadata regressions so local migration workflow verification uses one reproducible command.
10. `make sqlx-test` now covers the storage migration-inventory/catalog fallback checks, PostgreSQL-backed catalog list/detail queries for `/api/v1/repos` and repository detail reads, one explicitly requested local Git repository import handoff into the PostgreSQL catalog plus the file-backed admin-visible repository-permission binding and queued repository-sync job that hands the import to the worker baseline, the PostgreSQL-backed bootstrap store and `/api/v1/auth/bootstrap` + login regressions, the PostgreSQL-backed local-session store and login -> `/api/v1/auth/me` regression slice, the durable PostgreSQL local-account lookup/linked-account membership/member-roster/invite-redeem regressions, authenticated `/api/v1/auth/repository-sync-jobs` filtering against PostgreSQL-backed repository permissions, PostgreSQL-backed repository-sync-job upsert/claim/complete lifecycle regressions, PostgreSQL-backed review-agent-run store/merge/claim/complete/fail lifecycle regressions, PostgreSQL-backed review-webhook delivery-attempt store/merge regressions, PostgreSQL-backed ask-thread create/append/list/detail owner-scoped regressions, plus PostgreSQL-backed API-key inventory/create/revoke/bearer-auth and OAuth-client inventory/create regressions; catalog read metadata, one explicit local import handoff, bootstrap-admin, invited-account login, auth-me identity restoration, member rosters, linked-account memberships, invite acceptance, sync-job permission filtering and lifecycle rows, review-agent-run lifecycle rows, review-webhook delivery-attempt rows, ask-thread messages, API-key metadata, and OAuth-client metadata now all stay durable across API restarts when `DATABASE_URL` is configured.
11. `make sqlx-test-reset` refuses non-local or non-`_test` databases so the destructive reset flow stays scoped to the dedicated local metadata test database.
12. `make metadata-dev-bootstrap` is a local-only operator bootstrap helper that waits for local Postgres, ensures the dedicated test database exists, runs `make sqlx-migrate`, and then runs the focused `make sqlx-test` compatibility check.
13. `make metadata-dev-bootstrap` now exercises a bounded PostgreSQL catalog read path plus the durable auth metadata slices; catalog list/detail reads and one explicitly requested local Git repository import handoff use PostgreSQL when `DATABASE_URL` is configured; the authenticated local import route now also adds the imported repository to the admin organization visibility set and queues one repository-sync job for the existing default one-tick worker baseline, while broader connection management durability, analytics/audit aggregates, recursive/provider import, reindex execution, and broader organization aggregates still remain follow-up work.

## Local operator runtime baseline
1. Copy the example env file so `make` can auto-load the repo-local runtime contract:
   ```bash
   cp .env.example .env
   ```
2. Set `SOURCEBOT_DATA_DIR` in `.env` to the directory that should hold the current local runtime state. When only that shared base is set, the API and worker derive:
   - `bootstrap-state.json`
   - `local-sessions.json`
   - `organizations.json`
3. API-key inventory/create/revoke and OAuth-client inventory/create now restore from PostgreSQL-backed durable metadata when DATABASE_URL is configured. PostgreSQL-backed catalog list/detail reads also now power `/api/v1/repos` and repository detail lookup when `DATABASE_URL` is configured, and the catalog can persist one explicitly requested local Git repository import by upserting the local connection plus repository row in PostgreSQL. Repository-sync-job lifecycle rows are durably stored, atomically claimed, and completed in PostgreSQL when `DATABASE_URL` is configured. Review-agent-run lifecycle rows are durably stored, claimed, completed, and failed in PostgreSQL when `DATABASE_URL` is configured. Review-webhook delivery-attempt rows are durably stored and read from PostgreSQL when `DATABASE_URL` is configured. Ask-thread metadata and messages are stored in PostgreSQL when `DATABASE_URL` is configured, while the broader organization aggregate remains file-backed. The search store builds a real in-memory line index once at API startup over configured local repository roots, skips ignored directories/binary/oversize files, serves `/api/v1/search` from that index without rewalking the filesystem per query, supports explicit `mode=boolean|literal|regex` matching across startup and local-sync artifact fallback paths, exposes those modes through the dedicated `#/search` UI while preserving deep-link context, exposes visible-repository counts at `/api/v1/repos/{repo_id}/index-status` and the repository detail UI, and provides bounded top-level Rust plus TypeScript/JavaScript symbol extraction for definitions/code navigation. This is still not Tantivy, a persistent index, background reindexing, queue-depth reporting, retries, tree-sitter precision, or production-grade indexing. The same bounded auth slice persists bootstrap-admin, local-session, local-account, membership, and invite-auth metadata durably in PostgreSQL for `/api/v1/auth/login`, `/api/v1/auth/me`, `/api/v1/auth/members`, `/api/v1/auth/linked-accounts`, `/api/v1/auth/invite-redeem`, `/api/v1/auth/api-keys`, and `/api/v1/auth/oauth-clients`; authenticated `/api/v1/auth/repository-sync-jobs` filters sync-job history through PostgreSQL-backed organization membership and repository-permission bindings when `DATABASE_URL` is configured, while connection management, analytics, audit events, recursive/provider repository import, richer review-agent orchestration/retries, and full organization aggregate durability still remain follow-up work.
4. Optional explicit overrides still win for the file-backed state paths if you set `SOURCEBOT_BOOTSTRAP_STATE_PATH`, `SOURCEBOT_LOCAL_SESSION_STATE_PATH`, or `SOURCEBOT_ORGANIZATION_STATE_PATH`.
5. Start the API with the repo-local `.env` contract:
   ```bash
   make api
   ```
6. In a second shell, run the current worker baseline with that same `.env` contract:
   ```bash
   make worker
   ```
7. `make worker` defaults to a one-tick local bring-up path, but the worker can also run a bounded multi-tick loop with `SOURCEBOT_WORKER_MAX_TICKS=<n>` and `SOURCEBOT_WORKER_IDLE_SLEEP_MS=<ms>`. The worker logs the startup runtime baseline once, then each tick recovers stale `running` repository-sync jobs after a one-hour lease before the next claim, automatically queues one bounded retry for stale-lease failures after another one-hour backoff when no queued/running or prior automatic replacement exists, runs a timeout-bounded `git -C <repo_path> rev-parse --is-inside-work-tree` preflight plus bounded `git rev-parse HEAD`, `git ls-tree -r --name-only HEAD`, and `git symbolic-ref --short HEAD` probes before completing repository-sync jobs tied to configured `local` connections, writes a bounded repo-local read-only tracked-content manifest at `.sourcebot/local-sync/<organization_id>/<repository_id>/<job_id>/manifest.txt` plus sibling `snapshot/` and `search-index.json` artifacts on successful local sync, persists the successful terminal revision, current branch, and tracked-content file count on those sync jobs, lets explicitly scoped `/api/v1/search?repo_id=...` requests, unscoped/search-context `/api/v1/search` requests across visible repositories, and `/api/v1/repos/{repo_id}/index-status` load the latest successful local sync `search-index.json` artifact when present (falling back to the snapshot for pre-artifact rows), while no-revision `/api/v1/repos/{repo_id}/tree`/`blob` reads and no-revision `/api/v1/repos/{repo_id}/definitions`/`references` code-navigation reads use the latest successful local sync snapshot under the caller-authorized `(organization_id, repository_id)` boundary when the startup search/browse/code-navigation stores have no indexed rows/status, browseable tree/blob, or primary blob for that visible repository, fails closed with an operator-visible preflight or execution error if those Git checks hang or cannot prove a real working tree with a readable current revision, at least one tracked content path, and current branch, logs repository-sync terminal job identifiers and failure reasons when a sync job is processed, and logs bounded runtime completion before exiting.
8. Optional worker-only controls for the current baseline are:
   - `SOURCEBOT_STUB_REVIEW_AGENT_RUN_EXECUTION_OUTCOME=completed|failed`
   - `SOURCEBOT_STUB_REPOSITORY_SYNC_JOB_EXECUTION_OUTCOME=succeeded|failed`
   - `SOURCEBOT_WORKER_MAX_TICKS=<positive integer>` (defaults to `1`)
   - `SOURCEBOT_WORKER_IDLE_SLEEP_MS=<non-negative integer>` (defaults to `1000`)
   - `SOURCEBOT_WORKER_STATUS_PATH=<path>` (optional; preflights and writes a supervisor-readable JSON snapshot before execution, refreshes it after each bounded tick with `updated_at` and `process_id` heartbeat metadata, and writes it again after successful bounded completion with the last tick/outcome and nullable last work-item id/status)
9. Run the bounded local end-to-end smoke matrix when you want one repo-local operator check that bootstraps auth and then exercises authenticated connections, search, ask, public review-webhook intake, and default one-tick worker completion together:
   ```bash
   bash scripts/check_end_to_end_smoke_matrix.sh /opt/data/projects/sourcebot-rewrite
   ```
10. That smoke command is intentionally local and stub-backed: it creates an isolated temp runtime, uses the real API and worker binaries, checks `/healthz` plus the file-backed `/readyz` readiness baseline, drives the current auth/search/ask/review-agent baseline, and verifies one queued review-agent run reaches `completed`. It is not a production certification matrix.
11. The worker still does **not** claim supervised workers, real fetch/import/reindex execution beyond the local-Git preflight plus persisted revision/content-count/current-branch and bounded tracked-content manifest/snapshot/search-index artifact baseline, or the Generic Git `git ls-remote --symref -- <base_url> HEAD` / `git ls-remote --heads -- <base_url>` metadata-only revision/current-branch probe with bounded stdout/stderr handling. Content-file counts, manifests, snapshots, and search-index artifacts remain explicitly local-only; Generic Git jobs do not clone/fetch/import/reindex or write artifacts, and malformed HEAD metadata (including unexpected non-`HEAD` records) fails closed instead of falling back to a different branch. The worker also does not claim broad durable worker metadata beyond the shared state file plus optional local status snapshot, broad automated retry/backoff, or production scheduling/supervision; the bounded multi-tick loop is only a local/operator-controlled repeat-tick baseline, the status snapshot is only a local per-tick process heartbeat/last-run artifact rather than a supervised daemon heartbeat or observability backend, stale-running lease recovery is only a bounded queue recovery path before the next claim, and automatic retry is limited to one queued retry for stale-lease failures after a one-hour backoff when no active or prior automatic replacement exists.
12. `/healthz`, `/readyz`, `/api/v1/config`, and authenticated `GET /api/v1/auth/worker-status` define the current operator-visible runtime baseline. `/readyz` reports file-backed metadata as ready when `DATABASE_URL` is unset, and when `DATABASE_URL` is configured it connects to PostgreSQL and fails closed with `503` unless the SQLx migration inventory is readable. `/api/v1/auth/worker-status` requires an authenticated local session and returns either `not_configured`, a bounded JSON snapshot from `SOURCEBOT_WORKER_STATUS_PATH`, or a structured snapshot-read/parse/size error. This is a bounded local readiness/status check; it does not yet claim full dependency health, supervised-worker readiness, production-grade observability, durable worker metadata, or upgrade automation.

## Local operator maintenance baseline
1. Capture a backup of the current file-backed runtime state before maintenance:
   ```bash
   make runtime-backup
   ```
2. Record the runtime backup directory emitted by the helper; it contains copies of the current file-backed runtime paths plus a manifest. When `DATABASE_URL` is configured, active durable local sessions live in PostgreSQL instead of `local-sessions.json`, so pair runtime backups with the metadata backup flow below.
3. Start or confirm the local metadata dependency before metadata backup or schema maintenance:
   ```bash
   make dev-up
   ```
4. Capture a backup of the current local metadata database before schema maintenance:
   ```bash
   make metadata-backup
   ```
5. Record the metadata backup directory emitted by the helper; it contains a SQL dump and manifest for the current local `DATABASE_URL` target without storing plaintext credentials.
6. Run the current local SQLx migration workflow:
   ```bash
   make sqlx-migrate
   ```
7. Treat upgrades as a repo update plus migration plus local process restart sequence:
   ```bash
   git pull --ff-only
   make sqlx-migrate
   make api
   make worker
   ```
8. If maintenance fails, restore the file-backed runtime baseline from the captured runtime backup directory. The restore helper validates the backup manifest against the currently resolved runtime paths before copying files so an operator does not accidentally replay a backup captured for a different `SOURCEBOT_DATA_DIR` or explicit state-file override set:
   ```bash
   BACKUP_DIR=/absolute/path/to/backups/runtime/20260422T000000Z
   make runtime-restore BACKUP_DIR="$BACKUP_DIR"
   ```
9. If maintenance fails after a metadata change, restore the local metadata database from the captured metadata backup directory:
   ```bash
   BACKUP_DIR=/absolute/path/to/backups/metadata/20260422T000000Z
   make metadata-restore BACKUP_DIR="$BACKUP_DIR"
   ```
10. The metadata backup/restore helpers intentionally stay local-only for this baseline: they require `DATABASE_URL` to target `127.0.0.1` or `localhost`, validate a matching redacted manifest on restore, and rely on `pg_dump`/`psql` from the local operator environment.
11. This maintenance baseline now covers the current file-backed runtime state plus the local Postgres metadata dump/restore workflow; notably, bootstrap-admin, local sessions, local accounts, memberships, invite acceptance, API keys, OAuth clients, review-agent-run lifecycle rows, and review-webhook delivery-attempt rows are durable in PostgreSQL when `DATABASE_URL` is configured, but broader organization aggregates, catalog state beyond bounded reads, and the remaining runtime parity work still remain follow-up slices.

## License
Current default: MIT.
If you prefer a stronger explicit patent grant, we can switch to Apache-2.0 before first public release.
