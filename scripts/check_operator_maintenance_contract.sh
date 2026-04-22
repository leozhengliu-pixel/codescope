#!/bin/sh
set -eu

repo_root=${1:-.}
makefile="$repo_root/Makefile"
readme="$repo_root/README.md"
acceptance_doc="$repo_root/specs/acceptance/operator-maintenance.md"
acceptance_index="$repo_root/specs/acceptance/index.md"
journeys_doc="$repo_root/specs/acceptance/journeys.md"
parity_report="$repo_root/docs/reports/2026-04-18-parity-gap-report.md"
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

require_line() {
  file=$1
  pattern=$2
  description=$3

  if ! grep -Eq "$pattern" "$file"; then
    printf 'missing %s in %s\n' "$description" "$file" >&2
    exit 1
  fi
}

require_file "$makefile" 'Makefile'
require_file "$readme" 'README'
require_file "$backup_script" 'runtime backup helper'
require_file "$restore_script" 'runtime restore helper'
require_file "$acceptance_doc" 'operator maintenance acceptance doc'
require_file "$acceptance_index" 'acceptance index'
require_file "$journeys_doc" 'acceptance journeys doc'
require_file "$parity_report" 'parity gap report'

require_line "$makefile" '^\.PHONY: .*runtime-backup.*runtime-restore' 'Makefile runtime-backup/runtime-restore .PHONY entry'
require_line "$makefile" "^help:$" 'Makefile help target'
require_line "$makefile" "make runtime-backup[[:space:]]+- create a timestamped backup of the current local runtime state" 'Makefile help text for runtime-backup'
require_line "$makefile" "make runtime-restore BACKUP_DIR=/path/to/backup[[:space:]]+- restore the local runtime state from a captured backup directory" 'Makefile help text for runtime-restore'
require_line "$makefile" '^runtime-backup:$' 'Makefile runtime-backup target'
require_line "$makefile" '^[[:space:]]+bash scripts/backup_local_runtime_state\.sh backups/runtime$' 'Makefile runtime-backup command'
require_line "$makefile" '^runtime-restore:$' 'Makefile runtime-restore target'
require_line "$makefile" 'BACKUP_DIR:\?BACKUP_DIR must be set' 'Makefile BACKUP_DIR guard'
require_line "$makefile" '^[[:space:]]+bash scripts/restore_local_runtime_state\.sh "\$\$BACKUP_DIR"$' 'Makefile runtime-restore command'

require_line "$readme" '^## Local operator maintenance baseline$' 'README operator maintenance heading'
require_line "$readme" '^1\. Capture a backup of the current file-backed runtime state before maintenance:$' 'README backup step'
require_line "$readme" '^   make runtime-backup$' 'README runtime-backup command'
require_line "$readme" '^2\. Record the backup directory emitted by the helper; it contains copies of `bootstrap-state\.json`, `local-sessions\.json`, `organizations\.json`, and a manifest for the resolved runtime paths\.$' 'README backup manifest note'
require_line "$readme" '^3\. Start or confirm the local metadata dependency before schema maintenance:$' 'README migration prerequisite step'
require_line "$readme" '^   make dev-up$' 'README make dev-up maintenance command'
require_line "$readme" '^4\. Run the current local SQLx migration workflow:$' 'README sqlx-migrate maintenance step'
require_line "$readme" '^   make sqlx-migrate$' 'README make sqlx-migrate maintenance command'
require_line "$readme" '^5\. Treat upgrades as a repo update plus migration plus local process restart sequence:$' 'README upgrade step'
require_line "$readme" '^   git pull --ff-only$' 'README git pull upgrade command'
require_line "$readme" '^   make api$' 'README make api upgrade command'
require_line "$readme" '^   make worker$' 'README make worker upgrade command'
require_line "$readme" '^6\. If maintenance fails, restore the file-backed runtime baseline from the captured backup directory:$' 'README restore step'
require_line "$readme" '^   BACKUP_DIR=/absolute/path/to/backups/runtime/' 'README BACKUP_DIR example prefix'
require_line "$readme" '^   make runtime-restore BACKUP_DIR="\$BACKUP_DIR"$' 'README make runtime-restore command'
require_line "$readme" '^7\. This maintenance baseline is intentionally narrow: it covers only the current file-backed runtime state plus the local SQLx migration workflow, and it does not yet claim durable metadata backup/restore parity, readiness checks, or production-grade deployment automation\.$' 'README truthful scope note'

require_line "$acceptance_doc" '^# Operator Maintenance Acceptance$' 'operator maintenance acceptance heading'
require_line "$acceptance_doc" 'current local operator-maintenance baseline' 'operator maintenance acceptance scope'
require_line "$acceptance_doc" 'file-backed runtime state' 'operator maintenance acceptance file-backed scope'
require_line "$acceptance_doc" 'local SQLx migration workflow' 'operator maintenance acceptance migration scope'
require_line "$acceptance_doc" 'does \*\*not\*\* claim:' 'operator maintenance acceptance limits heading'
require_line "$acceptance_doc" 'durable metadata backup/restore parity' 'operator maintenance acceptance deferred durable backup/restore note'

require_line "$acceptance_index" 'Operator maintenance parity' 'acceptance index operator maintenance row'
require_line "$acceptance_index" 'specs/acceptance/operator-maintenance\.md' 'acceptance index operator maintenance spec path'
require_line "$journeys_doc" 'operator maintenance baseline' 'journeys operator maintenance reference'
require_line "$journeys_doc" 'specs/acceptance/operator-maintenance\.md' 'journeys operator maintenance spec path'

require_line "$parity_report" 'runtime-backup' 'parity report backup helper evidence'
require_line "$parity_report" 'runtime-restore' 'parity report restore helper evidence'
require_line "$parity_report" 'local SQLx migration workflow' 'parity report migration workflow wording'
require_line "$parity_report" 'does not yet claim durable metadata backup/restore parity' 'parity report truthful backup parity wording'

printf 'operator maintenance contract OK\n'
