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
5. `make` auto-loads `.env`, so `.env.example` stays the runnable local metadata DB contract for both the local-only `sourcebot` bootstrap database and the dedicated `sourcebot_test` test database.
6. `make sqlx-test-reset` uses `TEST_DATABASE_URL` plus the repo-local `.sqlx-cli` install root to drop, recreate, and re-migrate the deterministic local test database.
7. `make sqlx-test-reset` refuses non-local or non-`_test` databases so the destructive reset flow stays scoped to the dedicated local metadata test database.
8. The current API still falls back to the seeded in-memory catalog store even when `DATABASE_URL` is set; these workflows only bootstrap the metadata schema for upcoming durable-store slices.

## License
Current default: MIT.
If you prefer a stronger explicit patent grant, we can switch to Apache-2.0 before first public release.
