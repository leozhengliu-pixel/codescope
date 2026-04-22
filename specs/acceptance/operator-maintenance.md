# Operator Maintenance Acceptance

## Purpose
This acceptance spec defines the current local operator-maintenance baseline for the rewrite. It covers only backup and restore of the current file-backed runtime state plus the local SQLx migration workflow and local upgrade sequence already documented in the repo.

## Scope
This document covers:
- backing up the current file-backed runtime state with `scripts/backup_local_runtime_state.sh` and `make runtime-backup`
- restoring the current file-backed runtime state with `scripts/restore_local_runtime_state.sh` and `make runtime-restore BACKUP_DIR=/path/to/backup`
- resolving runtime paths from `SOURCEBOT_DATA_DIR`
- honoring explicit `SOURCEBOT_BOOTSTRAP_STATE_PATH`, `SOURCEBOT_LOCAL_SESSION_STATE_PATH`, and `SOURCEBOT_ORGANIZATION_STATE_PATH` overrides
- running the current local SQLx migration workflow with `make dev-up` and `make sqlx-migrate`
- treating upgrades as a local repo update, migration, and API/worker restart sequence

This document does **not** claim:
- durable metadata backup/restore parity
- a backup of the seeded in-memory catalog fallback or other still-undurable metadata surfaces
- production-grade readiness checks or deployment automation
- supervised worker rollout, rollback orchestration, or zero-downtime upgrade behavior

## Backup contract
Given the operator is using the current local runtime baseline,
when they run the backup helper,
then the helper creates a timestamped backup directory under the requested backup root and stores:
- `bootstrap-state.json`
- `local-sessions.json`
- `organizations.json`
- a manifest that records the backup timestamp and the resolved runtime file paths used for that capture

If local session or organization state files are still absent, the helper may materialize empty JSON baseline files because those stores already treat missing files as an empty file-backed baseline.

If the bootstrap state file is absent, the helper fails clearly instead of inventing bootstrap metadata that does not exist.

## Restore contract
Given the operator has a captured backup directory from the helper,
when they run the restore helper against that directory,
then the helper refuses missing or incomplete backup directories and otherwise copies the captured runtime files back to the currently resolved runtime paths, creating parent directories as needed.

## Migration and upgrade contract
Given the operator wants to perform local maintenance,
when they follow the documented runbook,
then they:
1. capture a runtime backup first
2. start local Postgres with `make dev-up`
3. run the current local SQLx migration workflow with `make sqlx-migrate`
4. treat an upgrade as a repo update plus migration plus `make api` and `make worker` restarts
5. use `make runtime-restore BACKUP_DIR=/path/to/backup` if they need to restore the current file-backed runtime baseline

## Deferred follow-up areas
The following parity-facing operator concerns remain outside the shipped maintenance baseline:
- durable metadata migration parity across every runtime surface
- production-safe metadata backup and restore
- readiness checks that validate migration state or dependencies
- production deployment, rollback, and observability parity
