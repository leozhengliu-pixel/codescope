#!/bin/sh
set -eu

repo_root=${1:-.}
makefile="$repo_root/Makefile"
readme="$repo_root/README.md"

require_file() {
  file=$1
  description=$2

  if [ ! -f "$file" ]; then
    printf 'missing %s at %s\n' "$description" "$file" >&2
    exit 1
  fi
}

require_line() {
  file=$1
  pattern=$2
  description=$3

  if ! grep -Eq "$pattern" "$file"; then
    printf 'missing %s in %s\n' "$description" "$file" >&2
    exit 1
  fi
}

require_file "$makefile" 'Makefile'
require_file "$readme" 'README'

require_line "$makefile" '^\.PHONY:.*\bmetadata-dev-bootstrap\b' 'Makefile metadata-dev-bootstrap phony target'
require_line "$makefile" 'make metadata-dev-bootstrap[[:space:]]+- wait for local Postgres, ensure the dedicated test metadata database exists, run migrations, and run focused metadata compatibility tests' 'Makefile help entry for metadata-dev-bootstrap'
require_line "$makefile" '^metadata-dev-bootstrap:$' 'Makefile metadata-dev-bootstrap target'
require_line "$makefile" '^[[:space:]]+bash scripts/bootstrap_local_metadata_dev\.sh$' 'Makefile bootstrap helper invocation'

require_line "$readme" '^6\. Or run the single local metadata bootstrap wrapper:$' 'README metadata-dev-bootstrap step'
require_line "$readme" '^   make metadata-dev-bootstrap$' 'README make metadata-dev-bootstrap command'
require_line "$readme" '^12\. `make metadata-dev-bootstrap` is a local-only operator bootstrap helper that waits for local Postgres, ensures the dedicated test database exists, runs `make sqlx-migrate`, and then runs the focused `make sqlx-test` compatibility check\.$' 'README metadata-dev-bootstrap workflow note'
require_line "$readme" '^13\. `make metadata-dev-bootstrap` now exercises a bounded PostgreSQL catalog read path plus the durable auth metadata slices; catalog list/detail reads and one explicitly requested local Git repository import handoff use PostgreSQL when `DATABASE_URL` is configured; the authenticated local import route now also adds the imported repository to the admin organization visibility set and queues one repository-sync job for the existing one-shot worker baseline, while broader connection management durability, analytics/audit aggregates, recursive/provider import, reindex execution, and broader organization aggregates still remain follow-up work\.$' 'README metadata-dev-bootstrap truthful scope note'

printf 'local metadata dev bootstrap contract OK\n'
