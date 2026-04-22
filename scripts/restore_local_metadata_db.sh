#!/bin/sh
set -eu

backup_dir=${1:-}
if [ -z "$backup_dir" ]; then
  printf 'usage: %s BACKUP_DIR\n' "$0" >&2
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

require_backup_file() {
  path=$1
  description=$2

  if [ ! -f "$path" ]; then
    printf 'missing %s at %s\n' "$description" "$path" >&2
    exit 1
  fi
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

require_manifest_line() {
  manifest=$1
  pattern=$2
  description=$3

  if ! grep -Eq "$pattern" "$manifest"; then
    printf 'missing %s in %s\n' "$description" "$manifest" >&2
    exit 1
  fi
}

if [ ! -d "$backup_dir" ]; then
  printf 'backup directory does not exist: %s\n' "$backup_dir" >&2
  exit 1
fi

require_postgres_database_url
require_local_database_url
command -v psql >/dev/null 2>&1 || {
  printf 'psql is required but was not found in PATH\n' >&2
  exit 1
}

require_backup_file "$backup_dir/dump.sql" 'backup metadata dump'
require_backup_file "$backup_dir/manifest.txt" 'backup manifest'
require_manifest_line "$backup_dir/manifest.txt" '^local_only_backup=true$' 'local-only backup marker'
require_manifest_line "$backup_dir/manifest.txt" '^database_url_redacted=' 'redacted database url'

expected_redacted_url=$(redacted_database_url)
if ! grep -Fxq "database_url_redacted=$expected_redacted_url" "$backup_dir/manifest.txt"; then
  printf 'backup manifest target does not match current DATABASE_URL\n' >&2
  exit 1
fi

psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f "$backup_dir/dump.sql"

printf 'restored metadata database from %s\n' "$backup_dir"
