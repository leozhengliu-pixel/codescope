# Acceptance Spec: Generic/local Git parity

## Purpose
This document creates the dedicated black-box acceptance home for generic Git host and local-path repository parity. It freezes the current rewrite contract around authenticated connection management, generic/local connection metadata, and the user/operator-visible gaps that still remain before self-hosted local mirrors count as first-class Sourcebot parity.

## Grounding and limits
- This is an acceptance contract for the currently evidenced generic/local Git surface, not a claim that end-to-end local mirror parity is already shipped.
- Grounding for this document is limited to live rewrite evidence in:
  - `crates/models/src/lib.rs`
  - `crates/api/src/main.rs`
  - `crates/core/src/lib.rs`
  - `web/src/App.tsx`
  - `web/src/App.test.tsx`
  - `specs/acceptance/index.md`
  - `specs/acceptance/integrations.md`
  - `specs/FEATURE_PARITY.md`
  - `docs/reports/2026-04-18-parity-gap-report.md`
  - `docs/plans/2026-04-18-sourcebot-full-parity-roadmap.md`
- When the rewrite exposes only connection records, repo/detail metadata, and settings-shell CRUD, this spec records that truthfully and defers real local mirror import, enumeration, sync execution, and durable catalog parity to later slices.

## Scope
- Authenticated connection CRUD for generic Git host and local-path records
- User-visible generic/local connection metadata in repository detail and settings surfaces
- Acceptance boundaries for self-hosted local mirrors versus the still-missing real ingestion/runtime path

## Current rewrite evidence anchors
- `crates/models/src/lib.rs` defines `ConnectionKind::{GenericGit, Local}` plus matching `ConnectionConfig::{GenericGit { base_url }, Local { repo_path }}` variants.
- `crates/api/src/main.rs` exposes authenticated `GET/POST /api/v1/auth/connections` and `PUT/DELETE /api/v1/auth/connections/{connection_id}` routes.
- `crates/core/src/lib.rs` assembles repository detail responses that include the associated `connection` metadata.
- `web/src/App.tsx` already includes the limited `#/settings/connections` settings shell, fetches `/api/v1/auth/connections`, and creates/updates local connections using `config.repo_path` while non-local connections use `config.base_url`.
- `web/src/App.tsx` fetches authenticated `/api/v1/auth/repository-sync-jobs` alongside the settings connection inventory and renders read-only sync history per connection.
- `web/src/App.tsx` repository detail rendering already shows `Connection` and `Connection kind` for known repositories, while the settings connections shell formats local configs as `Repo path: ...`.
- `web/src/App.test.tsx` covers supported connection kinds including `generic_git` and `local`, plus local create/delete handling and the broader settings-shell connection management flows.

## Inputs
- Authenticated local-session context for connection management and sync-history visibility
- Connection kind (`generic_git` or `local`)
- Generic host configuration (`base_url`) or local mirror configuration (`repo_path`)
- Repository detail lookup for repo-scoped connection metadata
- Read-only repository sync-job history for the configured connection

## Expected behavior
1. An authenticated admin/settings user can list existing connection records, including generic Git host and local-path variants.
2. Creating or updating a `local` connection uses a `repo_path` configuration contract rather than a SaaS-style `base_url`.
3. Creating or updating a `generic_git` connection uses a `base_url` configuration contract rather than a local path.
4. Repository detail surfaces expose the associated connection metadata, including connection name and kind, for repositories already present in the catalog.
5. The settings connections shell can show read-only repository sync history alongside the connection inventory without implying that full local/generic ingestion parity is already complete.
6. The rewrite must not claim generic/local Git parity solely because the models and settings shell exist; real repository enumeration/import, sync execution, and durable catalog-backed visibility still have to be proven.

## Permission behavior
- Generic/local connection CRUD requires an authenticated local session and follows the same auth/admin settings boundary as the existing `/api/v1/auth/connections` surface.
- Repository detail and sync-history visibility remain constrained by the caller's current repository visibility; generic/local metadata must not bypass the normal repo-visibility model.
- Hidden repositories or cross-org invisible sync jobs must fail closed rather than leaking through generic/local connection surfaces.

## Edge cases
- Local-path connection UX is distinct from SaaS/git-host providers: `repo_path` is the visible editable contract, not `base_url`.
- Generic/local connection records can exist before the rewrite proves real repository enumeration or indexing for those hosts.
- Read-only sync history can be visible for a connection even while the durable catalog-backed repository list/detail parity remains incomplete.
- Fixture-backed or seeded repository detail metadata is not sufficient evidence that adding a new local bare repository path triggers a real import/index cycle.

## Black-box examples
- Opening `#/settings/connections` shows existing `generic_git` and `local` connection records in the authenticated settings inventory.
- Creating a local mirror connection sends a nested request with `kind: "local"` and `config: { "provider": "local", "repo_path": "/srv/git/mirror" }` rather than a host-style `base_url` payload.
- Editing a generic Git host connection preserves the host-style `base_url` contract.
- Opening `#/repos/:repoId` shows repository detail with connection name/kind metadata for an already known repository.
- An authenticated user with repository visibility can inspect read-only sync history for repositories tied to a configured connection, but that does not by itself prove the rewrite can newly enumerate or ingest repositories from that host.

## Deferred parity gaps locked by this spec
This spec intentionally records the remaining gaps instead of over-claiming parity:

1. **Real generic/local ingestion parity is still missing.** The current rewrite evidence covers connection records, settings CRUD, and metadata display, but not the end-to-end workflow where adding a new local bare repository path or generic Git host actually enumerates/imports searchable repositories.
2. **Durable catalog parity is still blocked.** Repository list/detail parity on the persisted catalog path remains constrained by the unimplemented Postgres catalog repository queries recorded elsewhere in the roadmap state and repository-operations acceptance spec.
3. **Per-connection sync/index parity is still shallow.** The settings shell shows read-only sync history, but not full operator controls, retries, import progress, repo-discovery flows, or durable per-connection indexing status parity.
4. **Broader provider/runtime parity is still deferred.** GitLab, GitHub, and later multi-host provider work still need real auth/configuration, enumeration, sync execution, webhook/runtime behavior, and provider-specific acceptance follow-ups.
5. **Settings/admin navigation parity is still incomplete.** The rewrite has a limited `#/settings/connections` shell, but not the broader auth/admin/settings route family that would make generic/local connection management first-class product navigation.

## Acceptance-evidence anchors for future slices
- Use this spec as the acceptance home for roadmap Task 20 follow-ups on real generic/local Git ingestion, repo discovery/import, per-connection sync/index status, and later settings/admin UX expansion.
- Extend this document, rather than only `specs/acceptance/integrations.md`, when future slices add real local mirror import/index behavior, generic-host enumeration, per-connection operator controls, or durable catalog-backed generic/local repository parity.
