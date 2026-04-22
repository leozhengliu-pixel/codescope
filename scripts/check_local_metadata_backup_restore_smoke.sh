#!/bin/sh
set -eu

repo_root=${1:-.}
backup_script="$repo_root/scripts/backup_local_metadata_db.sh"
restore_script="$repo_root/scripts/restore_local_metadata_db.sh"

require_file() {
  file=$1
  description=$2

  if [ ! -x "$file" ]; then
    printf 'missing executable %s at %s\n' "$description" "$file" >&2
    exit 1
  fi
}

expect_failure() {
  description=$1
  shift

  if "$@" >/tmp/check_local_metadata_backup_restore_smoke.stdout 2>/tmp/check_local_metadata_backup_restore_smoke.stderr; then
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

require_file "$backup_script" 'metadata backup script'
require_file "$restore_script" 'metadata restore script'

workdir=$(mktemp -d)
trap 'rm -rf "$workdir" /tmp/check_local_metadata_backup_restore_smoke.stdout /tmp/check_local_metadata_backup_restore_smoke.stderr' EXIT INT TERM

bin_dir="$workdir/bin"
mkdir -p "$bin_dir"
log_file="$workdir/command.log"

cat > "$bin_dir/pg_dump" <<'EOF'
#!/bin/sh
set -eu
log_file=${TEST_COMMAND_LOG:?TEST_COMMAND_LOG must be set}
printf 'pg_dump %s\n' "$*" >> "$log_file"
output_file=''
while [ "$#" -gt 0 ]; do
  case "$1" in
    --file)
      shift
      output_file=$1
      ;;
    --file=*)
      output_file=${1#--file=}
      ;;
  esac
  shift || true
done
: "${output_file:?pg_dump stub expected --file}"
printf '%s\n' 'stub metadata dump' > "$output_file"
EOF
chmod +x "$bin_dir/pg_dump"

cat > "$bin_dir/psql" <<'EOF'
#!/bin/sh
set -eu
log_file=${TEST_COMMAND_LOG:?TEST_COMMAND_LOG must be set}
printf 'psql %s\n' "$*" >> "$log_file"
dump_file=''
while [ "$#" -gt 0 ]; do
  case "$1" in
    -f)
      shift
      dump_file=$1
      ;;
  esac
  shift || true
done
: "${dump_file:?psql stub expected -f dump file}"
[ -f "$dump_file" ]
EOF
chmod +x "$bin_dir/psql"

expect_failure 'backup requires DATABASE_URL' env PATH="$bin_dir:$PATH" TEST_COMMAND_LOG="$log_file" sh "$backup_script" "$workdir/backups"
expect_failure 'restore requires DATABASE_URL' env PATH="$bin_dir:$PATH" TEST_COMMAND_LOG="$log_file" sh "$restore_script" "$workdir/backups/missing"
expect_failure 'backup refuses remote hosts' env PATH="$bin_dir:$PATH" TEST_COMMAND_LOG="$log_file" DATABASE_URL='postgres://user:pass@example.com:5432/sourcebot' sh "$backup_script" "$workdir/backups"
expect_failure 'restore refuses remote hosts' env PATH="$bin_dir:$PATH" TEST_COMMAND_LOG="$log_file" DATABASE_URL='postgres://user:pass@example.com:5432/sourcebot' sh "$restore_script" "$workdir/backups/missing"
expect_failure 'backup refuses non-postgres schemes' env PATH="$bin_dir:$PATH" TEST_COMMAND_LOG="$log_file" DATABASE_URL='mysql://user:pass@127.0.0.1:5432/sourcebot' sh "$backup_script" "$workdir/backups"
expect_failure 'restore refuses non-postgres schemes' env PATH="$bin_dir:$PATH" TEST_COMMAND_LOG="$log_file" DATABASE_URL='mysql://user:pass@127.0.0.1:5432/sourcebot' sh "$restore_script" "$workdir/backups/missing"

backup_dir=$(env PATH="$bin_dir:$PATH" TEST_COMMAND_LOG="$log_file" DATABASE_URL='postgres://user:pass@127.0.0.1:5432/sourcebot' sh "$backup_script" "$workdir/backups")
[ -d "$backup_dir" ]
[ -f "$backup_dir/dump.sql" ]
[ -f "$backup_dir/manifest.txt" ]
require_line "$backup_dir/manifest.txt" '^backup_timestamp_utc=' 'backup manifest timestamp'
require_line "$backup_dir/manifest.txt" '^database_url_redacted=postgres://user:\*\*\*@127\.0\.0\.1:5432/sourcebot$' 'backup manifest redacted database url'
require_line "$backup_dir/manifest.txt" '^local_only_backup=true$' 'backup manifest local-only marker'
require_line "$log_file" 'pg_dump --file .*/dump\.sql postgres://user:pass@127\.0\.0\.1:5432/sourcebot' 'pg_dump invocation log'

rm -f "$backup_dir/manifest.txt"
expect_failure 'restore requires manifest' env PATH="$bin_dir:$PATH" TEST_COMMAND_LOG="$log_file" DATABASE_URL='postgres://user:pass@127.0.0.1:5432/sourcebot' sh "$restore_script" "$backup_dir"
cat > "$backup_dir/manifest.txt" <<'EOF'
backup_timestamp_utc=20260422T000000Z
database_url_redacted=postgres://user:***@127.0.0.1:5432/sourcebot
local_only_backup=false
EOF
expect_failure 'restore requires local-only marker' env PATH="$bin_dir:$PATH" TEST_COMMAND_LOG="$log_file" DATABASE_URL='postgres://user:pass@127.0.0.1:5432/sourcebot' sh "$restore_script" "$backup_dir"
cat > "$backup_dir/manifest.txt" <<'EOF'
backup_timestamp_utc=20260422T000000Z
local_only_backup=true
EOF
expect_failure 'restore requires redacted database url' env PATH="$bin_dir:$PATH" TEST_COMMAND_LOG="$log_file" DATABASE_URL='postgres://user:pass@127.0.0.1:5432/sourcebot' sh "$restore_script" "$backup_dir"
cat > "$backup_dir/manifest.txt" <<'EOF'
backup_timestamp_utc=20260422T000000Z
database_url_redacted=postgres://user:***@127.0.0.1:5432/sourcebot
local_only_backup=true
EOF

env PATH="$bin_dir:$PATH" TEST_COMMAND_LOG="$log_file" DATABASE_URL='postgres://user:pass@127.0.0.1:5432/sourcebot' sh "$restore_script" "$backup_dir"
require_line "$log_file" 'psql postgres://user:pass@127\.0\.0\.1:5432/sourcebot -v ON_ERROR_STOP=1 -f .*/dump\.sql' 'psql restore invocation log'

printf 'local metadata backup/restore smoke OK\n'
