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
require_line "$readme" '^9\. `make sqlx-test` wraps the deterministic reset plus the focused `sourcebot-api` metadata storage, PostgreSQL-backed bootstrap-admin, and durable local-session test suite so local migration workflow verification uses one reproducible command\.$' 'README sqlx-test wrapper note'
require_line "$readme" '^10\. `make sqlx-test` now covers the storage migration-inventory/catalog fallback checks, the PostgreSQL-backed bootstrap store and `/api/v1/auth/bootstrap` \+ login regressions, plus the PostgreSQL-backed local-session store and login -> `/api/v1/auth/me` regression slice; bootstrap admin metadata now persists in PostgreSQL when DATABASE_URL is configured, while broader durable catalog/auth/org runtime parity remains a later roadmap slice\.$' 'README sqlx-test truthful scope note'
require_line "$readme" '^11\. `make sqlx-test-reset` refuses non-local or non-`_test` databases so the destructive reset flow stays scoped to the dedicated local metadata test database\.$' 'README destructive safety note'
require_line "$readme" '^12\. `make metadata-dev-bootstrap` is a local-only operator bootstrap helper that waits for local Postgres, ensures the dedicated test database exists, runs `make sqlx-migrate`, and then runs the focused `make sqlx-test` compatibility check\.$' 'README metadata-dev-bootstrap note'
require_line "$readme" '^13\. `make metadata-dev-bootstrap` does not mean the API already uses durable metadata by default; the current API still routes `DATABASE_URL` through an unimplemented lazy `PgCatalogStore` path, so this helper is only a local bootstrap-and-compatibility workflow today\.$' 'README metadata-dev-bootstrap truthful scope note'

printf 'local metadata env contract OK\n'
