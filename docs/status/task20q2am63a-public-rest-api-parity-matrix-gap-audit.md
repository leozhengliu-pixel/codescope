# task20q2am63a — Public REST API parity-matrix gap audit

## Scope
Audit the next smallest Public REST API documentation drift after the MCP parity-matrix follow-up closed `task20q2am62b`.

## Grounded evidence
- `specs/FEATURE_PARITY.md:35` still leaves the `Public REST API` row at `Needs audit | _TBD_ |`.
- `specs/acceptance/integrations.md:26` already keeps the acceptance contract conservative and specific: public REST APIs should be versioned and return stable machine-readable responses.
- `specs/acceptance/integrations.md:40` already uses that requirement in a black-box example: calling the public REST API returns versioned JSON and permission-scoped results.
- `docs/reports/2026-04-18-parity-gap-report.md:68` already records the current rewrite as `Partial`, grounding that `crates/api/src/main.rs` serves versioned `/api/v1/...` routes across config, repo browse/search, auth, OAuth-client, and review-webhook surfaces.
- `docs/reports/2026-04-18-parity-gap-report.md:68` also already records the real remaining gap: endpoint completeness, integration-specific acceptance evidence, and durable connection/repository operations still need to expand before the rewrite satisfies the broader connector/admin/operator API contract.
- `crates/api/src/main.rs:157-239` shows the live router already mounts versioned `/api/v1/...` auth, connection, repository-import, API-key, search-context, sync-job, audit, analytics, review-webhook, and OAuth-client endpoints.
- `crates/api/src/main.rs:240-263` separately exposes versioned public repo/search/source/commit/review-webhook event routes, proving the public REST surface is broader than a single settings-only contract.
- `web/src/App.tsx:241-246` defines a typed `fetchJson<T>(...)` helper that expects machine-readable JSON from the versioned API.
- `web/src/App.tsx:297-335` and `web/src/App.tsx:462-466` show the frontend already consumes `/api/v1/repos`, `/api/v1/search`, and `/api/v1/repos/:repoId` JSON responses, while the repo/detail UI surfaces typed connection and sync metadata from those responses.

## Finding
The next smallest truthful drift is the parity-matrix row itself, not the acceptance spec or canonical gap report.

The acceptance spec and gap report already agree that Public REST API parity is only partial: the rewrite already exposes a real versioned `/api/v1/...` JSON surface consumed by the frontend, but it still lacks the broader endpoint completeness, connector/admin/operator coverage, and durable connection/repository behavior needed for full parity. The parity matrix is now the stale artifact because it still says `Needs audit | _TBD_ |` instead of reflecting that already-grounded `Partial` status and evidence.

## Smallest follow-up slice
`task20q2am63b` should tighten only the `specs/FEATURE_PARITY.md` `Public REST API` row so it records the current status as `Partial` and cites the already-grounded acceptance/gap-report evidence, without changing product behavior or broadening other integration rows.

## Out of scope
- Implementing broader Public REST API parity work from later roadmap tasks.
- Editing `docs/reports/2026-04-18-parity-gap-report.md` or `specs/acceptance/integrations.md`, which already express the conservative partial-parity state.
- Claiming full connector/admin/operator API parity, durable repo-operation parity, or provider-specific REST coverage that the repo still does not ship.
