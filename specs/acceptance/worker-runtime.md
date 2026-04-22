# Worker Runtime Acceptance

## Purpose
This document defines the current clean-room acceptance contract for the `sourcebot-worker` runtime in `crates/worker/src/main.rs`. It describes the shipped one-shot, stub-oriented worker baseline without over-claiming real execution, retries, scheduling, durable worker metadata, or production observability.

## Grounding and limits
- Grounded in:
  - `crates/worker/src/main.rs`
  - `crates/worker/src/lib.rs`
  - `crates/worker/tests/worker_runtime_logging_smoke.rs`
  - existing focused worker smoke coverage in `crates/worker/tests/*.rs`
- The current worker is intentionally one-shot. One invocation performs at most one worker tick and then exits.
- Review-agent and repository-sync execution are currently stub outcomes selected from environment configuration.
- Deferred follow-up work includes real execution, retries, scheduling loops, supervision, durable worker metadata, queue recovery, and production-grade observability.

## Acceptance scenarios
1. Running the real `sourcebot-worker` binary with a resolved organization-state path must log a startup runtime-baseline line that names:
   - the resolved `organization_state_path`
   - the selected review-agent stub outcome
   - the selected repository-sync stub outcome
   - explicit one-shot runtime wording
2. When no queued review-agent run or repository-sync job is available, the worker must exit successfully and log the no-work path instead of claiming progress.
3. When a queued review-agent run exists, one invocation may claim and persist only that single oldest queued run's configured stub terminal outcome before exiting.
4. When no review-agent run is available but a queued repository-sync job exists, one invocation may claim and persist only that single oldest queued repository-sync job's configured stub terminal outcome before exiting.
5. When `SOURCEBOT_STUB_REPOSITORY_SYNC_JOB_EXECUTION_OUTCOME=failed`, the worker must still exit successfully, persist the failed stub outcome, and log a repository-sync terminal-status line.
6. Invalid stub outcome configuration must fail closed before execution rather than silently falling back to another outcome.

## Operator-visible runtime contract
- `SOURCEBOT_DATA_DIR` may provide the shared local runtime base directory.
- `SOURCEBOT_ORGANIZATION_STATE_PATH` may explicitly override the organization-state file path.
- `SOURCEBOT_STUB_REVIEW_AGENT_RUN_EXECUTION_OUTCOME` currently supports `completed` and `failed`.
- `SOURCEBOT_STUB_REPOSITORY_SYNC_JOB_EXECUTION_OUTCOME` currently supports `succeeded` and `failed`.
- Worker logs are the current runtime-baseline evidence for startup resolution, no-work exits, and stub terminal outcomes.

## Explicit deferrals
The current worker runtime acceptance does **not** claim:
- real review-agent execution
- real repository-sync execution beyond stubbed terminal outcomes
- retries, backoff, or replay
- continuous polling, scheduling, daemonization, or supervision
- durable worker-specific metadata beyond the shared organization-state file
- queue depth metrics, dashboards, alerts, or production-ready observability
- production deployment guidance beyond the local operator/runtime baseline already documented elsewhere
