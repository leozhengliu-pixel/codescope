#!/usr/bin/env bash
set -euo pipefail

repo_root=${1:-$(pwd)}
smoke_script="$repo_root/scripts/check_end_to_end_smoke_matrix.sh"
stdout_file=$(mktemp)
stderr_file=$(mktemp)
trap 'rm -f "$stdout_file" "$stderr_file"' EXIT INT TERM

set +e
bash "$smoke_script" "$repo_root" >"$stdout_file" 2>"$stderr_file"
status=$?
set -e

if [[ $status -ne 0 ]]; then
  echo "FAIL: end-to-end smoke matrix command exited with status $status" >&2
  echo "--- stdout ---" >&2
  cat "$stdout_file" >&2
  echo "--- stderr ---" >&2
  cat "$stderr_file" >&2
  exit 1
fi

combined_file=$(mktemp)
trap 'rm -f "$stdout_file" "$stderr_file" "$combined_file"' EXIT INT TERM
cat "$stdout_file" "$stderr_file" >"$combined_file"

for marker in auth integrations search ask review-agent "[repository-sync] worker completed queued sync job" "SMOKE MATRIX PASS"; do
  if ! grep -Fq "$marker" "$combined_file"; then
    echo "FAIL: missing smoke marker '$marker'" >&2
    echo "--- stdout ---" >&2
    cat "$stdout_file" >&2
    echo "--- stderr ---" >&2
    cat "$stderr_file" >&2
    exit 1
  fi
done

echo "PASS: end-to-end smoke matrix contract verified"
