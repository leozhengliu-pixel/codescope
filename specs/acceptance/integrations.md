# Acceptance Spec: Integrations

## Scope
- Git host connectors
- Local Git / generic Git host ingestion
- OIDC / SSO providers
- MCP server
- Public REST API surface

## Inputs
- Connector configuration
- Repository sync requests
- Identity provider metadata/configuration
- External client API requests

## Expected behavior
1. Supported connectors can register credentials/configuration and enumerate accessible repositories.
2. Sync state and indexing status are inspectable per connection and per repository.
3. Local Git or generic Git host ingestion works without requiring a SaaS provider.
4. OIDC/SSO login can be enabled with provider metadata and mapped to local users/orgs.
5. MCP server exposes repository-aware tools under the caller's permission scope.
6. Public REST APIs are versioned and return stable machine-readable responses.

## Permission behavior
- Connector sync must not import repositories invisible to the configured principal unless explicitly allowed by policy.
- API and MCP requests must enforce the same repository visibility model as the web app.

## Edge cases
- Partial connector failures should mark per-repository sync state instead of failing the entire installation silently.
- Invalid provider credentials should be diagnosable without exposing secrets.
- Disconnected integrations should degrade gracefully for dependent features.

## Black-box examples
- Adding a GitHub connection enumerates accessible repositories and starts sync.
- Adding a local bare repository path makes it searchable after indexing completes.
- Calling the public REST API returns versioned JSON and permission-scoped results.
