#!/bin/sh
set -eu

repo_root=${1:-.}
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

require_line "$makefile" '^\.PHONY:.*\bsqlx-test\b' 'Makefile sqlx-test phony target'
require_line "$makefile" 'make sqlx-test[[:space:]]+- reset the deterministic test metadata database and run focused metadata storage tests' 'Makefile help entry for sqlx-test'
require_line "$makefile" '^sqlx-test:$' 'Makefile sqlx-test target'
require_line "$makefile" '^[[:space:]]+\$\(MAKE\) sqlx-test-reset$' 'sqlx-test reset wrapper command'
require_line "$makefile" 'DATABASE_URL="\$\$TEST_DATABASE_URL" \$\(CARGO\) test -p sourcebot-api --bin sourcebot-api storage::tests -- --nocapture' 'sqlx-test focused cargo test command'

require_line "$readme" '^5\. Run the focused metadata-schema test wrapper:$' 'README sqlx-test step'
require_line "$readme" '^   make sqlx-test$' 'README make sqlx-test command'
require_line "$readme" '^9\. `make sqlx-test` wraps the deterministic reset plus the focused `sourcebot-api` metadata storage test suite so local migration workflow verification uses one reproducible command\.$' 'README sqlx-test workflow note'
require_line "$readme" '^10\. `make sqlx-test` runs the current storage migration-inventory and catalog fallback tests, not full Postgres-backed runtime parity; durable-store execution remains a later roadmap slice\.$' 'README sqlx-test truthful scope note'

printf 'sqlx-test workflow contract OK\n'
