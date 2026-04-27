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
- The repo-local smoke matrix now proves the API enqueue/history surface can hand a queued repository-sync job to a later `sourcebot-worker` invocation and observe the terminal `succeeded` progress through the authenticated history API and persisted organization state.
- Repository-sync jobs tied to a configured `local` connection now run a timeout-bounded real Git preflight (`git -C <repo_path> rev-parse --is-inside-work-tree`) plus bounded revision, content-discovery, and current-branch probes (`git rev-parse HEAD`, `git ls-tree -r --name-only HEAD`, and `git symbolic-ref --short HEAD`) before falling back to stub outcomes: missing, invalid, empty, empty-tree, detached, or hung local repositories fail closed with operator-visible `local repository sync preflight failed` or `local repository sync execution failed` errors, while a real local Git working tree with a readable HEAD, at least one tracked content path, and current branch can complete successfully even when the configured stub outcome is `failed`.
- Review-agent execution and non-local repository-sync execution are currently stub outcomes selected from environment configuration.
- Deferred follow-up work includes real fetch/import execution, retries, scheduling loops, supervision, durable worker metadata, queue recovery, and production-grade observability.

## Acceptance scenarios
1. Running the real `sourcebot-worker` binary with a resolved organization-state path must log a startup runtime-baseline line that names:
   - the resolved `organization_state_path`
   - the selected review-agent stub outcome
   - the selected repository-sync stub outcome
   - explicit one-shot runtime wording
2. When no queued review-agent run or repository-sync job is available, the worker must exit successfully and log the no-work path instead of claiming progress.
3. When a queued review-agent run exists, one invocation may claim and persist only that single oldest queued run's configured stub terminal outcome before exiting.
4. When no review-agent run is available but a queued repository-sync job exists, one invocation may claim and persist only that single oldest queued repository-sync job before exiting; non-local jobs and local jobs without a matching configured local connection still use the selected stub terminal outcome.
5. A repository-sync job tied to a configured `local` connection must run a timeout-bounded real local Git working-tree preflight plus bounded HEAD/content-discovery/current-branch probes before terminal completion; missing, invalid, empty, empty-tree, detached, or hung paths must persist `failed` with a `local repository sync preflight failed` or `local repository sync execution failed` error, while a real local Git repository with a readable current revision, at least one tracked content path, and current branch may persist `succeeded` independently of the configured stub failure outcome.
6. The repo-local end-to-end smoke matrix must be able to enqueue a repository-sync job through `POST /api/v1/auth/repository-sync-jobs`, read it as `queued` through `GET /api/v1/auth/repository-sync-jobs`, run the real worker once after higher-priority review-agent work is already drained, and then read the same job as `succeeded` with `started_at` and `finished_at` populated.
7. When `SOURCEBOT_STUB_REPOSITORY_SYNC_JOB_EXECUTION_OUTCOME=failed`, the worker must still exit successfully, persist the failed stub outcome for non-local work, and log a repository-sync terminal-status line.
8. Invalid stub outcome configuration must fail closed before execution rather than silently falling back to another outcome.

## Operator-visible runtime contract
- `SOURCEBOT_DATA_DIR` may provide the shared local runtime base directory.
- `SOURCEBOT_ORGANIZATION_STATE_PATH` may explicitly override the organization-state file path.
- `SOURCEBOT_STUB_REVIEW_AGENT_RUN_EXECUTION_OUTCOME` currently supports `completed` and `failed`.
- `SOURCEBOT_STUB_REPOSITORY_SYNC_JOB_EXECUTION_OUTCOME` currently supports `succeeded` and `failed`.
- Worker logs plus authenticated sync-history responses are the current runtime-baseline evidence for startup resolution, no-work exits, stub terminal outcomes, timeout-bounded local-Git preflight plus revision/content-discovery/current-branch probe outcomes, and repository-sync progress after an admin enqueues a visible repository job; repository-sync terminal logs include the job id, organization id, repository id, connection id, status, and failure reason when one exists so operators can triage the bounded worker path without querying the state file first.
- The local-Git worker baseline only proves that a configured local repository path is an actual Git working tree with a readable current revision, at least one tracked content path discoverable from `HEAD`, and current branch before recording terminal sync-job status; it does not fetch, import, reindex, mirror, or reconcile repository contents.

## Explicit deferrals
The current worker runtime acceptance does **not** claim:
- real review-agent execution
- real repository-sync fetch/import/mirror execution beyond local Git working-tree preflight plus revision/content-discovery/current-branch probes, stubbed terminal outcomes, or the bounded enqueue-to-history progress smoke
- retries, backoff, or replay
- continuous polling, scheduling, daemonization, or supervision
- durable worker-specific metadata beyond the shared organization-state file
- queue depth metrics, dashboards, alerts, or production-ready observability
- production deployment guidance beyond the local operator/runtime baseline already documented elsewhere
