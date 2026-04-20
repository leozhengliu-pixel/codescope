# task20q2am59a audit — next generic/local settings-shell sync-history evidence gap

## Scope
Audit the next smallest evidence/report gap after `task20q2am58b` without changing application behavior, backend APIs, retry semantics, or broader admin/settings navigation.

## Grounded findings
1. `specs/acceptance/generic-local-git.md` is now the detailed acceptance source for the limited `#/settings/connections` shell, the local explicit-path import baseline, and the focused sync-history/latest-sync regressions already landed in `web/src/App.test.tsx`.
2. `specs/FEATURE_PARITY.md` is still high level but truthful: it already says the rewrite has local `repo_path` handling, local-only explicit-path import UX, quick repo-detail navigation, and read-only sync-history visibility.
3. `docs/reports/2026-04-18-parity-gap-report.md` still underreports the live state in the generic/local row by listing `Add settings-driven import UX` as a remaining highest-value gap even though `web/src/App.tsx` and `specs/acceptance/generic-local-git.md` already ground that limited settings-driven import UX.

## Smallest truthful next slice
Queue `task20q2am59b`: tighten the generic/local row in `docs/reports/2026-04-18-parity-gap-report.md` so it stops claiming the already-landed settings-driven import UX is missing, while keeping the real remaining gaps limited to generic-host/GitLab discovery, durable catalog-backed parity, and richer sync/index runtime behavior.

## Why this is next
- It is a docs-only closure.
- It corrects a stale report-level claim without expanding product scope.
- It keeps the canonical acceptance doc as the detailed source of truth while aligning the parity gap report with already-grounded evidence.
