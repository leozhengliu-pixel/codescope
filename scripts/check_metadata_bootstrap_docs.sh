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
require_line '^1\. Start the local Postgres service:$' 'step to start Postgres'
require_line '^   ```bash$' 'bash code fence'
require_line '^   make dev-up$' 'make dev-up command'
require_line '^2\. Set `DATABASE_URL` to the local metadata database:$' 'DATABASE_URL setup step'
require_line '^   export DATABASE_URL=postgres://sourcebot:sourcebot@127\.0\.0\.1:5432/sourcebot$' 'export DATABASE_URL example'
require_line '^3\. Run the SQLx metadata-schema migrations:$' 'sqlx migration step'
require_line '^   make sqlx-migrate$' 'make sqlx-migrate command'
require_line '^4\. The current API still falls back to the seeded in-memory catalog store even when `DATABASE_URL` is set; this workflow only bootstraps the metadata schema for upcoming durable-store slices\.$' 'truthful fallback note'
require_line '^5\. Deterministic dev/test database setup remains deferred to a later roadmap slice\.$' 'deferred deterministic setup note'

printf 'metadata bootstrap docs contract OK\n'
