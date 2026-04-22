#!/bin/sh
set -eu

repo_root=${1:-.}
backup_script="$repo_root/scripts/backup_local_runtime_state.sh"
restore_script="$repo_root/scripts/restore_local_runtime_state.sh"

require_file() {
  file=$1
  description=$2

  if [ ! -f "$file" ]; then
    printf 'missing %s at %s\n' "$description" "$file" >&2
    exit 1
  fi
}

require_file "$backup_script" 'runtime backup helper'
require_file "$restore_script" 'runtime restore helper'

tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT INT TERM

runtime_dir="$tmpdir/runtime"
backups_dir="$tmpdir/backups"
mkdir -p "$runtime_dir" "$backups_dir"

cat > "$runtime_dir/bootstrap-state.json" <<'EOF'
{"is_initialized":true}
EOF
cat > "$runtime_dir/local-sessions.json" <<'EOF'
{"sessions":[]}
EOF
cat > "$runtime_dir/organizations.json" <<'EOF'
{"organizations":[]}
EOF

export SOURCEBOT_DATA_DIR="$runtime_dir"

first_backup=$(bash "$backup_script" "$backups_dir")
second_backup=$(bash "$backup_script" "$backups_dir")

if [ "$first_backup" = "$second_backup" ]; then
  printf 'backup helper reused the same directory for consecutive runs: %s\n' "$first_backup" >&2
  exit 1
fi

if [ ! -d "$first_backup" ] || [ ! -d "$second_backup" ]; then
  printf 'expected both backup directories to exist\n' >&2
  exit 1
fi

printf 'runtime backup uniqueness OK\n'
