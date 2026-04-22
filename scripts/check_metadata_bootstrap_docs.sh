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
require_line '^5\. Run the focused metadata-schema test wrapper:$' 'sqlx test wrapper step'
require_line '^   make sqlx-test$' 'make sqlx-test command'
require_line '^6\. Or run the single local metadata bootstrap wrapper:$' 'metadata-dev-bootstrap step'
require_line '^   make metadata-dev-bootstrap$' 'make metadata-dev-bootstrap command'
require_line '^7\. `make` auto-loads `.env`, so `.env.example` stays the runnable local metadata DB contract for both the local-only `sourcebot` bootstrap database and the dedicated `sourcebot_test` test database\.$' 'runnable .env contract note'
require_line '^8\. `make sqlx-test-reset` uses `TEST_DATABASE_URL` plus the repo-local `\.sqlx-cli` install root to drop, recreate, and re-migrate the deterministic local test database\.$' 'truthful test reset note'
require_line '^9\. `make sqlx-test` wraps the deterministic reset plus the focused `sourcebot-api` metadata storage, PostgreSQL-backed bootstrap-admin, and durable local-session test suite so local migration workflow verification uses one reproducible command\.$' 'truthful sqlx-test wrapper note'
require_line '^10\. `make sqlx-test` now covers the storage migration-inventory/catalog fallback checks, the PostgreSQL-backed bootstrap store and `/api/v1/auth/bootstrap` \+ login regressions, plus the PostgreSQL-backed local-session store and login -> `/api/v1/auth/me` regression slice; bootstrap admin metadata now persists in PostgreSQL when DATABASE_URL is configured, while broader durable catalog/auth/org runtime parity remains a later roadmap slice\.$' 'truthful sqlx-test scope note'
require_line '^11\. `make sqlx-test-reset` refuses non-local or non-`_test` databases so the destructive reset flow stays scoped to the dedicated local metadata test database\.$' 'truthful destructive safety note'
require_line '^12\. `make metadata-dev-bootstrap` is a local-only operator bootstrap helper that waits for local Postgres, ensures the dedicated test database exists, runs `make sqlx-migrate`, and then runs the focused `make sqlx-test` compatibility check\.$' 'truthful metadata-dev-bootstrap note'
require_line '^13\. `make metadata-dev-bootstrap` does not mean the API already uses durable metadata by default; the current API still routes `DATABASE_URL` through an unimplemented lazy `PgCatalogStore` path, so this helper is only a local bootstrap-and-compatibility workflow today\.$' 'truthful metadata-dev-bootstrap scope note'

printf 'metadata bootstrap docs contract OK\n'
