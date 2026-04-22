# Task 80a Operator Runtime Baseline Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Ship one bounded deployment/config parity closure that gives operators a shared runtime data-directory contract for the API and worker, plus truthful local runbook/acceptance grounding for the current runtime baseline.

**Architecture:** Extend `AppConfig::from_env()` with a single `SOURCEBOT_DATA_DIR` base directory that derives the existing bootstrap/session/organization state files unless more specific path overrides are present. Keep the current file-backed runtime model, add a `make worker` entrypoint alongside `make api`, then document the now-shipped operator baseline in README + acceptance/report docs instead of splitting those runtime/docs updates into later paperwork slices.

**Tech Stack:** Rust config crate, worker smoke tests, Makefile, Markdown acceptance/report docs

---

### Task 1: Add failing config + worker regressions for shared runtime data-dir wiring

**Objective:** Prove the missing operator baseline before implementation.

**Files:**
- Modify: `crates/config/src/lib.rs`
- Modify: `crates/worker/tests/no_queued_review_agent_run_idle_smoke.rs`

**Step 1: Write failing tests**
- Add config tests asserting `SOURCEBOT_DATA_DIR=/tmp/sourcebot-runtime` derives:
  - `bootstrap_state_path = /tmp/sourcebot-runtime/bootstrap-state.json`
  - `local_session_state_path = /tmp/sourcebot-runtime/local-sessions.json`
  - `organization_state_path = /tmp/sourcebot-runtime/organizations.json`
- Add a second config test proving explicit path env vars still override the shared base directory.
- Add a worker smoke test that sets only `SOURCEBOT_DATA_DIR=<tempdir>` and verifies the worker uses `<tempdir>/organizations.json` without needing `SOURCEBOT_ORGANIZATION_STATE_PATH`.

**Step 2: Run tests to verify RED**
Run: `cargo test -p sourcebot-config data_dir -- --nocapture`
Expected: FAIL because `SOURCEBOT_DATA_DIR` is ignored.

Run: `cargo test -p sourcebot-worker data_dir --test no_queued_review_agent_run_idle_smoke -- --nocapture`
Expected: FAIL because the worker still requires the explicit organization-state env var to hit the temp file.

### Task 2: Implement the shared data-dir runtime baseline

**Objective:** Land the smallest meaningful operator-visible config/runtime closure in one vertical slice.

**Files:**
- Modify: `crates/config/src/lib.rs`
- Modify: `crates/worker/tests/no_queued_review_agent_run_idle_smoke.rs`
- Modify: `Makefile`
- Modify: `.env.example`
- Modify: `README.md`

**Step 1: Implement config derivation**
- Teach `AppConfig::from_env()` to read `SOURCEBOT_DATA_DIR`.
- Derive the three existing file-backed state paths from that directory when specific overrides are absent.
- Preserve explicit `SOURCEBOT_BOOTSTRAP_STATE_PATH`, `SOURCEBOT_LOCAL_SESSION_STATE_PATH`, and `SOURCEBOT_ORGANIZATION_STATE_PATH` precedence.

**Step 2: Add the operator entrypoint script surface**
- Add `make worker` so operators can start the one-shot worker with the same `.env` contract used by `make api`.
- Keep `make api` behavior intact.

**Step 3: Re-run the focused tests**
Run: `cargo test -p sourcebot-config data_dir -- --nocapture`
Expected: PASS.

Run: `cargo test -p sourcebot-worker data_dir --test no_queued_review_agent_run_idle_smoke -- --nocapture`
Expected: PASS.

### Task 3: Ground the shipped runtime baseline truthfully in docs

**Objective:** Close the operator-runtime acceptance/doc gap in the same run instead of leaving a later paperwork slice.

**Files:**
- Create: `specs/acceptance/operator-runtime.md`
- Modify: `specs/acceptance/index.md`
- Modify: `specs/acceptance/journeys.md`
- Modify: `docs/reports/2026-04-18-parity-gap-report.md`
- Modify: `README.md`
- Modify: `.env.example`

**Step 1: Add the dedicated acceptance home**
- Create `specs/acceptance/operator-runtime.md` covering the currently shipped baseline only:
  - `/healthz`
  - `/api/v1/config`
  - shared `SOURCEBOT_DATA_DIR` / explicit-path override contract
  - `make api` + `make worker` local runtime bring-up baseline
- Keep migrations, durable metadata, readiness, supervision, and production-grade observability explicitly deferred.

**Step 2: Update index/journeys/report**
- Move operator runtime from “missing acceptance spec” to the new dedicated acceptance home.
- Update the ops gap report to mention the new shared runtime data-dir + make entrypoint baseline without overstating production parity.

**Step 3: Verify raw-text truthfulness**
Run: `git diff -- README.md .env.example specs/acceptance/operator-runtime.md specs/acceptance/index.md specs/acceptance/journeys.md docs/reports/2026-04-18-parity-gap-report.md`
Expected: scoped wording for the shipped operator baseline only.

### Task 4: Final verification and closure

**Objective:** Prove the slice is safe to land and ready for roadmap-state closure.

**Files:**
- Verify current diff only

**Step 1: Run focused runtime tests**
Run: `cargo test -p sourcebot-config data_dir -- --nocapture`
Expected: PASS.

Run: `cargo test -p sourcebot-worker --test no_queued_review_agent_run_idle_smoke`
Expected: PASS.

**Step 2: Run broader confidence signals**
Run: `cargo test -p sourcebot-config`
Expected: PASS.

Run: `cargo test -p sourcebot-worker`
Expected: PASS.

Run: `make -n worker`
Expected: prints the worker command using the repo-local env contract.

**Step 3: Run mechanical + review gates**
Run: `git diff --check`
Expected: no output.

Run: Python added-lines security scan over the diff.
Expected: 0 findings.

**Step 4: Commit after independent review**
```bash
git add crates/config/src/lib.rs crates/worker/tests/no_queued_review_agent_run_idle_smoke.rs Makefile .env.example README.md specs/acceptance/operator-runtime.md specs/acceptance/index.md specs/acceptance/journeys.md docs/reports/2026-04-18-parity-gap-report.md docs/plans/2026-04-21-task80a-operator-runtime-baseline-plan.md
git commit -m "feat: add operator runtime baseline"
```
