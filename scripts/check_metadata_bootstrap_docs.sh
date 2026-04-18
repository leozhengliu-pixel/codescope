#!/bin/sh
set -eu

readme=${1:-README.md}

require_line() {
  pattern=$1
  description=$2

  if ! grep -Eq "$pattern" "$readme"; then
    printf 'missing %s\n' "$description" >&2
    exit 1
  fi
}

require_line '^## Local metadata DB bootstrap$' 'Local metadata DB bootstrap section heading'
require_line '^1\. Copy the example env file for the deterministic local Postgres defaults:$' 'step to copy .env.example into .env'
require_line '^   ```bash$' 'bash code fence'
require_line '^   cp \.env\.example \.env$' 'copy .env.example command'
require_line '^2\. Start the local Postgres service:$' 'step to start Postgres'
require_line '^   make dev-up$' 'make dev-up command'
require_line '^3\. Run the SQLx metadata-schema migrations:$' 'sqlx migration step'
require_line '^   make sqlx-migrate$' 'make sqlx-migrate command'
require_line '^4\. `make` auto-loads `.env`, so `.env.example` stays the runnable local metadata DB contract for the local-only `sourcebot` / `sourcebot` bootstrap defaults\.$' 'runnable .env contract note'
require_line '^5\. The current API still falls back to the seeded in-memory catalog store even when `DATABASE_URL` is set; this workflow only bootstraps the metadata schema for upcoming durable-store slices\.$' 'truthful fallback note'
require_line '^6\. Deterministic dev/test database setup remains deferred to a later roadmap slice\.$' 'deferred deterministic setup note'

printf 'metadata bootstrap docs contract OK\n'
