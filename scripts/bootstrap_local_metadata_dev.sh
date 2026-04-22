#!/bin/sh
set -eu

: "${DATABASE_URL:?DATABASE_URL must be set}"
: "${TEST_DATABASE_URL:?TEST_DATABASE_URL must be set}"

require_local_postgres_url() {
  url=$1
  label=$2

  case "$url" in
    postgres://*@127.0.0.1:5432/*) return 0 ;;
    postgres://*@localhost:5432/*) return 0 ;;
    *)
      printf '%s\n' "$label must target postgres on 127.0.0.1:5432 or localhost:5432" >&2
      exit 1
      ;;
  esac
}

url_db_name() {
  url=$1
  printf '%s' "${url##*/}"
}

require_safe_database_name() {
  db_name=$1
  label=$2

  case "$db_name" in
    ''|*[!A-Za-z0-9_]*)
      printf '%s\n' "$label must use only letters, numbers, and underscores" >&2
      exit 1
      ;;
  esac
}

admin_database_url() {
  url=$1
  printf '%s/postgres' "${url%/*}"
}

wait_for_ready() {
  url=$1

  attempts=0
  while [ "$attempts" -lt 30 ]; do
    if pg_isready -d "$url" >/dev/null 2>&1; then
      return 0
    fi
    attempts=$((attempts + 1))
    sleep 1
  done

  printf 'postgres did not become ready for %s\n' "$url" >&2
  exit 1
}

ensure_database_exists() {
  db_name=$1
  admin_url=$2

  exists=$(psql "$admin_url" -tAc "SELECT 1 FROM pg_database WHERE datname = '$db_name'" 2>/dev/null || true)
  if [ "$exists" = "1" ]; then
    return 0
  fi

  psql "$admin_url" -c "CREATE DATABASE \"$db_name\""
}

main_db=$(url_db_name "$DATABASE_URL")
test_db=$(url_db_name "$TEST_DATABASE_URL")

require_local_postgres_url "$DATABASE_URL" 'DATABASE_URL'
require_local_postgres_url "$TEST_DATABASE_URL" 'TEST_DATABASE_URL'
require_safe_database_name "$main_db" 'DATABASE_URL database name'
require_safe_database_name "$test_db" 'TEST_DATABASE_URL database name'

case "$main_db" in
  sourcebot) ;;
  *)
    printf '%s\n' 'DATABASE_URL must target the dedicated local sourcebot database' >&2
    exit 1
    ;;
esac

case "$test_db" in
  sourcebot_test) ;;
  *)
    printf '%s\n' 'TEST_DATABASE_URL must target the dedicated local sourcebot_test database' >&2
    exit 1
    ;;
esac

wait_for_ready "$DATABASE_URL"
ensure_database_exists "$test_db" "$(admin_database_url "$DATABASE_URL")"
wait_for_ready "$TEST_DATABASE_URL"

make sqlx-migrate
make sqlx-test
