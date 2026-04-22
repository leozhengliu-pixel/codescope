# Task 81a Operator Maintenance Baseline Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Ship one meaningful Phase 11 operator-maintenance closure that adds a real local backup/restore capability for the current file-backed runtime state and grounds the current migration/upgrade workflow truthfully.

**Architecture:** Keep the current operator baseline narrow and honest. Add small repo-owned shell helpers that back up and restore the current file-backed runtime state derived from `SOURCEBOT_DATA_DIR` and per-file overrides, then document how operators use those helpers before running the existing local migration/upgrade commands. Do not overclaim durable metadata backup parity while the runtime still falls back to file-backed auth/org state plus a seeded catalog.

**Tech Stack:** POSIX shell, Makefile, README/acceptance markdown, focused shell-script contract checks.

---

### Task 1: Add a failing maintenance-contract check for backup/restore and runbook wording

**Objective:** Define the exact operator-maintenance contract before implementation.

**Files:**
- Create: `scripts/check_operator_maintenance_contract.sh`
- Modify: `README.md`
- Modify: `Makefile`
- Create: `scripts/backup_local_runtime_state.sh`
- Create: `scripts/restore_local_runtime_state.sh`
- Create: `specs/acceptance/operator-maintenance.md`
- Modify: `specs/acceptance/index.md`
- Modify: `specs/acceptance/journeys.md`
- Modify: `docs/reports/2026-04-18-parity-gap-report.md`

**Step 1: Write failing test**

Create `scripts/check_operator_maintenance_contract.sh` that asserts all of the following must exist:
- `Makefile` help plus `.PHONY` entries for `runtime-backup` and `runtime-restore`
- `runtime-backup` runs `scripts/backup_local_runtime_state.sh`
- `runtime-restore` requires `BACKUP_DIR` and runs `scripts/restore_local_runtime_state.sh`
- `README.md` has a dedicated operator-maintenance section with backup, restore, migration, and upgrade steps
- `specs/acceptance/operator-maintenance.md` exists and truthfully limits scope to the current local runtime baseline
- the acceptance index and journeys docs point at the new acceptance home
- the parity report no longer says backup/restore docs are entirely missing

**Step 2: Run test to verify failure**

Run: `bash scripts/check_operator_maintenance_contract.sh /opt/data/projects/sourcebot-rewrite`
Expected: FAIL because the new scripts/docs/Make targets do not exist yet.

**Step 3: Write minimal implementation**

Create the shell check with clear `require_line` assertions and repo-root support.

**Step 4: Run test to verify pass later**

Run: `bash scripts/check_operator_maintenance_contract.sh /opt/data/projects/sourcebot-rewrite`
Expected after later steps: PASS.

**Step 5: Commit**

```bash
git add scripts/check_operator_maintenance_contract.sh
git commit -m "test: define operator maintenance contract"
```

### Task 2: Implement local runtime backup/restore helpers

**Objective:** Add a real operator-visible backup/restore capability for the current file-backed runtime state.

**Files:**
- Create: `scripts/backup_local_runtime_state.sh`
- Create: `scripts/restore_local_runtime_state.sh`
- Modify: `Makefile`

**Step 1: Write failing test**

Create a focused shell smoke in the contract script (or a second focused shell script if needed) that expects:
- `scripts/backup_local_runtime_state.sh` to create a backup directory containing the resolved `bootstrap-state.json`, `local-sessions.json`, and `organizations.json` copies plus a manifest
- `scripts/restore_local_runtime_state.sh` to copy those files back to the resolved runtime paths
- both scripts to respect `SOURCEBOT_DATA_DIR` and the explicit per-file override environment variables

**Step 2: Run test to verify failure**

Run a shell smoke such as:

```bash
TMPDIR=$(mktemp -d)
SOURCEBOT_DATA_DIR="$TMPDIR/runtime" scripts/backup_local_runtime_state.sh "$TMPDIR/backups"
```

Expected: FAIL because the script does not exist yet.

**Step 3: Write minimal implementation**

Implement backup and restore helpers that:
- resolve the runtime file paths from env
- create parent directories as needed
- copy missing source files as empty JSON baseline files only when that matches the current file-backed contract, otherwise fail clearly
- emit a small manifest that records backup timestamp and resolved paths
- refuse restore when `BACKUP_DIR` is missing or the expected files are absent

**Step 4: Run test to verify pass**

Run the focused shell smoke and verify the restored files match the pre-backup contents.

**Step 5: Commit**

```bash
git add Makefile scripts/backup_local_runtime_state.sh scripts/restore_local_runtime_state.sh
git commit -m "feat: add local runtime backup and restore helpers"
```

### Task 3: Ground the operator-maintenance runbook and acceptance docs

**Objective:** Close the contract/docs/report drift in one run once the helper scripts exist.

**Files:**
- Modify: `README.md`
- Create: `specs/acceptance/operator-maintenance.md`
- Modify: `specs/acceptance/index.md`
- Modify: `specs/acceptance/journeys.md`
- Modify: `docs/reports/2026-04-18-parity-gap-report.md`

**Step 1: Write failing test**

Use `bash scripts/check_operator_maintenance_contract.sh /opt/data/projects/sourcebot-rewrite` as the failing contract test if the required docs/wording are not yet present.

**Step 2: Run test to verify failure**

Expected: FAIL until the docs contain the exact backup/restore/migration/upgrade wording.

**Step 3: Write minimal implementation**

Document only the current truthful baseline:
- back up the file-backed runtime state before maintenance
- restore the file-backed runtime state from a captured backup directory
- run `make dev-up` and `make sqlx-migrate` for the current local metadata migration workflow
- treat upgrade as a local repo update + migration + API/worker restart sequence
- explicitly defer durable metadata backup/restore, readiness, and production-grade deployment parity

**Step 4: Run test to verify pass**

Run: `bash scripts/check_operator_maintenance_contract.sh /opt/data/projects/sourcebot-rewrite`
Expected: PASS.

**Step 5: Commit**

```bash
git add README.md specs/acceptance/operator-maintenance.md specs/acceptance/index.md specs/acceptance/journeys.md docs/reports/2026-04-18-parity-gap-report.md
git commit -m "docs: add operator maintenance baseline runbook"
```

### Task 4: Verify the full closure

**Objective:** Prove the new operator-maintenance slice works end-to-end and is truthfully documented.

**Files:**
- Verify only; no new files required unless fixes are needed.

**Step 1: Run targeted shell contract checks**

```bash
bash scripts/check_operator_maintenance_contract.sh /opt/data/projects/sourcebot-rewrite
```

Expected: PASS.

**Step 2: Run focused backup/restore smoke**

```bash
python3 - <<'PY'
import json, os, pathlib, subprocess, tempfile
repo = pathlib.Path('/opt/data/projects/sourcebot-rewrite')
with tempfile.TemporaryDirectory() as tmp:
    tmp = pathlib.Path(tmp)
    runtime = tmp / 'runtime'
    runtime.mkdir()
    (runtime / 'bootstrap-state.json').write_text('{"is_initialized":true}\n')
    (runtime / 'local-sessions.json').write_text('{"sessions":[]}\n')
    (runtime / 'organizations.json').write_text('{"organizations":[]}\n')
    env = os.environ.copy()
    env['SOURCEBOT_DATA_DIR'] = str(runtime)
    subprocess.run(['bash', str(repo / 'scripts/backup_local_runtime_state.sh'), str(tmp / 'backups')], check=True, env=env, cwd=repo)
    (runtime / 'organizations.json').write_text('{"organizations":[{"id":"changed"}]}\n')
    backup_dirs = sorted((tmp / 'backups').iterdir())
    subprocess.run(['bash', str(repo / 'scripts/restore_local_runtime_state.sh'), str(backup_dirs[-1])], check=True, env=env, cwd=repo)
    assert json.loads((runtime / 'organizations.json').read_text()) == {"organizations": []}
print('operator maintenance smoke OK')
PY
```

Expected: PASS.

**Step 3: Run broader confidence signal**

Run: `bash scripts/check_local_metadata_env_contract.sh /opt/data/projects/sourcebot-rewrite`
Expected: PASS.

**Step 4: Independent review**

Use requesting-code-review skill: diff, added-lines security scan, and one independent reviewer subagent.

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: add operator maintenance baseline"
```
