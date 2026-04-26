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
require_line "$makefile" 'make sqlx-test[[:space:]]+- reset the deterministic test metadata database and run focused metadata plus durable catalog/bootstrap/local-account/repo-permission/repository-sync-job/review-agent-run/ask-thread/api-key/oauth-client auth tests' 'Makefile help entry for sqlx-test'
require_line "$makefile" '^sqlx-test:$' 'Makefile sqlx-test target'
require_line "$makefile" '^[[:space:]]+\$\(MAKE\) sqlx-test-reset$' 'sqlx-test reset wrapper command'
require_line "$makefile" 'DATABASE_URL="\$\$TEST_DATABASE_URL" TEST_DATABASE_URL="\$\$TEST_DATABASE_URL" \$\(CARGO\) test -p sourcebot-api --bin sourcebot-api storage::tests -- --nocapture' 'sqlx-test focused cargo test command'
require_line "$makefile" 'TEST_DATABASE_URL="\$\$TEST_DATABASE_URL" \$\(CARGO\) test -p sourcebot-api --bin sourcebot-api pg_catalog_store_ -- --nocapture --test-threads=1' 'sqlx-test postgres catalog command'
require_line "$makefile" 'TEST_DATABASE_URL="\$\$TEST_DATABASE_URL" \$\(CARGO\) test -p sourcebot-api --bin sourcebot-api pg_org_auth_metadata_ -- --nocapture --test-threads=1' 'sqlx-test org-auth metadata command'
require_line "$makefile" 'TEST_DATABASE_URL="\$\$TEST_DATABASE_URL" \$\(CARGO\) test -p sourcebot-api --bin sourcebot-api postgres_backed_local_account_ -- --nocapture --test-threads=1' 'sqlx-test postgres local-account command'
require_line "$makefile" 'TEST_DATABASE_URL="\$\$TEST_DATABASE_URL" \$\(CARGO\) test -p sourcebot-api --bin sourcebot-api auth_members_ -- --nocapture --test-threads=1' 'sqlx-test auth members command'
require_line "$makefile" 'TEST_DATABASE_URL="\$\$TEST_DATABASE_URL" \$\(CARGO\) test -p sourcebot-api --bin sourcebot-api auth_linked_accounts_ -- --nocapture --test-threads=1' 'sqlx-test auth linked-accounts command'
require_line "$makefile" 'TEST_DATABASE_URL="\$\$TEST_DATABASE_URL" \$\(CARGO\) test -p sourcebot-api --bin sourcebot-api auth_repository_sync_jobs_ -- --nocapture --test-threads=1' 'sqlx-test auth repository sync jobs command'
require_line "$makefile" 'TEST_DATABASE_URL="\$\$TEST_DATABASE_URL" \$\(CARGO\) test -p sourcebot-api --bin sourcebot-api pg_ask_thread_store_ -- --nocapture --test-threads=1' 'sqlx-test postgres ask-thread command'
require_line "$makefile" 'TEST_DATABASE_URL="\$\$TEST_DATABASE_URL" \$\(CARGO\) test -p sourcebot-api --bin sourcebot-api invite_redeem_ -- --nocapture --test-threads=1' 'sqlx-test invite redeem command'

require_line "$readme" '^5\. Run the focused metadata-schema test wrapper:$' 'README sqlx-test step'
require_line "$readme" '^   make sqlx-test$' 'README make sqlx-test command'
require_line "$readme" '^9\. `make sqlx-test` wraps the deterministic reset plus the focused `sourcebot-api` metadata storage, PostgreSQL-backed catalog list/detail, PostgreSQL-backed bootstrap-admin, durable local-session, PostgreSQL-backed local-account/membership/invite auth, PostgreSQL-backed repository-permission filtering for authenticated sync-job history, PostgreSQL-backed repository-sync-job lifecycle storage, PostgreSQL-backed review-agent-run lifecycle storage, PostgreSQL-backed ask-thread/message storage, and durable API-key/OAuth-client metadata regressions so local migration workflow verification uses one reproducible command\.$' 'README sqlx-test workflow note'
require_line "$readme" '^10\. `make sqlx-test` now covers the storage migration-inventory/catalog fallback checks, PostgreSQL-backed catalog list/detail queries for `/api/v1/repos` and repository detail reads, the PostgreSQL-backed bootstrap store and `/api/v1/auth/bootstrap` \+ login regressions, the PostgreSQL-backed local-session store and login -> `/api/v1/auth/me` regression slice, the durable PostgreSQL local-account lookup/linked-account membership/member-roster/invite-redeem regressions, authenticated `/api/v1/auth/repository-sync-jobs` filtering against PostgreSQL-backed repository permissions, PostgreSQL-backed repository-sync-job upsert/claim/complete lifecycle regressions, PostgreSQL-backed review-agent-run store/merge/claim/complete/fail lifecycle regressions, PostgreSQL-backed ask-thread create/append/list/detail owner-scoped regressions, plus PostgreSQL-backed API-key inventory/create/revoke/bearer-auth and OAuth-client inventory/create regressions; catalog read metadata, bootstrap-admin, invited-account login, auth-me identity restoration, member rosters, linked-account memberships, invite acceptance, sync-job permission filtering and lifecycle rows, review-agent-run lifecycle rows, ask-thread messages, API-key metadata, and OAuth-client metadata now all stay durable across API restarts when `DATABASE_URL` is configured\.$' 'README sqlx-test truthful scope note'

printf 'sqlx-test workflow contract OK\n'
