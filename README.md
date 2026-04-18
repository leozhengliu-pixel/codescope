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
4. `make` auto-loads `.env`, so `.env.example` stays the runnable local metadata DB contract for the local-only `sourcebot` / `sourcebot` bootstrap defaults.
5. The current API still falls back to the seeded in-memory catalog store even when `DATABASE_URL` is set; this workflow only bootstraps the metadata schema for upcoming durable-store slices.
6. Deterministic dev/test database setup remains deferred to a later roadmap slice.

## License
Current default: MIT.
If you prefer a stronger explicit patent grant, we can switch to Apache-2.0 before first public release.
