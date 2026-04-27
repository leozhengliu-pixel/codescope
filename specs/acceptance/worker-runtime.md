# Worker Runtime Acceptance

## Purpose
This document defines the current clean-room acceptance contract for the `sourcebot-worker` runtime in `crates/worker/src/main.rs`. It describes the shipped one-tick default plus bounded multi-tick, stub-oriented worker baseline without over-claiming real execution, retries, production scheduling, durable worker metadata, or production observability.

## Grounding and limits
- Grounded in:
  - `crates/worker/src/main.rs`
  - `crates/worker/src/lib.rs`
  - `crates/worker/tests/worker_runtime_logging_smoke.rs`
  - existing focused worker smoke coverage in `crates/worker/tests/*.rs`
- By default, the worker remains a one-tick local bring-up path. Setting `SOURCEBOT_WORKER_MAX_TICKS=<n>` lets one invocation run a bounded multi-tick loop and then exit; `SOURCEBOT_WORKER_IDLE_SLEEP_MS=<ms>` controls the sleep between no-work ticks.
- The repo-local smoke matrix now proves the API enqueue/history surface can hand a queued repository-sync job to a later `sourcebot-worker` invocation; authenticated local import also queues a newly imported local repository for that same worker baseline after adding the imported repository to the admin organization visibility set. The terminal `succeeded` progress remains observable through the authenticated history API and persisted organization state.
- Repository-sync jobs tied to a configured `local` connection now run a timeout-bounded real Git preflight (`git -C <repo_path> rev-parse --is-inside-work-tree`) plus bounded revision, content-discovery, and current-branch probes (`git rev-parse HEAD`, `git ls-tree -r --name-only HEAD`, and `git symbolic-ref --short HEAD`) before falling back to stub outcomes: missing, invalid, empty, empty-tree, detached, or hung local repositories fail closed with operator-visible `local repository sync preflight failed` or `local repository sync execution failed` errors and do not create a successful manifest/snapshot, while a real local Git working tree with a readable HEAD, at least one tracked content path, and current branch can complete successfully even when the configured stub outcome is `failed`, materializes `.sourcebot/local-sync/<organization_id>/<repository_id>/<job_id>/manifest.txt` with the synced revision, branch, and tracked `HEAD` paths plus a sibling `snapshot/` tree populated from tracked `HEAD` file bytes, and persists `synced_revision`, `synced_branch`, and `synced_content_file_count` on the terminal sync job.
- Review-agent execution and non-local repository-sync execution are currently stub outcomes selected from environment configuration.
- Deferred follow-up work includes real fetch/import execution, broad automated retries/backoff, production scheduling/supervision, durable worker metadata beyond the shared state file, and production-grade observability; the bounded multi-tick loop only repeats the existing tick path under explicit operator control, repository-sync ticks recover stale `running` jobs after a bounded lease before claiming more work, and the automatic retry baseline is limited to one queued retry for stale-lease failures after an additional one-hour backoff when no queued/running or prior automatic replacement exists.

## Acceptance scenarios
1. Running the real `sourcebot-worker` binary with a resolved organization-state path must log a startup runtime-baseline line that names:
   - the resolved `organization_state_path`
   - the selected review-agent stub outcome
   - the selected repository-sync stub outcome
   - configured bounded tick count and idle sleep
2. When no queued review-agent run or repository-sync job is available, the worker must exit successfully and log the no-work path instead of claiming progress.
3. When a queued review-agent run exists, the default invocation may claim and persist only that single oldest queued run's configured stub terminal outcome before exiting.
4. When no review-agent run is available but a queued repository-sync job exists, the default invocation may claim and persist only that single oldest queued repository-sync job before exiting; non-local jobs and local jobs without a matching configured local connection still use the selected stub terminal outcome.
5. A repository-sync job tied to a configured `local` connection must run a timeout-bounded real local Git working-tree preflight plus bounded HEAD/content-discovery/current-branch probes before terminal completion; missing, invalid, empty, empty-tree, detached, or hung paths must persist `failed` with a `local repository sync preflight failed` or `local repository sync execution failed` error and must not leave a successful tracked-content manifest/snapshot, while a real local Git repository with a readable current revision, at least one tracked content path, and current branch may persist `succeeded` independently of the configured stub failure outcome, record the terminal revision, branch, and tracked-content file count, write a deterministic repo-local `.sourcebot/local-sync/<organization_id>/<repository_id>/<job_id>/manifest.txt` containing the revision, branch, and tracked paths from `git ls-tree -r --name-only HEAD`, and materialize a sibling `snapshot/` tree containing only tracked `HEAD` content.
6. The repo-local end-to-end smoke matrix must be able to enqueue a repository-sync job through `POST /api/v1/auth/repository-sync-jobs`, read it as `queued` through `GET /api/v1/auth/repository-sync-jobs`, run the real worker once after higher-priority review-agent work is already drained, and then read the same job as `succeeded` with `started_at` and `finished_at` populated.
7. When `SOURCEBOT_STUB_REPOSITORY_SYNC_JOB_EXECUTION_OUTCOME=failed`, the worker must still exit successfully, persist the failed stub outcome for non-local work, and log a repository-sync terminal-status line.
8. When `SOURCEBOT_WORKER_MAX_TICKS` is set to a positive integer greater than one, one worker invocation may repeat the same priority-ordered tick path up to that bounded count, process multiple queued jobs across ticks, and log configured bounded runtime completion before exiting; this is still explicit operator-controlled looping, not daemon supervision.
9. Before claiming another repository-sync job, each repository-sync tick must recover `running` repository-sync jobs whose `started_at` timestamp is at least one hour old by marking them `failed` with an operator-visible lease-expiry error, while preserving fresh `running` jobs and then continuing to claim the oldest queued job for that tick.
10. A failed stale-lease recovery record whose `finished_at` is at least one hour old may enqueue exactly one automatic replacement job before the tick claims work, but only when no queued/running or prior automatic replacement already exists for the same organization, repository, and connection; this is bounded stale-lease replay, not general retry/backoff parity.
11. Invalid stub outcome or bounded-loop configuration must fail closed before execution rather than silently falling back to another outcome.

## Operator-visible runtime contract
- `SOURCEBOT_DATA_DIR` may provide the shared local runtime base directory.
- `SOURCEBOT_ORGANIZATION_STATE_PATH` may explicitly override the organization-state file path.
- `SOURCEBOT_STUB_REVIEW_AGENT_RUN_EXECUTION_OUTCOME` currently supports `completed` and `failed`.
- `SOURCEBOT_STUB_REPOSITORY_SYNC_JOB_EXECUTION_OUTCOME` currently supports `succeeded` and `failed`.
- `SOURCEBOT_WORKER_MAX_TICKS` defaults to `1`; when set, it must be a positive integer and bounds the number of ticks one worker process will run before exiting.
- `SOURCEBOT_WORKER_IDLE_SLEEP_MS` defaults to `1000`; when set, it must be a non-negative integer and controls how long the bounded loop sleeps after a no-work tick before trying the next configured tick.
- Worker logs plus authenticated sync-history responses are the current runtime-baseline evidence for startup resolution, no-work exits, stub terminal outcomes, bounded multi-tick runtime completion, timeout-bounded local-Git preflight plus persisted revision/content-count/current-branch probe outcomes, bounded repo-local tracked-content manifests/snapshots, stale-running repository-sync lease recovery before the next claim, one automatic stale-lease retry after a one-hour backoff when no active or prior automatic replacement exists, and repository-sync progress after an admin enqueues a visible repository job; repository-sync terminal logs include the job id, organization id, repository id, connection id, status, and failure reason when one exists so operators can triage the bounded worker path without querying the state file first.
- The local-Git worker baseline only proves that a configured local repository path is an actual Git working tree with a readable current revision, at least one tracked content path discoverable from `HEAD`, and current branch before recording terminal sync-job status plus the terminal revision/branch/content-count metadata and writing the bounded manifest/snapshot; it does not fetch, import, reindex, mirror, or reconcile repository contents beyond that repo-local tracked `HEAD` materialization.

## Explicit deferrals
The current worker runtime acceptance does **not** claim:
- real review-agent execution
- real repository-sync fetch/import/mirror execution beyond local Git working-tree preflight plus revision/content-discovery/current-branch probes, bounded tracked-content manifest/snapshot status, stubbed terminal outcomes, or the bounded enqueue-to-history progress smoke
- automated retries/backoff beyond one bounded automatic stale-lease retry and the bounded manual retry API
- production polling, daemonization, or supervision beyond the explicit bounded multi-tick loop
- durable worker-specific metadata beyond the shared organization-state file
- queue depth metrics, dashboards, alerts, or production-ready observability
- production deployment guidance beyond the local operator/runtime baseline already documented elsewhere
