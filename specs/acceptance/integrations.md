# Acceptance Spec: Integrations

## Scope
- Git host connectors
- Local Git / generic Git host ingestion
- OIDC / SSO providers
- MCP server
- Public REST API surface

## Current slice note
- `specs/acceptance/generic-local-git.md` is now the dedicated acceptance home for the currently evidenced generic/local Git connection-management and metadata baseline.
- This document remains the broad integrations umbrella and should keep host/provider claims conservative until later slices land real provider auth, enumeration, ingestion, and runtime parity.

## Inputs
- Connector configuration
- Repository sync requests
- Identity provider metadata/configuration
- External client API requests

## Expected behavior
1. Supported connectors have a stable configuration-management contract, while provider-specific repository enumeration/import behavior is locked by dedicated host specs as it becomes real.
2. Sync state and indexing visibility are tracked per repository today, with richer per-connection and durable index-status parity deferred to repository-operations and host-specific follow-up specs.
3. Generic/local Git parity is currently grounded by `specs/acceptance/generic-local-git.md`: authenticated connection CRUD, settings-shell metadata, repo-detail connection metadata, and read-only sync-history visibility exist, while real host ingestion without a SaaS provider remains deferred.
4. OIDC/SSO login can be enabled with provider metadata and mapped to local users/orgs once the dedicated identity-provider slices land.
5. MCP server exposes repository-aware tools under the caller's permission scope, and the current local browse retrieval baseline skips obvious build-artifact directories (`.git`, `target`, `node_modules`, `dist`) during repo-scoped recursive file discovery.
6. Public REST APIs are versioned and return stable machine-readable responses.

## Permission behavior
- Connector sync must not import repositories invisible to the configured principal unless explicitly allowed by policy.
- API and MCP requests must enforce the same repository visibility model as the web app.

## Edge cases
- Partial connector failures should mark per-repository sync state instead of failing the entire installation silently.
- Invalid provider credentials should be diagnosable without exposing secrets.
- Disconnected integrations should degrade gracefully for dependent features.

## Black-box examples
- An authenticated admin can open the settings connections shell, inspect existing connection records, and manage generic/local connection metadata through the versioned `/api/v1/auth/connections` API.
- Repository detail shows connection metadata, and authenticated sync-history views remain read-only until later provider/runtime slices land real enumeration/import/index behavior.
- Calling the public REST API returns versioned JSON and permission-scoped results.
