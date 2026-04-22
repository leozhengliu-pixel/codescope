#!/bin/sh
set -eu

repo_root=${1:-.}
script="$repo_root/scripts/bootstrap_local_metadata_dev.sh"

expect_failure() {
  description=$1
  shift

  if "$@" >"$out_file" 2>"$err_file"; then
    printf 'expected failure for %s\n' "$description" >&2
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

workdir=$(mktemp -d)
out_file="$workdir/stdout.log"
err_file="$workdir/stderr.log"
trap 'rm -rf "$workdir"' EXIT INT TERM

expect_failure 'requires DATABASE_URL' env sh "$script"
expect_failure 'requires TEST_DATABASE_URL' env DATABASE_URL='postgres://user:***@127.0.0.1:5432/sourcebot' sh "$script"
expect_failure 'rejects non-sourcebot main database names' env DATABASE_URL='postgres://user:***@127.0.0.1:5432/foo' TEST_DATABASE_URL='postgres://user:***@127.0.0.1:5432/sourcebot_test' sh "$script"
expect_failure 'rejects non-sourcebot test database names' env DATABASE_URL='postgres://user:***@127.0.0.1:5432/sourcebot' TEST_DATABASE_URL='postgres://user:***@127.0.0.1:5432/foo_test' sh "$script"
expect_failure 'rejects unsafe database names' env DATABASE_URL='postgres://user:***@127.0.0.1:5432/sourcebot' TEST_DATABASE_URL='postgres://user:***@127.0.0.1:5432/sourcebot-test' sh "$script"

bin_dir="$workdir/bin"
mkdir -p "$bin_dir"
log_file="$workdir/command.log"

cat > "$bin_dir/pg_isready" <<'EOF'
#!/bin/sh
set -eu
log_file=${TEST_COMMAND_LOG:?TEST_COMMAND_LOG must be set}
printf 'pg_isready %s\n' "$*" >> "$log_file"
exit 0
EOF
chmod +x "$bin_dir/pg_isready"

cat > "$bin_dir/psql" <<'EOF'
#!/bin/sh
set -eu
log_file=${TEST_COMMAND_LOG:?TEST_COMMAND_LOG must be set}
printf 'psql %s\n' "$*" >> "$log_file"
if printf '%s' "$*" | grep -q 'SELECT 1 FROM pg_database'; then
  exit 1
fi
exit 0
EOF
chmod +x "$bin_dir/psql"

cat > "$bin_dir/make" <<'EOF'
#!/bin/sh
set -eu
log_file=${TEST_COMMAND_LOG:?TEST_COMMAND_LOG must be set}
printf 'make %s\n' "$*" >> "$log_file"
EOF
chmod +x "$bin_dir/make"

env \
  PATH="$bin_dir:$PATH" \
  TEST_COMMAND_LOG="$log_file" \
  DATABASE_URL='postgres://user:***@127.0.0.1:5432/sourcebot' \
  TEST_DATABASE_URL='postgres://user:***@127.0.0.1:5432/sourcebot_test' \
  sh "$script"

require_line "$log_file" '^pg_isready -d postgres://user:\*\*\*@127\.0\.0\.1:5432/sourcebot$' 'pg_isready DATABASE_URL check'
require_line "$log_file" '^pg_isready -d postgres://user:\*\*\*@127\.0\.0\.1:5432/sourcebot_test$' 'pg_isready TEST_DATABASE_URL check'
require_line "$log_file" 'SELECT 1 FROM pg_database WHERE datname = '\''sourcebot_test'\''' 'test database existence probe'
require_line "$log_file" 'CREATE DATABASE "sourcebot_test"' 'test database creation'
require_line "$log_file" '^make sqlx-migrate$' 'sqlx-migrate invocation'
require_line "$log_file" '^make sqlx-test$' 'sqlx-test invocation'

printf 'local metadata dev bootstrap smoke OK\n'
