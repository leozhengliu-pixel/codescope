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
require_line '^4\. Reset the dedicated deterministic local test database when a local test run needs a clean metadata schema:$' 'sqlx test reset step'
require_line '^   make sqlx-test-reset$' 'make sqlx-test-reset command'
require_line '^5\. `make` auto-loads `.env`, so `.env.example` stays the runnable local metadata DB contract for both the local-only `sourcebot` bootstrap database and the dedicated `sourcebot_test` test database\.$' 'runnable .env contract note'
require_line '^6\. `make sqlx-test-reset` uses `TEST_DATABASE_URL` plus the repo-local `\.sqlx-cli` install root to drop, recreate, and re-migrate the deterministic local test database\.$' 'truthful test reset note'
require_line '^7\. `make sqlx-test-reset` refuses non-local or non-`_test` databases so the destructive reset flow stays scoped to the dedicated local metadata test database\.$' 'truthful destructive safety note'
require_line '^8\. The current API still falls back to the seeded in-memory catalog store even when `DATABASE_URL` is set; these workflows only bootstrap the metadata schema for upcoming durable-store slices\.$' 'truthful fallback note'

printf 'metadata bootstrap docs contract OK\n'
