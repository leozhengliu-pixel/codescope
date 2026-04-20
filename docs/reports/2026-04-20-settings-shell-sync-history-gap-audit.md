# Settings Shell Sync-History Gap Audit — 2026-04-20

## Scope

This audit closes roadmap-state task `task20q2am` by inspecting the remaining generic/local `#/settings/connections` sync-history parity surface after the reverse-order reused-repository-id latest-sync-summary family completed in `task20q2al`.

The goal of this audit is not to broaden product scope. It only identifies the next smallest focused coverage slice that remains unblocked within the existing settings-shell sync-history behavior.

## Grounded evidence inspected

- `web/src/App.tsx`
  - `compareRepositorySyncJobs(...)` sorts per-connection jobs by `queued_at`, then by activity timestamp (`finished_at ?? started_at ?? queued_at`).
  - The latest-sync summary is derived from the first sorted job for each connection.
- `web/src/App.test.tsx`
  - Existing latest-sync-summary coverage already locks same-card and sibling-card terminal-state families, including reverse API order, tied newest `queued_at`, and reused repository ids.
  - Existing in-progress coverage locks row ordering, row timestamp details, and badge rendering, but does not yet isolate the analogous latest-sync-summary tie/reverse-order family for queued-vs-running rows.
- `specs/acceptance/generic-local-git.md`
  - The current acceptance evidence records broad settings-shell sync-history behavior, but does not yet call out a focused latest-sync-summary proof for queued-vs-running rows that share the same newest `queued_at` and arrive in reverse order.

## Findings

### Already covered

The current test suite already covers these latest-sync-summary families:

1. Same-card mixed terminal-state summaries (`failed` vs `succeeded`).
2. Sibling-card mixed terminal-state summaries.
3. Tied newest `queued_at` terminal-state summaries.
4. Reverse-order terminal-state summaries.
5. Reused-repository-id terminal-state summaries.

### Next smallest uncovered gap

The next smallest unblocked parity gap is:

- **same authenticated connection card**
- **queued vs running in-progress rows**
- **same newest `queued_at` timestamp**
- **API returns those tied rows in reverse order**
- **latest-sync summary must stay truthful to the row that currently wins the same-`queued_at` tie-break contract (`finished_at ?? started_at ?? queued_at`)**

This is the closest uncovered analogue to the just-finished reverse-order terminal-state family, while staying entirely inside the existing settings-shell sync-history surface.

## Chosen next execution slice

Promote the next task to:

- `task20q2am1`
- **Title:** Add focused latest-sync summary coverage for the same authenticated connection card when queued and running rows share the same newest `queued_at` timestamp and arrive in reverse API order.

## Why this is the right next slice

- It is smaller than introducing sibling-card scoping at the same time.
- It does not require backend API, worker, retry, or broader settings-navigation changes.
- It keeps the roadmap moving through a single focused coverage contract instead of jumping to a larger sync-history audit or implementation bundle.
- If the sort/summary contract regresses later, this slice will catch it at the UI boundary where parity matters.

## Deferred follow-ups after `task20q2am1`

Not part of this run:

1. Sibling-card queued-vs-running latest-sync-summary tie cases.
2. Any broader deterministic policy when both `queued_at` and activity timestamps are identical.
3. Backend sync-job semantics, retries, or operator controls.
