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
expected_url="postgres://${postgres_user}:${postgres_password}@127.0.0.1:5432/${postgres_db}"

if [ "$database_url" != "$expected_url" ]; then
  printf 'DATABASE_URL mismatch: expected %s but found %s\n' "$expected_url" "$database_url" >&2
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

require_line "$readme" '^## Local metadata DB bootstrap$' 'README local metadata bootstrap heading'
require_line "$readme" '^1\. Copy the example env file for the deterministic local Postgres defaults:$' 'README .env bootstrap step'
require_line "$readme" '^   cp \.env\.example \.env$' 'README cp .env.example .env command'
require_line "$readme" '^2\. Start the local Postgres service:$' 'README dev-up step'
require_line "$readme" '^   make dev-up$' 'README make dev-up command'
require_line "$readme" '^3\. Run the SQLx metadata-schema migrations:$' 'README sqlx-migrate step'
require_line "$readme" '^   make sqlx-migrate$' 'README make sqlx-migrate command'
require_line "$readme" '^4\. `make` auto-loads `.env`, so `.env.example` stays the runnable local metadata DB contract for the local-only `sourcebot` / `sourcebot` bootstrap defaults\.$' 'README .env auto-load note'
require_line "$readme" '^5\. The current API still falls back to the seeded in-memory catalog store even when `DATABASE_URL` is set; this workflow only bootstraps the metadata schema for upcoming durable-store slices\.$' 'README fallback note'
require_line "$readme" '^6\. Deterministic dev/test database setup remains deferred to a later roadmap slice\.$' 'README deferred setup note'

printf 'local metadata env contract OK\n'
