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
require_line "$makefile" 'make sqlx-test[[:space:]]+- reset the deterministic test metadata database and run focused metadata plus durable bootstrap/local-account/org-auth tests' 'Makefile help entry for sqlx-test'
require_line "$makefile" '^sqlx-test:$' 'Makefile sqlx-test target'
require_line "$makefile" '^[[:space:]]+\$\(MAKE\) sqlx-test-reset$' 'sqlx-test reset wrapper command'
require_line "$makefile" 'DATABASE_URL="\$\$TEST_DATABASE_URL" \$\(CARGO\) test -p sourcebot-api --bin sourcebot-api storage::tests -- --nocapture' 'sqlx-test focused cargo test command'
require_line "$makefile" 'TEST_DATABASE_URL="\$\$TEST_DATABASE_URL" \$\(CARGO\) test -p sourcebot-api --bin sourcebot-api pg_org_auth_metadata_ -- --nocapture --test-threads=1' 'sqlx-test org-auth metadata command'
require_line "$makefile" 'TEST_DATABASE_URL="\$\$TEST_DATABASE_URL" \$\(CARGO\) test -p sourcebot-api --bin sourcebot-api postgres_backed_local_account_ -- --nocapture --test-threads=1' 'sqlx-test postgres local-account command'
require_line "$makefile" 'TEST_DATABASE_URL="\$\$TEST_DATABASE_URL" \$\(CARGO\) test -p sourcebot-api --bin sourcebot-api auth_members_ -- --nocapture --test-threads=1' 'sqlx-test auth members command'
require_line "$makefile" 'TEST_DATABASE_URL="\$\$TEST_DATABASE_URL" \$\(CARGO\) test -p sourcebot-api --bin sourcebot-api auth_linked_accounts_ -- --nocapture --test-threads=1' 'sqlx-test auth linked-accounts command'
require_line "$makefile" 'TEST_DATABASE_URL="\$\$TEST_DATABASE_URL" \$\(CARGO\) test -p sourcebot-api --bin sourcebot-api invite_redeem_ -- --nocapture --test-threads=1' 'sqlx-test invite redeem command'

require_line "$readme" '^5\. Run the focused metadata-schema test wrapper:$' 'README sqlx-test step'
require_line "$readme" '^   make sqlx-test$' 'README make sqlx-test command'
require_line "$readme" '^9\. `make sqlx-test` wraps the deterministic reset plus the focused `sourcebot-api` metadata storage, PostgreSQL-backed bootstrap-admin, durable local-session, and PostgreSQL-backed local-account/membership/invite auth regressions so local migration workflow verification uses one reproducible command\.$' 'README sqlx-test workflow note'
require_line "$readme" '^10\. `make sqlx-test` now covers the storage migration-inventory/catalog fallback checks, the PostgreSQL-backed bootstrap store and `/api/v1/auth/bootstrap` \+ login regressions, the PostgreSQL-backed local-session store and login -> `/api/v1/auth/me` regression slice, plus the durable PostgreSQL local-account lookup, linked-account membership, member-roster, and invite-redeem regressions; bootstrap-admin, invited-account login, auth-me identity restoration, member rosters, linked-account memberships, and invite acceptance now persist in PostgreSQL when `DATABASE_URL` is configured, while API keys, OAuth clients, connections, analytics, audit events, and the remaining whole-aggregate organization state remain later roadmap work\.$' 'README sqlx-test truthful scope note'

printf 'sqlx-test workflow contract OK\n'
