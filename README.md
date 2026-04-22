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
- Search/indexing: Rust service with Tantivy + regex-automata + tree-sitter
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
6. `make` auto-loads `.env`, so `.env.example` stays the runnable local metadata DB contract for both the local-only `sourcebot` bootstrap database and the dedicated `sourcebot_test` test database.
7. `make sqlx-test-reset` uses `TEST_DATABASE_URL` plus the repo-local `.sqlx-cli` install root to drop, recreate, and re-migrate the deterministic local test database.
8. `make sqlx-test` wraps the deterministic reset plus the focused `sourcebot-api` metadata storage test suite so local migration workflow verification uses one reproducible command.
9. `make sqlx-test` runs the current storage migration-inventory and catalog fallback tests, not full Postgres-backed runtime parity; durable-store execution remains a later roadmap slice.
10. `make sqlx-test-reset` refuses non-local or non-`_test` databases so the destructive reset flow stays scoped to the dedicated local metadata test database.
11. The current API still falls back to the seeded in-memory catalog store even when `DATABASE_URL` is set; these workflows only bootstrap the metadata schema for upcoming durable-store slices.

## Local operator runtime baseline
1. Copy the example env file so `make` can auto-load the repo-local runtime contract:
   ```bash
   cp .env.example .env
   ```
2. Set `SOURCEBOT_DATA_DIR` in `.env` to the directory that should hold the current local file-backed runtime state. When only that shared base is set, the API and worker derive:
   - `bootstrap-state.json`
   - `local-sessions.json`
   - `organizations.json`
3. Optional explicit overrides still win for individual files if you set `SOURCEBOT_BOOTSTRAP_STATE_PATH`, `SOURCEBOT_LOCAL_SESSION_STATE_PATH`, or `SOURCEBOT_ORGANIZATION_STATE_PATH`.
4. Start the API with the repo-local `.env` contract:
   ```bash
   make api
   ```
5. In a second shell, run the current worker baseline with that same `.env` contract:
   ```bash
   make worker
   ```
6. `make worker` is intentionally a one-shot local bring-up path: it runs one worker tick, logs a startup runtime baseline that includes the resolved organization-state path plus the selected review-agent and repository-sync stub outcomes, and then exits.
7. Optional worker-only stub controls for the current baseline are:
   - `SOURCEBOT_STUB_REVIEW_AGENT_RUN_EXECUTION_OUTCOME=completed|failed`
   - `SOURCEBOT_STUB_REPOSITORY_SYNC_JOB_EXECUTION_OUTCOME=succeeded|failed`
8. The worker still does **not** claim supervised workers, real execution, durable worker metadata, retries, scheduling, or continuous background orchestration.
9. `/healthz` and `/api/v1/config` define the current operator-visible runtime baseline. They do not yet claim dependency readiness, migration readiness, or production-grade observability.

## Local operator maintenance baseline
1. Capture a backup of the current file-backed runtime state before maintenance:
   ```bash
   make runtime-backup
   ```
2. Record the backup directory emitted by the helper; it contains copies of `bootstrap-state.json`, `local-sessions.json`, `organizations.json`, and a manifest for the resolved runtime paths.
3. Start or confirm the local metadata dependency before schema maintenance:
   ```bash
   make dev-up
   ```
4. Run the current local SQLx migration workflow:
   ```bash
   make sqlx-migrate
   ```
5. Treat upgrades as a repo update plus migration plus local process restart sequence:
   ```bash
   git pull --ff-only
   make sqlx-migrate
   make api
   make worker
   ```
6. If maintenance fails, restore the file-backed runtime baseline from the captured backup directory:
   ```bash
   BACKUP_DIR=/absolute/path/to/backups/runtime/20260422T000000Z
   make runtime-restore BACKUP_DIR="$BACKUP_DIR"
   ```
7. This maintenance baseline is intentionally narrow: it covers only the current file-backed runtime state plus the local SQLx migration workflow, and it does not yet claim durable metadata backup/restore parity, readiness checks, or production-grade deployment automation.

## License
Current default: MIT.
If you prefer a stronger explicit patent grant, we can switch to Apache-2.0 before first public release.
