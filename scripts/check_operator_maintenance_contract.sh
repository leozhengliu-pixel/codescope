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
metadata_backup_script="$repo_root/scripts/backup_local_metadata_db.sh"
metadata_restore_script="$repo_root/scripts/restore_local_metadata_db.sh"

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
require_file "$metadata_backup_script" 'metadata backup helper'
require_file "$metadata_restore_script" 'metadata restore helper'
require_file "$acceptance_doc" 'operator maintenance acceptance doc'
require_file "$acceptance_index" 'acceptance index'
require_file "$journeys_doc" 'acceptance journeys doc'
require_file "$parity_report" 'parity gap report'

require_line "$makefile" '^\.PHONY: .*runtime-backup.*runtime-restore.*metadata-backup.*metadata-restore' 'Makefile backup/restore .PHONY entry'
require_line "$makefile" "^help:$" 'Makefile help target'
require_line "$makefile" "make runtime-backup[[:space:]]+- create a timestamped backup of the current local runtime state" 'Makefile help text for runtime-backup'
require_line "$makefile" "make runtime-restore BACKUP_DIR=/path/to/backup[[:space:]]+- restore the local runtime state from a captured backup directory" 'Makefile help text for runtime-restore'
require_line "$makefile" "make metadata-backup[[:space:]]+- create a timestamped backup of the current local metadata database" 'Makefile help text for metadata-backup'
require_line "$makefile" "make metadata-restore BACKUP_DIR=/path/to/backup[[:space:]]+- restore the local metadata database from a captured backup directory" 'Makefile help text for metadata-restore'
require_line "$makefile" '^runtime-backup:$' 'Makefile runtime-backup target'
require_line "$makefile" '^[[:space:]]+bash scripts/backup_local_runtime_state\.sh backups/runtime$' 'Makefile runtime-backup command'
require_line "$makefile" '^runtime-restore:$' 'Makefile runtime-restore target'
require_line "$makefile" 'BACKUP_DIR:\?BACKUP_DIR must be set' 'Makefile BACKUP_DIR guard'
require_line "$makefile" '^[[:space:]]+bash scripts/restore_local_runtime_state\.sh "\$\$BACKUP_DIR"$' 'Makefile runtime-restore command'
require_line "$makefile" '^metadata-backup:$' 'Makefile metadata-backup target'
require_line "$makefile" 'DATABASE_URL:\?DATABASE_URL must be set' 'Makefile DATABASE_URL guard'
require_line "$makefile" '^[[:space:]]+bash scripts/backup_local_metadata_db\.sh backups/metadata$' 'Makefile metadata-backup command'
require_line "$makefile" '^metadata-restore:$' 'Makefile metadata-restore target'
require_line "$makefile" '^[[:space:]]+bash scripts/restore_local_metadata_db\.sh "\$\$BACKUP_DIR"$' 'Makefile metadata-restore command'

require_line "$readme" '^## Local operator maintenance baseline$' 'README operator maintenance heading'
require_line "$readme" '^1\. Capture a backup of the current file-backed runtime state before maintenance:$' 'README runtime backup step'
require_line "$readme" '^2\. Record the runtime backup directory emitted by the helper; it contains copies of `bootstrap-state\.json`, `local-sessions\.json`, `organizations\.json`, and a manifest for the resolved runtime paths\.$' 'README runtime backup manifest note'
require_line "$readme" '^3\. Start or confirm the local metadata dependency before metadata backup or schema maintenance:$' 'README metadata prerequisite step'
require_line "$readme" '^   make dev-up$' 'README make dev-up maintenance command'
require_line "$readme" '^4\. Capture a backup of the current local metadata database before schema maintenance:$' 'README metadata backup step'
require_line "$readme" '^   make metadata-backup$' 'README metadata-backup command'
require_line "$readme" '^5\. Record the metadata backup directory emitted by the helper; it contains a SQL dump and manifest for the current local `DATABASE_URL` target without storing plaintext credentials\.$' 'README metadata manifest note'
require_line "$readme" '^6\. Run the current local SQLx migration workflow:$' 'README sqlx-migrate maintenance step'
require_line "$readme" '^   make sqlx-migrate$' 'README make sqlx-migrate maintenance command'
require_line "$readme" '^7\. Treat upgrades as a repo update plus migration plus local process restart sequence:$' 'README upgrade step'
require_line "$readme" '^   git pull --ff-only$' 'README git pull upgrade command'
require_line "$readme" '^   make api$' 'README make api upgrade command'
require_line "$readme" '^   make worker$' 'README make worker upgrade command'
require_line "$readme" '^8\. If maintenance fails, restore the file-backed runtime baseline from the captured runtime backup directory:$' 'README runtime restore step'
require_line "$readme" '^   BACKUP_DIR=/absolute/path/to/backups/runtime/' 'README runtime BACKUP_DIR example prefix'
require_line "$readme" '^   make runtime-restore BACKUP_DIR="\$BACKUP_DIR"$' 'README make runtime-restore command'
require_line "$readme" '^9\. If maintenance fails after a metadata change, restore the local metadata database from the captured metadata backup directory:$' 'README metadata restore step'
require_line "$readme" '^   BACKUP_DIR=/absolute/path/to/backups/metadata/' 'README metadata BACKUP_DIR example prefix'
require_line "$readme" '^   make metadata-restore BACKUP_DIR="\$BACKUP_DIR"$' 'README make metadata-restore command'
require_line "$readme" '^10\. The metadata backup/restore helpers intentionally stay local-only for this baseline: they require `DATABASE_URL` to target `127\.0\.0\.1` or `localhost`, validate a matching redacted manifest on restore, and rely on `pg_dump`/`psql` from the local operator environment\.$' 'README local-only metadata note'
require_line "$readme" '^11\. This maintenance baseline now covers the current file-backed runtime state plus the local Postgres metadata dump/restore workflow, but it still does not claim that every product/runtime surface is durable yet, nor does it claim readiness checks or production-grade deployment automation\.$' 'README truthful scope note'

require_line "$acceptance_doc" '^# Operator Maintenance Acceptance$' 'operator maintenance acceptance heading'
require_line "$acceptance_doc" 'local operator-maintenance baseline' 'operator maintenance acceptance scope'
require_line "$acceptance_doc" 'file-backed runtime state' 'operator maintenance acceptance runtime scope'
require_line "$acceptance_doc" 'local Postgres metadata database' 'operator maintenance acceptance metadata scope'
require_line "$acceptance_doc" 'does \*\*not\*\* claim:' 'operator maintenance acceptance limits heading'
require_line "$acceptance_doc" 'still-undurable metadata surfaces' 'operator maintenance acceptance deferred metadata note'

require_line "$acceptance_index" 'Operator maintenance parity' 'acceptance index operator maintenance row'
require_line "$acceptance_index" 'specs/acceptance/operator-maintenance\.md' 'acceptance index operator maintenance spec path'
require_line "$acceptance_index" 'metadata-backup' 'acceptance index metadata-backup reference'
require_line "$journeys_doc" 'operator maintenance baseline' 'journeys operator maintenance reference'
require_line "$journeys_doc" 'specs/acceptance/operator-maintenance\.md' 'journeys operator maintenance spec path'
require_line "$journeys_doc" 'metadata-backup' 'journeys metadata-backup reference'

require_line "$parity_report" 'runtime-backup' 'parity report runtime backup helper evidence'
require_line "$parity_report" 'metadata-backup' 'parity report metadata backup helper evidence'
require_line "$parity_report" 'local Postgres SQL dump' 'parity report metadata dump wording'
require_line "$parity_report" 'does not yet mean every runtime surface is durable' 'parity report truthful durable limitation'

printf 'operator maintenance contract OK\n'
