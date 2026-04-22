# Task 84 End-to-End Smoke Matrix Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Add one reproducible local smoke-matrix command that exercises the current auth, integrations, search, ask, and review-agent baseline in a single bounded run.

**Architecture:** Add a repo-local shell smoke harness that creates an isolated runtime fixture, starts the real API binary with the stub LLM provider, drives authenticated/public HTTP flows for bootstrap/login/search/ask/review-webhook intake, runs the one-shot worker to process the queued review-agent run, and then verifies the persisted/API-visible terminal state. Back it with a contract test script that fails before the harness exists and with focused acceptance/reporting updates that describe this as a local smoke baseline, not full production parity.

**Tech Stack:** Bash, python3 stdlib, curl, cargo, sourcebot-api, sourcebot-worker, existing file-backed runtime state.

---

### Task 1: Add the failing smoke-matrix contract test

**Objective:** Prove the new operator command does not exist yet and define the expected success markers.

**Files:**
- Create: `scripts/check_end_to_end_smoke_matrix_contract.sh`
- Test: `scripts/check_end_to_end_smoke_matrix_contract.sh`

**Step 1: Write failing test**

Create a contract test script that:
- runs `bash scripts/check_end_to_end_smoke_matrix.sh /opt/data/projects/sourcebot-rewrite`
- expects exit code 0
- asserts stdout/stderr contains markers for `auth`, `search`, `ask`, `review-agent`, and overall success wording
- fails cleanly if the smoke script is missing or incomplete

**Step 2: Run test to verify failure**

Run: `bash scripts/check_end_to_end_smoke_matrix_contract.sh /opt/data/projects/sourcebot-rewrite`
Expected: FAIL because `scripts/check_end_to_end_smoke_matrix.sh` does not exist yet.

**Step 3: Commit**

Do not commit yet; continue into implementation after observing RED.

### Task 2: Implement the smoke-matrix harness

**Objective:** Ship one bounded local smoke command that exercises the critical current journeys end-to-end.

**Files:**
- Create: `scripts/check_end_to_end_smoke_matrix.sh`
- Modify: `README.md`
- Test: `scripts/check_end_to_end_smoke_matrix_contract.sh`

**Step 1: Write minimal implementation**

Implement `scripts/check_end_to_end_smoke_matrix.sh` to:
- accept repo root as its first argument
- create a temp runtime directory and temp log files
- write a minimal `organizations.json` fixture containing:
  - `org_acme`
  - bootstrap-admin membership for `local_user_bootstrap_admin`
  - a visible repo binding for `repo_sourcebot_rewrite`
  - `conn_github`
- start the real API binary with isolated env:
  - `SOURCEBOT_BIND_ADDR=127.0.0.1:<ephemeral fixed test port>`
  - `SOURCEBOT_DATA_DIR=<tempdir>`
  - `SOURCEBOT_LLM_PROVIDER=stub-citations`
  - `SOURCEBOT_LLM_MODEL=task84-smoke`
- poll `/healthz`
- bootstrap and log in the admin via `/api/v1/auth/bootstrap` and `/api/v1/auth/login`
- verify `/api/v1/auth/me`
- verify integrations/authz surface via `/api/v1/auth/connections`
- verify search via `/api/v1/search?q=healthz&repo_id=repo_sourcebot_rewrite`
- verify ask via `/api/v1/ask/completions`, `/api/v1/ask/threads`, and `/api/v1/ask/threads/{thread_id}`
- create a review webhook, intake an event, verify queued run visibility, run the real worker binary with `SOURCEBOT_STUB_REVIEW_AGENT_RUN_EXECUTION_OUTCOME=completed`, then verify the run reaches `completed`
- print clear success markers
- clean up background processes and temp files on exit

**Step 2: Run focused test to verify pass**

Run: `bash scripts/check_end_to_end_smoke_matrix_contract.sh /opt/data/projects/sourcebot-rewrite`
Expected: PASS.

**Step 3: Run the smoke harness directly**

Run: `bash scripts/check_end_to_end_smoke_matrix.sh /opt/data/projects/sourcebot-rewrite`
Expected: PASS with per-surface markers and final success output.

### Task 3: Ground the new local smoke baseline truthfully

**Objective:** Update operator docs and parity wording to reflect the shipped smoke command without overclaiming full release parity.

**Files:**
- Modify: `README.md`
- Modify: `specs/acceptance/index.md`
- Modify: `specs/acceptance/journeys.md`
- Modify: `docs/reports/2026-04-18-parity-gap-report.md`

**Step 1: Patch docs minimally**

Document that:
- the repo now has a local end-to-end smoke matrix command
- the command covers the current auth, integrations, search, ask, and review-agent baseline in one bounded local run
- it uses stubbed local/runtime fixtures and is not yet a production/release certification matrix

**Step 2: Verify truthfulness**

Run exact raw-content checks for the new wording and make sure adjacent docs do not still claim Task 84 has no smoke command.

**Step 3: Broader validation**

Run:
- `bash scripts/check_end_to_end_smoke_matrix_contract.sh /opt/data/projects/sourcebot-rewrite`
- `bash scripts/check_end_to_end_smoke_matrix.sh /opt/data/projects/sourcebot-rewrite`
- `cargo test -p sourcebot-api review_webhook -- --nocapture`
- `cargo test -p sourcebot-api ask_ -- --nocapture`
- `cargo test -p sourcebot-worker --test explicit_completed_stub_outcome_smoke -- --nocapture`
- `git diff --check`

**Step 4: Commit**

```bash
git add scripts/check_end_to_end_smoke_matrix.sh \
        scripts/check_end_to_end_smoke_matrix_contract.sh \
        README.md \
        specs/acceptance/index.md \
        specs/acceptance/journeys.md \
        docs/reports/2026-04-18-parity-gap-report.md \
        docs/plans/2026-04-22-task84-end-to-end-smoke-matrix-plan.md
git commit -m "feat: add local end-to-end smoke matrix for task84"
```
