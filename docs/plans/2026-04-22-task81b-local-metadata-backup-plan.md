# Task 81b Local Metadata Backup/Restore Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Ship one meaningful Phase 11 operator-maintenance closure that adds a real local durable-metadata backup/restore capability for the current SQLx/Postgres workflow and grounds the upgraded maintenance runbook truthfully.

**Architecture:** Extend the shipped file-backed runtime maintenance baseline with narrow repo-owned shell helpers for the local metadata database only. Keep the slice honest: back up and restore the current `DATABASE_URL` Postgres database with explicit local-only safety guards, focused shell contract checks, and scoped docs that still defer full durable-store parity, readiness, and production deployment guarantees.

**Tech Stack:** POSIX shell, Makefile, README/acceptance markdown, focused shell-script contract/smoke checks.

---

### Task 1: Define the local metadata maintenance contract first

**Objective:** Write the failing contract test before implementation so the new operator-visible behavior is explicit.

**Files:**
- Create: `scripts/check_local_metadata_backup_contract.sh`
- Modify: `Makefile`
- Modify: `README.md`
- Modify: `specs/acceptance/operator-maintenance.md`
- Modify: `specs/acceptance/index.md`
- Modify: `specs/acceptance/journeys.md`
- Modify: `docs/reports/2026-04-18-parity-gap-report.md`

**Step 1: Write failing test**

Create `scripts/check_local_metadata_backup_contract.sh` that asserts all of the following must exist:
- `Makefile` help plus `.PHONY` entries for `metadata-backup` and `metadata-restore`
- `metadata-backup` requires `DATABASE_URL` and runs `scripts/backup_local_metadata_db.sh`
- `metadata-restore` requires both `DATABASE_URL` and `BACKUP_DIR` and runs `scripts/restore_local_metadata_db.sh`
- the README operator-maintenance section includes metadata backup/restore before the migration/upgrade steps
- `specs/acceptance/operator-maintenance.md` truthfully narrows the new scope to the current local Postgres metadata workflow and still defers full durable-store/runtime parity
- acceptance index, journeys, and parity report all point at the upgraded operator-maintenance baseline instead of saying durable metadata backup/restore is entirely missing

**Step 2: Run test to verify failure**

Run: `bash scripts/check_local_metadata_backup_contract.sh /opt/data/projects/sourcebot-rewrite`
Expected: FAIL because the scripts/targets/docs do not exist yet.

**Step 3: Write minimal implementation**

Implement the contract check with exact `require_line` assertions and repo-root support.

**Step 4: Run test to verify pass later**

Run: `bash scripts/check_local_metadata_backup_contract.sh /opt/data/projects/sourcebot-rewrite`
Expected after later steps: PASS.

**Step 5: Commit**

```bash
git add scripts/check_local_metadata_backup_contract.sh
git commit -m "test: define local metadata backup contract"
```

### Task 2: Implement local metadata backup/restore helpers with local-only safety

**Objective:** Add a real operator-visible durable-metadata recovery capability for the current local Postgres workflow.

**Files:**
- Create: `scripts/backup_local_metadata_db.sh`
- Create: `scripts/restore_local_metadata_db.sh`
- Modify: `Makefile`
- Test: `scripts/check_local_metadata_backup_restore_smoke.sh`

**Step 1: Write failing test**

Create `scripts/check_local_metadata_backup_restore_smoke.sh` that expects:
- `scripts/backup_local_metadata_db.sh` to refuse missing `DATABASE_URL`
- backup/restore to refuse non-local hosts so the baseline stays local-only and fail-closed
- backup to create a timestamped directory with a SQL dump plus manifest recording the sanitized/local metadata contract
- restore to require the expected dump/manifest files and replay them with `psql`
- both scripts to allow command stubbing through `PATH` so the smoke can verify the exact `pg_dump`/`psql` invocations without a real database server

**Step 2: Run test to verify failure**

Run: `bash scripts/check_local_metadata_backup_restore_smoke.sh /opt/data/projects/sourcebot-rewrite`
Expected: FAIL because the helper scripts do not exist yet.

**Step 3: Write minimal implementation**

Implement shell helpers that:
- require `DATABASE_URL`
- accept only local `localhost` / `127.0.0.1` Postgres URLs for this baseline
- create a timestamped backup directory under the requested root
- run `pg_dump --file dump.sql "$DATABASE_URL"`
- write a manifest that records timestamp and a sanitized database URL / local-host contract note without persisting credentials
- restore only when `dump.sql` and `manifest.txt` are present, then run `psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f dump.sql`

**Step 4: Run test to verify pass**

Run: `bash scripts/check_local_metadata_backup_restore_smoke.sh /opt/data/projects/sourcebot-rewrite`
Expected: PASS.

**Step 5: Commit**

```bash
git add Makefile scripts/backup_local_metadata_db.sh scripts/restore_local_metadata_db.sh scripts/check_local_metadata_backup_restore_smoke.sh
git commit -m "feat: add local metadata backup and restore helpers"
```

### Task 3: Ground the upgraded operator-maintenance baseline truthfully

**Objective:** Close the contract/docs/report drift in one run once the metadata helper scripts exist.

**Files:**
- Modify: `README.md`
- Modify: `specs/acceptance/operator-maintenance.md`
- Modify: `specs/acceptance/index.md`
- Modify: `specs/acceptance/journeys.md`
- Modify: `docs/reports/2026-04-18-parity-gap-report.md`

**Step 1: Write failing test**

Use `bash scripts/check_local_metadata_backup_contract.sh /opt/data/projects/sourcebot-rewrite` as the failing contract test until the exact docs/wording exist.

**Step 2: Run test to verify failure**

Expected: FAIL until the docs include metadata backup/restore wording and truthful scope limits.

**Step 3: Write minimal implementation**

Document only the current truthful baseline:
- capture both the file-backed runtime-state backup and the local metadata database backup before maintenance
- restore the file-backed runtime state and the local metadata dump separately when maintenance fails
- keep migration/upgrade flow scoped to the current local `make dev-up` + `make sqlx-migrate` workflow
- explicitly state that this new slice covers only local Postgres backup/restore for the current metadata schema and does not imply every product/runtime surface is durable yet
- continue to defer readiness, production deployment automation, and full durable-store parity

**Step 4: Run test to verify pass**

Run: `bash scripts/check_local_metadata_backup_contract.sh /opt/data/projects/sourcebot-rewrite`
Expected: PASS.

**Step 5: Commit**

```bash
git add README.md specs/acceptance/operator-maintenance.md specs/acceptance/index.md specs/acceptance/journeys.md docs/reports/2026-04-18-parity-gap-report.md
git commit -m "docs: extend operator maintenance baseline to local metadata backups"
```

### Task 4: Verify the full closure

**Objective:** Prove the upgraded maintenance slice works end-to-end and is truthfully documented.

**Files:**
- Verify only; no new files unless fixes are needed.

**Step 1: Run targeted shell contract checks**

```bash
bash scripts/check_local_metadata_backup_contract.sh /opt/data/projects/sourcebot-rewrite
bash scripts/check_local_metadata_backup_restore_smoke.sh /opt/data/projects/sourcebot-rewrite
```

Expected: PASS.

**Step 2: Run focused existing maintenance checks**

```bash
bash scripts/check_operator_maintenance_contract.sh /opt/data/projects/sourcebot-rewrite
bash scripts/check_local_metadata_env_contract.sh /opt/data/projects/sourcebot-rewrite
```

Expected: PASS.

**Step 3: Run broader confidence signal**

```bash
git diff --check
```

Expected: PASS.

**Step 4: Independent review**

Use requesting-code-review skill: diff, added-lines security scan, and one independent reviewer subagent.

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: add local metadata backup maintenance baseline"
```
