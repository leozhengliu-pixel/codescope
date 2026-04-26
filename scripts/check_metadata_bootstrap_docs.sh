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
require_line '^9\. `make sqlx-test` wraps the deterministic reset plus the focused `sourcebot-api` metadata storage, PostgreSQL-backed catalog list/detail, PostgreSQL-backed bootstrap-admin, durable local-session, PostgreSQL-backed local-account/membership/invite auth, PostgreSQL-backed repository-permission filtering for authenticated sync-job history, PostgreSQL-backed repository-sync-job lifecycle storage, PostgreSQL-backed review-agent-run and delivery-attempt lifecycle storage, PostgreSQL-backed ask-thread/message storage, and durable API-key/OAuth-client metadata regressions so local migration workflow verification uses one reproducible command\.$' 'truthful sqlx-test wrapper note'
require_line '^10\. `make sqlx-test` now covers the storage migration-inventory/catalog fallback checks, PostgreSQL-backed catalog list/detail queries for `/api/v1/repos` and repository detail reads, the PostgreSQL-backed bootstrap store and `/api/v1/auth/bootstrap` \+ login regressions, the PostgreSQL-backed local-session store and login -> `/api/v1/auth/me` regression slice, the durable PostgreSQL local-account lookup/linked-account membership/member-roster/invite-redeem regressions, authenticated `/api/v1/auth/repository-sync-jobs` filtering against PostgreSQL-backed repository permissions, PostgreSQL-backed repository-sync-job upsert/claim/complete lifecycle regressions, PostgreSQL-backed review-agent-run store/merge/claim/complete/fail lifecycle regressions, PostgreSQL-backed review-webhook delivery-attempt store/merge regressions, PostgreSQL-backed ask-thread create/append/list/detail owner-scoped regressions, plus PostgreSQL-backed API-key inventory/create/revoke/bearer-auth and OAuth-client inventory/create regressions; catalog read metadata, bootstrap-admin, invited-account login, auth-me identity restoration, member rosters, linked-account memberships, invite acceptance, sync-job permission filtering and lifecycle rows, review-agent-run lifecycle rows, review-webhook delivery-attempt rows, ask-thread messages, API-key metadata, and OAuth-client metadata now all stay durable across API restarts when `DATABASE_URL` is configured\.$' 'truthful sqlx-test scope note'
require_line '^11\. `make sqlx-test-reset` refuses non-local or non-`_test` databases so the destructive reset flow stays scoped to the dedicated local metadata test database\.$' 'truthful destructive safety note'
require_line '^12\. `make metadata-dev-bootstrap` is a local-only operator bootstrap helper that waits for local Postgres, ensures the dedicated test database exists, runs `make sqlx-migrate`, and then runs the focused `make sqlx-test` compatibility check\.$' 'truthful metadata-dev-bootstrap note'
require_line '^13\. `make metadata-dev-bootstrap` now exercises a bounded PostgreSQL catalog read path plus the durable auth metadata slices; catalog list/detail reads use PostgreSQL when `DATABASE_URL` is configured, while local repository import, connection management, analytics/audit aggregates, repo-permission sync, and broader organization aggregates still remain follow-up work\.$' 'truthful metadata-dev-bootstrap scope note'

printf 'metadata bootstrap docs contract OK\n'
