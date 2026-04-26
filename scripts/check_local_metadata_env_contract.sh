#!/bin/sh
set -eu

repo_root=${1:-.}
env_file="$repo_root/.env.example"
compose_file="$repo_root/docker-compose.yml"
makefile="$repo_root/Makefile"
readme="$repo_root/README.md"

require_line() {
  file=$1
  pattern=$2
  description=$3

  if ! grep -Eq "$pattern" "$file"; then
    printf 'missing %s in %s\n' "$description" "$file" >&2
    exit 1
  fi
}

env_value() {
  key=$1
  value=$(grep -E "^${key}=" "$env_file" | head -n 1 | cut -d= -f2- || true)
  if [ -z "$value" ]; then
    printf 'missing %s in %s\n' "$key" "$env_file" >&2
    exit 1
  fi
  printf '%s' "$value"
}

postgres_db=$(env_value POSTGRES_DB)
postgres_user=$(env_value POSTGRES_USER)
postgres_password=$(env_value POSTGRES_PASSWORD)
database_url=$(env_value DATABASE_URL)
test_database_url=$(env_value TEST_DATABASE_URL)
expected_url="postgres://${postgres_user}:${postgres_password}@127.0.0.1:5432/${postgres_db}"
expected_test_url="postgres://${postgres_user}:${postgres_password}@127.0.0.1:5432/${postgres_db}_test"

if [ "$database_url" != "$expected_url" ]; then
  printf 'DATABASE_URL mismatch: expected %s but found %s\n' "$expected_url" "$database_url" >&2
  exit 1
fi

if [ "$test_database_url" != "$expected_test_url" ]; then
  printf 'TEST_DATABASE_URL mismatch: expected %s but found %s\n' "$expected_test_url" "$test_database_url" >&2
  exit 1
fi

require_line "$compose_file" '^      POSTGRES_DB: \$\{POSTGRES_DB:-sourcebot\}$' 'docker compose POSTGRES_DB default'
require_line "$compose_file" '^      POSTGRES_USER: \$\{POSTGRES_USER:-sourcebot\}$' 'docker compose POSTGRES_USER default'
require_line "$compose_file" '^      POSTGRES_PASSWORD: \$\{POSTGRES_PASSWORD:-sourcebot\}$' 'docker compose POSTGRES_PASSWORD default'

require_line "$makefile" '^ifneq \(,\$\(wildcard \.env\)\)$' 'Makefile .env presence guard'
require_line "$makefile" '^include \.env$' 'Makefile .env include'
require_line "$makefile" '^export$' 'Makefile env export'
require_line "$makefile" '^dev-up:$' 'Makefile dev-up target'
require_line "$makefile" '^[[:space:]]+docker compose up -d postgres$' 'Makefile docker compose bootstrap command'
require_line "$makefile" '^sqlx-migrate:$' 'Makefile sqlx-migrate target'
require_line "$makefile" 'DATABASE_URL:\?DATABASE_URL must be set' 'Makefile DATABASE_URL guard'
require_line "$makefile" '^sqlx-test-reset:$' 'Makefile sqlx-test-reset target'
require_line "$makefile" 'TEST_DATABASE_URL:\?TEST_DATABASE_URL must be set' 'Makefile TEST_DATABASE_URL guard'
require_line "$makefile" 'TEST_DATABASE_URL must target the dedicated local sourcebot_test database on 127\.0\.0\.1 or localhost' 'Makefile destructive test DB safety guard'
require_line "$makefile" 'DATABASE_URL="\$\$TEST_DATABASE_URL" \$\(SQLX_CLI_ROOT\)/bin/sqlx database reset --source crates/api/migrations -y' 'Makefile deterministic test database reset command'
require_line "$makefile" '^sqlx-test:$' 'Makefile sqlx-test target'
require_line "$makefile" '^[[:space:]]+\$\(MAKE\) sqlx-test-reset$' 'Makefile sqlx-test reset wrapper command'
require_line "$makefile" 'DATABASE_URL="\$\$TEST_DATABASE_URL" \$\(CARGO\) test -p sourcebot-api --bin sourcebot-api storage::tests -- --nocapture' 'Makefile focused metadata storage test command'
require_line "$makefile" 'TEST_DATABASE_URL="\$\$TEST_DATABASE_URL" \$\(CARGO\) test -p sourcebot-api --bin sourcebot-api pg_org_auth_metadata_ -- --nocapture --test-threads=1' 'Makefile pg org-auth metadata test command'
require_line "$makefile" 'TEST_DATABASE_URL="\$\$TEST_DATABASE_URL" \$\(CARGO\) test -p sourcebot-api --bin sourcebot-api postgres_backed_local_account_ -- --nocapture --test-threads=1' 'Makefile postgres local-account regression command'
require_line "$makefile" 'TEST_DATABASE_URL="\$\$TEST_DATABASE_URL" \$\(CARGO\) test -p sourcebot-api --bin sourcebot-api auth_members_ -- --nocapture --test-threads=1' 'Makefile auth members regression command'
require_line "$makefile" 'TEST_DATABASE_URL="\$\$TEST_DATABASE_URL" \$\(CARGO\) test -p sourcebot-api --bin sourcebot-api auth_linked_accounts_ -- --nocapture --test-threads=1' 'Makefile linked accounts regression command'
require_line "$makefile" 'TEST_DATABASE_URL="\$\$TEST_DATABASE_URL" \$\(CARGO\) test -p sourcebot-api --bin sourcebot-api invite_redeem_ -- --nocapture --test-threads=1' 'Makefile invite redeem regression command'
require_line "$makefile" '^metadata-dev-bootstrap:$' 'Makefile metadata-dev-bootstrap target'
require_line "$makefile" '^[[:space:]]+bash scripts/bootstrap_local_metadata_dev\.sh$' 'Makefile metadata-dev-bootstrap helper command'

require_line "$readme" '^## Local metadata DB bootstrap$' 'README local metadata bootstrap heading'
require_line "$readme" '^1\. Copy the example env file for the deterministic local Postgres defaults:$' 'README .env bootstrap step'
require_line "$readme" '^   cp \.env\.example \.env$' 'README cp .env.example .env command'
require_line "$readme" '^2\. Start the local Postgres service:$' 'README dev-up step'
require_line "$readme" '^   make dev-up$' 'README make dev-up command'
require_line "$readme" '^3\. Run the SQLx metadata-schema migrations:$' 'README sqlx-migrate step'
require_line "$readme" '^   make sqlx-migrate$' 'README make sqlx-migrate command'
require_line "$readme" '^4\. Reset the dedicated deterministic local test database when a local test run needs a clean metadata schema:$' 'README sqlx-test-reset step'
require_line "$readme" '^   make sqlx-test-reset$' 'README make sqlx-test-reset command'
require_line "$readme" '^5\. Run the focused metadata-schema test wrapper:$' 'README sqlx-test step'
require_line "$readme" '^   make sqlx-test$' 'README make sqlx-test command'
require_line "$readme" '^6\. Or run the single local metadata bootstrap wrapper:$' 'README metadata-dev-bootstrap step'
require_line "$readme" '^   make metadata-dev-bootstrap$' 'README make metadata-dev-bootstrap command'
require_line "$readme" '^7\. `make` auto-loads `.env`, so `.env.example` stays the runnable local metadata DB contract for both the local-only `sourcebot` bootstrap database and the dedicated `sourcebot_test` test database\.$' 'README .env auto-load note'
require_line "$readme" '^8\. `make sqlx-test-reset` uses `TEST_DATABASE_URL` plus the repo-local `\.sqlx-cli` install root to drop, recreate, and re-migrate the deterministic local test database\.$' 'README test reset note'
require_line "$readme" '^9\. `make sqlx-test` wraps the deterministic reset plus the focused `sourcebot-api` metadata storage, PostgreSQL-backed catalog list/detail, PostgreSQL-backed bootstrap-admin, durable local-session, PostgreSQL-backed local-account/membership/invite auth, PostgreSQL-backed repository-permission filtering for authenticated sync-job history, PostgreSQL-backed repository-sync-job lifecycle storage, and durable API-key/OAuth-client metadata regressions so local migration workflow verification uses one reproducible command\.$' 'README sqlx-test wrapper note'
require_line "$readme" '^10\. `make sqlx-test` now covers the storage migration-inventory/catalog fallback checks, PostgreSQL-backed catalog list/detail queries for `/api/v1/repos` and repository detail reads, the PostgreSQL-backed bootstrap store and `/api/v1/auth/bootstrap` \+ login regressions, the PostgreSQL-backed local-session store and login -> `/api/v1/auth/me` regression slice, the durable PostgreSQL local-account lookup/linked-account membership/member-roster/invite-redeem regressions, authenticated `/api/v1/auth/repository-sync-jobs` filtering against PostgreSQL-backed repository permissions, PostgreSQL-backed repository-sync-job upsert/claim/complete lifecycle regressions, plus PostgreSQL-backed API-key inventory/create/revoke/bearer-auth and OAuth-client inventory/create regressions; catalog read metadata, bootstrap-admin, invited-account login, auth-me identity restoration, member rosters, linked-account memberships, invite acceptance, sync-job permission filtering and lifecycle rows, API-key metadata, and OAuth-client metadata now all stay durable across API restarts when `DATABASE_URL` is configured\.$' 'README sqlx-test scope note'
require_line "$readme" '^11\. `make sqlx-test-reset` refuses non-local or non-`_test` databases so the destructive reset flow stays scoped to the dedicated local metadata test database\.$' 'README destructive safety note'
require_line "$readme" '^12\. `make metadata-dev-bootstrap` is a local-only operator bootstrap helper that waits for local Postgres, ensures the dedicated test database exists, runs `make sqlx-migrate`, and then runs the focused `make sqlx-test` compatibility check\.$' 'README metadata-dev-bootstrap note'
require_line "$readme" '^13\. `make metadata-dev-bootstrap` now exercises a bounded PostgreSQL catalog read path plus the durable auth metadata slices; catalog list/detail reads use PostgreSQL when `DATABASE_URL` is configured, while local repository import, connection management, analytics/audit aggregates, repo-permission sync, and broader organization aggregates still remain follow-up work\.$' 'README metadata-dev-bootstrap truthful scope note'

printf 'local metadata env contract OK\n'
