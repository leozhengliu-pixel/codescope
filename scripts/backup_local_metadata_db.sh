#!/bin/sh
set -eu

backup_root=${1:-}
if [ -z "$backup_root" ]; then
  printf 'usage: %s BACKUP_ROOT\n' "$0" >&2
  exit 1
fi

: "${DATABASE_URL:?DATABASE_URL must be set}"

parse_database_host() {
  authority_and_path=${DATABASE_URL#*://}
  authority=${authority_and_path%%/*}
  case "$authority" in
    *@*) hostport=${authority#*@} ;;
    *) hostport=$authority ;;
  esac

  case "$hostport" in
    \[*\]:*)
      host=${hostport#\[}
      host=${host%%\]:*}
      ;;
    \[*\])
      host=${hostport#\[}
      host=${host%\]}
      ;;
    *:*)
      host=${hostport%%:*}
      ;;
    *)
      host=$hostport
      ;;
  esac

  printf '%s\n' "$host"
}

require_local_database_url() {
  host=$(parse_database_host)
  case "$host" in
    127.0.0.1|localhost) ;;
    *)
      printf 'DATABASE_URL must target a local postgres host (127.0.0.1 or localhost), got %s\n' "$host" >&2
      exit 1
      ;;
  esac
}

require_postgres_database_url() {
  case "$DATABASE_URL" in
    postgres://*|postgresql://*) ;;
    *)
      printf 'DATABASE_URL must use a postgres:// or postgresql:// scheme\n' >&2
      exit 1
      ;;
  esac
}

redacted_database_url() {
  printf '%s\n' "$DATABASE_URL" | sed 's#^\([^:]*://[^:]*:\)[^@]*@#\1***@#'
}

require_postgres_database_url
require_local_database_url
command -v pg_dump >/dev/null 2>&1 || {
  printf 'pg_dump is required but was not found in PATH\n' >&2
  exit 1
}

timestamp=$(date -u +%Y%m%dT%H%M%SZ)
mkdir -p "$backup_root"
backup_dir=$(mktemp -d "$backup_root/${timestamp}-XXXXXX")
dump_file="$backup_dir/dump.sql"

pg_dump --file "$dump_file" "$DATABASE_URL"

cat > "$backup_dir/manifest.txt" <<EOF
backup_timestamp_utc=$timestamp
database_url_redacted=$(redacted_database_url)
local_only_backup=true
EOF

printf '%s\n' "$backup_dir"
