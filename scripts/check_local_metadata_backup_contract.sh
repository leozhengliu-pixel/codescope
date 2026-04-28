#!/bin/sh
set -eu

repo_root=${1:-.}
makefile="$repo_root/Makefile"
readme="$repo_root/README.md"
acceptance_doc="$repo_root/specs/acceptance/operator-maintenance.md"
acceptance_index="$repo_root/specs/acceptance/index.md"
journeys_doc="$repo_root/specs/acceptance/journeys.md"
parity_report="$repo_root/docs/reports/2026-04-18-parity-gap-report.md"
backup_script="$repo_root/scripts/backup_local_metadata_db.sh"
restore_script="$repo_root/scripts/restore_local_metadata_db.sh"

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
require_file "$acceptance_doc" 'operator maintenance acceptance doc'
require_file "$acceptance_index" 'acceptance index'
require_file "$journeys_doc" 'acceptance journeys doc'
require_file "$parity_report" 'parity gap report'
require_file "$backup_script" 'metadata backup helper'
require_file "$restore_script" 'metadata restore helper'

require_line "$makefile" '^\.PHONY: .*metadata-backup.*metadata-restore' 'Makefile metadata-backup/metadata-restore .PHONY entry'
require_line "$makefile" 'make metadata-backup[[:space:]]+- create a timestamped backup of the current local metadata database' 'Makefile help text for metadata-backup'
require_line "$makefile" 'make metadata-restore BACKUP_DIR=/path/to/backup[[:space:]]+- restore the local metadata database from a captured backup directory' 'Makefile help text for metadata-restore'
require_line "$makefile" '^metadata-backup:$' 'Makefile metadata-backup target'
require_line "$makefile" 'DATABASE_URL:\?DATABASE_URL must be set' 'Makefile DATABASE_URL guard'
require_line "$makefile" '^[[:space:]]+bash scripts/backup_local_metadata_db\.sh backups/metadata$' 'Makefile metadata-backup command'
require_line "$makefile" '^metadata-restore:$' 'Makefile metadata-restore target'
require_line "$makefile" 'BACKUP_DIR:\?BACKUP_DIR must be set' 'Makefile BACKUP_DIR guard for metadata restore'
require_line "$makefile" '^[[:space:]]+bash scripts/restore_local_metadata_db\.sh "\$\$BACKUP_DIR"$' 'Makefile metadata-restore command'

require_line "$readme" '^## Local operator maintenance baseline$' 'README operator maintenance heading'
require_line "$readme" '^3\. Start or confirm the local metadata dependency before metadata backup or schema maintenance:$' 'README metadata prerequisite step'
require_line "$readme" '^   make dev-up$' 'README make dev-up maintenance command'
require_line "$readme" '^4\. Capture a backup of the current local metadata database before schema maintenance:$' 'README metadata backup step'
require_line "$readme" '^   make metadata-backup$' 'README metadata-backup command'
require_line "$readme" '^5\. Record the metadata backup directory emitted by the helper; it contains a SQL dump and manifest for the current local `DATABASE_URL` target without storing plaintext credentials\.$' 'README metadata manifest note'
require_line "$readme" '^7\. Treat upgrades as a repo update plus migration plus local process restart sequence:$' 'README upgrade step after metadata backup'
require_line "$readme" '^8\. If maintenance fails, restore the file-backed runtime baseline from the captured runtime backup directory\. The restore helper validates the backup manifest against the currently resolved runtime paths before copying files so an operator does not accidentally replay a backup captured for a different `SOURCEBOT_DATA_DIR` or explicit state-file override set:$' 'README runtime restore step'
require_line "$readme" '^11\. This maintenance baseline now covers the current file-backed runtime state plus the local Postgres metadata dump/restore workflow; notably, bootstrap-admin, local sessions, local accounts, memberships, invite acceptance, API keys, OAuth clients, review-agent-run lifecycle rows, and review-webhook delivery-attempt rows are durable in PostgreSQL when `DATABASE_URL` is configured, but broader organization aggregates, catalog state beyond bounded reads, and the remaining runtime parity work still remain follow-up slices\.$' 'README truthful widened scope note'

require_line "$acceptance_doc" 'local Postgres metadata workflow' 'operator maintenance acceptance metadata workflow scope'
require_line "$acceptance_doc" 'metadata-backup' 'operator maintenance acceptance metadata backup command'
require_line "$acceptance_doc" 'metadata-restore' 'operator maintenance acceptance metadata restore command'
require_line "$acceptance_doc" 'local-only database backup/restore baseline' 'operator maintenance acceptance truthful local-only note'
require_line "$acceptance_doc" 'still-undurable metadata surfaces' 'operator maintenance acceptance still-undurable note'

require_line "$acceptance_index" 'Operator maintenance parity' 'acceptance index operator maintenance row'
require_line "$acceptance_index" 'local Postgres metadata backup/restore' 'acceptance index metadata backup wording'
require_line "$journeys_doc" 'metadata-backup' 'journeys metadata-backup reference'
require_line "$journeys_doc" 'metadata-restore' 'journeys metadata-restore reference'

require_line "$parity_report" 'metadata-backup' 'parity report metadata-backup evidence'
require_line "$parity_report" 'metadata-restore' 'parity report metadata-restore evidence'
require_line "$parity_report" 'local Postgres metadata dump/restore baseline' 'parity report truthful local metadata wording'
require_line "$parity_report" 'does not yet mean every runtime surface is durable' 'parity report truthful durable-surface limitation'

printf 'local metadata backup contract OK\n'
