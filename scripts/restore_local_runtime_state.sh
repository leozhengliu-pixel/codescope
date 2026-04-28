#!/bin/sh
set -eu

backup_dir=${1:-}
if [ -z "$backup_dir" ]; then
  printf 'usage: %s BACKUP_DIR\n' "$0" >&2
  exit 1
fi

configured_data_dir() {
  if [ "${SOURCEBOT_DATA_DIR+x}" = x ]; then
    trimmed=$(printf '%s' "$SOURCEBOT_DATA_DIR" | sed 's/^[[:space:]]*//; s/[[:space:]]*$//')
    if [ -n "$trimmed" ]; then
      printf '%s\n' "$trimmed"
      return 0
    fi
  fi

  return 1
}

runtime_state_path() {
  explicit_var=$1
  default_path=$2
  file_name=$3
  eval "explicit_value=\${$explicit_var-}"

  if [ -n "$explicit_value" ]; then
    printf '%s\n' "$explicit_value"
    return 0
  fi

  if data_dir=$(configured_data_dir); then
    printf '%s/%s\n' "$data_dir" "$file_name"
    return 0
  fi

  printf '%s\n' "$default_path"
}

ensure_parent_dir() {
  path=$1
  parent=$(dirname "$path")
  mkdir -p "$parent"
}

require_backup_file() {
  path=$1
  description=$2

  if [ ! -f "$path" ]; then
    printf 'missing %s at %s\n' "$description" "$path" >&2
    exit 1
  fi
}

manifest_value() {
  manifest=$1
  key=$2

  value=$(grep -E "^${key}=" "$manifest" | head -n 1 | cut -d= -f2- || true)
  if [ -z "$value" ]; then
    printf 'missing %s in %s\n' "$key" "$manifest" >&2
    exit 1
  fi

  printf '%s\n' "$value"
}

require_manifest_value() {
  manifest=$1
  key=$2
  expected=$3
  description=$4

  actual=$(manifest_value "$manifest" "$key")
  if [ "$actual" != "$expected" ]; then
    printf 'backup manifest %s does not match current %s path: expected %s, got %s\n' "$key" "$description" "$expected" "$actual" >&2
    exit 1
  fi
}

if [ ! -d "$backup_dir" ]; then
  printf 'backup directory does not exist: %s\n' "$backup_dir" >&2
  exit 1
fi

require_backup_file "$backup_dir/bootstrap-state.json" 'backup bootstrap state file'
require_backup_file "$backup_dir/local-sessions.json" 'backup local session state file'
require_backup_file "$backup_dir/organizations.json" 'backup organization state file'
require_backup_file "$backup_dir/manifest.txt" 'backup manifest'

bootstrap_path=$(runtime_state_path SOURCEBOT_BOOTSTRAP_STATE_PATH .sourcebot/bootstrap-state.json bootstrap-state.json)
local_sessions_path=$(runtime_state_path SOURCEBOT_LOCAL_SESSION_STATE_PATH .sourcebot/local-sessions.json local-sessions.json)
organizations_path=$(runtime_state_path SOURCEBOT_ORGANIZATION_STATE_PATH .sourcebot/organizations.json organizations.json)

require_manifest_value "$backup_dir/manifest.txt" 'bootstrap_state_path' "$bootstrap_path" 'bootstrap state'
require_manifest_value "$backup_dir/manifest.txt" 'local_session_state_path' "$local_sessions_path" 'local session state'
require_manifest_value "$backup_dir/manifest.txt" 'organization_state_path' "$organizations_path" 'organization state'

ensure_parent_dir "$bootstrap_path"
ensure_parent_dir "$local_sessions_path"
ensure_parent_dir "$organizations_path"

cp "$backup_dir/bootstrap-state.json" "$bootstrap_path"
cp "$backup_dir/local-sessions.json" "$local_sessions_path"
cp "$backup_dir/organizations.json" "$organizations_path"

printf 'restored runtime state from %s\n' "$backup_dir"
