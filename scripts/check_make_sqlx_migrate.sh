#!/bin/sh
set -eu

makefile=${1:-Makefile}

require_line() {
  pattern=$1
  description=$2

  if ! grep -Eq "$pattern" "$makefile"; then
    printf 'missing %s\n' "$description" >&2
    exit 1
  fi
}

require_line '^\.PHONY:.*\bsqlx-migrate\b' 'sqlx-migrate phony target'
require_line "make sqlx-migrate[[:space:]]+- run SQLx database migrations for the metadata schema against DATABASE_URL" 'help entry for sqlx-migrate'
require_line '^SQLX_CLI_VERSION \?= 0\.8\.6$' 'pinned SQLX_CLI_VERSION'
require_line '^SQLX_CLI_ROOT \?= \.sqlx-cli$' 'repo-local SQLX_CLI_ROOT'
require_line '^sqlx-migrate:$' 'sqlx-migrate target'
require_line 'DATABASE_URL:\?DATABASE_URL must be set' 'DATABASE_URL guard'
require_line 'install --locked sqlx-cli --version \$\(SQLX_CLI_VERSION\) --no-default-features --features rustls,postgres --root \$\(SQLX_CLI_ROOT\)' 'reproducible sqlx-cli install command'
require_line '\$\(SQLX_CLI_ROOT\)/bin/sqlx migrate run --source crates/api/migrations' 'sqlx migrate run command'

printf 'sqlx-migrate Makefile contract OK\n'
