# Operator Maintenance Acceptance

## Purpose
This acceptance spec defines the current local operator-maintenance baseline for the rewrite. It covers backup and restore of the current file-backed runtime state plus a local-only database backup/restore baseline for the current local Postgres metadata workflow / SQLx metadata workflow and the local upgrade sequence already documented in the repo.

## Scope
This document covers:
- backing up the current file-backed runtime state with `scripts/backup_local_runtime_state.sh` and `make runtime-backup`
- restoring the current file-backed runtime state with `scripts/restore_local_runtime_state.sh` and `make runtime-restore BACKUP_DIR=/path/to/backup`
- resolving runtime paths from `SOURCEBOT_DATA_DIR`
- honoring explicit `SOURCEBOT_BOOTSTRAP_STATE_PATH`, `SOURCEBOT_LOCAL_SESSION_STATE_PATH`, and `SOURCEBOT_ORGANIZATION_STATE_PATH` overrides
- backing up the current local Postgres metadata database with `scripts/backup_local_metadata_db.sh` and `make metadata-backup`
- restoring the current local Postgres metadata database with `scripts/restore_local_metadata_db.sh` and `make metadata-restore BACKUP_DIR=/path/to/backup`
- restricting metadata backup/restore to `DATABASE_URL` targets on `127.0.0.1` or `localhost`
- running the current local SQLx migration workflow with `make dev-up` and `make sqlx-migrate`
- running the local metadata bootstrap compatibility workflow with `make metadata-dev-bootstrap`
- treating upgrades as a local repo update, migration, and API/worker restart sequence

This document does **not** claim:
- that every product/runtime surface already persists to the metadata database
- a backup of the seeded in-memory catalog fallback or other still-undurable metadata surfaces
- production-grade readiness checks or deployment automation
- supervised worker rollout, rollback orchestration, or zero-downtime upgrade behavior

## Runtime-state backup contract
Given the operator is using the current local runtime baseline,
when they run the runtime backup helper,
then the helper creates a timestamped backup directory under the requested backup root and stores:
- `bootstrap-state.json`
- `local-sessions.json`
- `organizations.json`
- a manifest that records the backup timestamp and the resolved runtime file paths used for that capture

If local session or organization state files are still absent, the helper may materialize empty JSON baseline files because those stores already treat missing files as an empty file-backed baseline.

If the bootstrap state file is absent, the helper fails clearly instead of inventing bootstrap metadata that does not exist.

## Runtime-state restore contract
Given the operator has a captured runtime backup directory from the helper,
when they run the runtime restore helper against that directory,
then the helper refuses missing or incomplete backup directories and otherwise copies the captured runtime files back to the currently resolved runtime paths, creating parent directories as needed.

## Metadata backup contract
Given the operator is using the current local metadata baseline,
when they run `make metadata-backup`,
then the helper requires `DATABASE_URL`, refuses non-local hosts, creates a timestamped backup directory, writes a `dump.sql` created with `pg_dump`, and records a manifest with a redacted database URL plus the local-only backup marker.

## Metadata restore contract
Given the operator has a captured metadata backup directory from the helper,
when they run `make metadata-restore BACKUP_DIR=/path/to/backup`,
then the helper requires `DATABASE_URL`, refuses non-local hosts, validates that `dump.sql` and `manifest.txt` exist, requires the local-only backup marker plus a matching redacted target URL in the manifest, and replays the dump into the current local metadata target with `psql -v ON_ERROR_STOP=1 -f dump.sql`.

## Migration and upgrade contract
Given the operator wants to perform local maintenance,
when they follow the documented runbook,
then they:
1. capture a runtime backup first
2. start local Postgres with `make dev-up` before metadata backup or schema maintenance
3. capture a metadata backup before schema maintenance
4. run the current local SQLx migration workflow with `make sqlx-migrate`
5. optionally run `make metadata-dev-bootstrap` for the bounded local-only workflow that waits for local Postgres, ensures the dedicated `_test` database exists, runs `make sqlx-migrate`, and then runs the focused `make sqlx-test` compatibility check
6. treat an upgrade as a repo update plus migration plus `make api` and `make worker` restarts
7. use `make runtime-restore BACKUP_DIR=/path/to/backup` if they need to restore the current file-backed runtime baseline
8. use `make metadata-restore BACKUP_DIR=/path/to/backup` if they need to restore the current local metadata dump

The `make metadata-dev-bootstrap` helper is intentionally local-only orchestration for the current metadata schema contract; it does **not** mean the API already uses durable metadata by default, because `DATABASE_URL` still routes through an unfinished lazy `PgCatalogStore` path rather than a shipped durable runtime baseline.

## Deferred follow-up areas
The following parity-facing operator concerns remain outside the shipped maintenance baseline:
- durable metadata migration parity across every runtime surface
- production-safe remote or managed metadata backup and restore
- readiness checks that validate migration state or dependencies
- production deployment, rollback, and observability parity
