#!/bin/sh
set -eu

backup_root=${1:-}
if [ -z "$backup_root" ]; then
  printf 'usage: %s BACKUP_ROOT\n' "$0" >&2
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

write_default_json_if_missing() {
  path=$1
  default_json=$2

  if [ -f "$path" ]; then
    return 0
  fi

  ensure_parent_dir "$path"
  printf '%s\n' "$default_json" > "$path"
}

copy_required_file() {
  source_path=$1
  target_path=$2
  description=$3

  if [ ! -f "$source_path" ]; then
    printf 'missing %s at %s\n' "$description" "$source_path" >&2
    exit 1
  fi

  cp "$source_path" "$target_path"
}

bootstrap_path=$(runtime_state_path SOURCEBOT_BOOTSTRAP_STATE_PATH .sourcebot/bootstrap-state.json bootstrap-state.json)
local_sessions_path=$(runtime_state_path SOURCEBOT_LOCAL_SESSION_STATE_PATH .sourcebot/local-sessions.json local-sessions.json)
organizations_path=$(runtime_state_path SOURCEBOT_ORGANIZATION_STATE_PATH .sourcebot/organizations.json organizations.json)

timestamp=$(date -u +%Y%m%dT%H%M%SZ)
mkdir -p "$backup_root"
backup_dir=$(mktemp -d "$backup_root/${timestamp}-XXXXXX")

write_default_json_if_missing "$local_sessions_path" '{"sessions":[]}'
write_default_json_if_missing "$organizations_path" '{"organizations":[]}'

copy_required_file "$bootstrap_path" "$backup_dir/bootstrap-state.json" 'bootstrap state file'
copy_required_file "$local_sessions_path" "$backup_dir/local-sessions.json" 'local session state file'
copy_required_file "$organizations_path" "$backup_dir/organizations.json" 'organization state file'

cat > "$backup_dir/manifest.txt" <<EOF
backup_timestamp_utc=$timestamp
bootstrap_state_path=$bootstrap_path
local_session_state_path=$local_sessions_path
organization_state_path=$organizations_path
EOF

printf '%s\n' "$backup_dir"
