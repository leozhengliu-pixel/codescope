# Task 80b Worker Runtime Baseline Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Ship one bounded Phase 11 closure that turns the current one-shot worker into a truthfully documented operator/runtime baseline with explicit startup diagnostics and a dedicated acceptance home.

**Architecture:** Keep the worker intentionally one-shot and stub-oriented, but make the execution baseline operator-visible in one run: log the resolved runtime contract at startup, keep terminal/no-work outcome logs explicit, add focused binary smoke coverage for those logs, and create `specs/acceptance/worker-runtime.md` so the acceptance corpus no longer treats worker runtime as an unnamed gap.

**Tech Stack:** Rust worker binary, cargo integration tests, Markdown acceptance/report docs

---

### Task 1: Add failing worker-runtime logging smoke coverage

**Objective:** Prove the missing operator-visible execution baseline before changing the worker.

**Files:**
- Create: `crates/worker/tests/worker_runtime_logging_smoke.rs`
- Reference: `crates/worker/src/main.rs`

**Step 1: Write failing tests**

Add binary smoke tests that run `sourcebot-worker` and assert stderr/log output includes:
- a startup line that names the resolved `organization_state_path`
- the selected `SOURCEBOT_STUB_REVIEW_AGENT_RUN_EXECUTION_OUTCOME`
- the selected `SOURCEBOT_STUB_REPOSITORY_SYNC_JOB_EXECUTION_OUTCOME`
- explicit one-shot runtime wording
- a no-work log when there is no queued work
- a repository-sync terminal-status log when a queued repository sync job is processed with `failed`

Use real file-backed state fixtures so the tests exercise the shipped binary, not an in-process helper.

**Step 2: Run test to verify RED**

Run: `cargo test -p sourcebot-worker --test worker_runtime_logging_smoke -- --nocapture`
Expected: FAIL because the worker does not yet log the startup/runtime contract details required by the new execution baseline.

### Task 2: Implement the worker-runtime execution baseline

**Objective:** Add the smallest meaningful operator-visible capability needed to make the acceptance/doc closure truthful.

**Files:**
- Modify: `crates/worker/src/main.rs`
- Create: `crates/worker/tests/worker_runtime_logging_smoke.rs`

**Step 1: Write minimal implementation**

Update `crates/worker/src/main.rs` so the binary logs one explicit startup/runtime-baseline line before executing the tick. That log must include:
- the resolved `organization_state_path`
- review-agent stub outcome
- repository-sync stub outcome
- explicit one-shot execution wording

Keep the existing one-shot behavior and terminal/no-work logs truthful; do not add retries, loops, or background supervision.

**Step 2: Re-run the focused test**

Run: `cargo test -p sourcebot-worker --test worker_runtime_logging_smoke -- --nocapture`
Expected: PASS.

### Task 3: Create the dedicated worker-runtime acceptance home and grounded docs

**Objective:** Close the acceptance/doc drift in the same run instead of leaving a later paperwork-only slice.

**Files:**
- Create: `specs/acceptance/worker-runtime.md`
- Modify: `specs/acceptance/index.md`
- Modify: `specs/acceptance/journeys.md`
- Modify: `docs/reports/2026-04-18-parity-gap-report.md`
- Modify: `README.md`
- Modify: `.env.example`

**Step 1: Add the dedicated acceptance spec**

Create `specs/acceptance/worker-runtime.md` covering only the currently shipped baseline:
- `make worker` / `sourcebot-worker` one-shot execution
- resolved organization-state path from explicit env or `SOURCEBOT_DATA_DIR`
- configured stub outcomes for review-agent and repository-sync work
- explicit no-work vs terminal-outcome logging
- one oldest queued item per invocation

Keep real execution, retries, scheduling loops, supervision, durable metadata, and production observability explicitly deferred.

**Step 2: Update surrounding docs**

- Move worker runtime in `specs/acceptance/index.md` from missing to present.
- Update `specs/acceptance/journeys.md` so worker runtime now has its own acceptance home.
- Update the parity gap report worker + ops sections to reflect the new dedicated acceptance doc and startup/runtime-baseline logging evidence.
- Update `README.md` and `.env.example` so operators can discover the current worker stub env vars without overclaiming production parity.

**Step 3: Verify raw-text truthfulness**

Run: `git diff -- README.md .env.example specs/acceptance/worker-runtime.md specs/acceptance/index.md specs/acceptance/journeys.md docs/reports/2026-04-18-parity-gap-report.md`
Expected: only the current one-shot worker baseline is documented; retries/supervision/orchestration remain explicitly deferred.

### Task 4: Final verification and closure

**Objective:** Prove the slice is safe to land and ready for roadmap-state closure.

**Files:**
- Verify current diff only

**Step 1: Run focused worker tests**

Run: `cargo test -p sourcebot-worker --test worker_runtime_logging_smoke -- --nocapture`
Expected: PASS.

Run: `cargo test -p sourcebot-worker --test no_queued_review_agent_run_idle_smoke -- --nocapture`
Expected: PASS.

Run: `cargo test -p sourcebot-worker --test repository_sync_claim_smoke -- --nocapture`
Expected: PASS.

**Step 2: Run broader confidence signal**

Run: `cargo test -p sourcebot-worker`
Expected: PASS.

**Step 3: Run mechanical + review gates**

Run: `git diff --check`
Expected: no output.

Run: Python added-lines security scan over the diff.
Expected: 0 findings.

Run: one independent review against the final diff.
Expected: no blocking security or logic issues.

**Step 4: Commit after review**

```bash
git add crates/worker/src/main.rs crates/worker/tests/worker_runtime_logging_smoke.rs README.md .env.example specs/acceptance/worker-runtime.md specs/acceptance/index.md specs/acceptance/journeys.md docs/reports/2026-04-18-parity-gap-report.md docs/plans/2026-04-22-task80b-worker-runtime-baseline-plan.md
git commit -m "feat: add worker runtime baseline"
```
