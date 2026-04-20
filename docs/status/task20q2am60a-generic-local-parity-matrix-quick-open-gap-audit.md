# task20q2am60a — Generic/local parity-matrix quick-open gap audit

## Scope
Audit the next smallest generic/local parity evidence/report drift after `task20q2am59b` tightened the canonical gap-report row.

## Grounded evidence
- `web/src/App.tsx:1185-1200` exposes a generic-Git quick-open URL only for safe `http:` / `https:` base URLs.
- `web/src/App.tsx:1804-1822` renders the generic-Git discovery-status block plus an `Open host for manual discovery` link when that safe quick-open URL exists.
- `web/src/App.test.tsx:8004-8104` covers the generic-Git quick-open affordance on `#/settings/connections` and proves unsafe `javascript:` base URLs fail closed by rendering the discovery-status text without a clickable link.
- `specs/acceptance/generic-local-git.md:34-36` already records that truthful generic-host quick-open affordance and its fail-closed unsafe-URL rejection as part of the current generic/local baseline.
- `specs/FEATURE_PARITY.md:32` still lists the generic/local baseline as connection kinds/configs, authenticated CRUD, one local import path, repo-detail metadata, limited settings shell, quick repo-detail navigation after successful local import, and read-only sync-history visibility.
- `docs/reports/2026-04-18-parity-gap-report.md:27` still summarizes the domain at a broader level as settings-shell CRUD plus read-only sync/history baseline.

## Finding
The next smallest truthful generic/local evidence drift is now in `specs/FEATURE_PARITY.md`, not in the canonical gap-report row.

The acceptance spec already grounds one shipped generic-host settings affordance that the parity matrix omits: generic-Git connections expose a truthful quick-open host link for manual discovery while still stating that repository discovery is not available yet, and unsafe `javascript:` host values fail closed without rendering a clickable link.

The canonical gap-report row is still acceptable as a broad domain summary because it intentionally compresses several generic/local subfeatures into one row. By contrast, the parity matrix is the repo’s more feature-specific evidence ledger, so omitting the already-landed quick-open/fail-closed baseline there is the tightest remaining evidence drift.

## Smallest follow-up slice
`task20q2am60b` should tighten only the `specs/FEATURE_PARITY.md` generic/local row so it mentions the already-landed generic-host quick-open affordance and the fail-closed unsafe-URL behavior, without expanding the broader gap-report row or changing product behavior.

## Out of scope
- Changing backend or frontend runtime behavior.
- Rewriting `docs/reports/2026-04-18-parity-gap-report.md` just to enumerate another already-landed subfeature.
- Claiming generic-host discovery, recursive local enumeration, durable catalog parity, or richer per-connection operator controls are done.
