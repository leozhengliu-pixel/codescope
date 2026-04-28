# Acceptance Spec: Search

## Scope
- Dedicated `#/search` frontend route for API-backed code search
- Cross-repo and cross-branch code search
- Literal, regex, and boolean-style query modes
- Repo/language/path filters
- Snippet results with file and revision context

## Inputs
- User query string
- Optional search mode: literal | regex | boolean
- Optional filters: repositories, branches, languages, path globs
- Authenticated user context

## Expected behavior
1. The top-level frontend shell exposes a dedicated `#/search` route so code search is reachable without returning to the repository home inventory.
2. That route reuses the existing API-backed search flow with query input, optional repository filter, and result rendering grounded in `/api/v1/search`.
3. A valid query returns ranked matches across all repositories the user can access; the current local/index-artifact baseline matches query text case-insensitively while preserving the caller's trimmed query text in the response.
4. Each result includes repository, revision or branch context, file path, line-oriented snippet, and enough metadata to open the file view.
5. Filters narrow the result set without leaking inaccessible repositories.
6. Authenticated users can create, list, and delete saved search contexts through `/api/v1/auth/search-contexts`; `GET /api/v1/search?context_id=...` applies the saved context as an additional repository-scope filter.
7. Empty-result queries return a successful response with zero hits, not an internal error.
8. Invalid regex queries return a user-safe validation error.
9. `/api/v1/search` accepts bounded `limit` and `offset` parameters and returns a `pagination` object containing `limit`, `offset`, visibility-scoped `total_count`, and `has_more` for the current query.
10. Pagination is stable for the same repository revision and query parameters.

## Permission behavior
- Results from repositories outside the caller's permissions must not appear in results, pagination counts, snippets, suggestions, or facets.
- Search contexts must only expand to repositories visible to the caller.
- Saved search contexts are caller-owned. Unknown, deleted, other-user, or present-but-blank context ids fail closed instead of broadening search, and an explicit `repo_id` outside the saved context scope fails closed.
- Explicit `repo_id` filters are fail-closed: hidden, unknown, or present-but-blank repository ids return `404` instead of being treated as absent and broadening the search across all visible repositories.
- This saved-context, pagination, and local search-matching baseline now includes API metadata plus bounded `#/search` previous/next controls for result pages, explicit blank-`repo_id` fail-closed behavior, and case-insensitive text matching for the startup/local-sync search store; saved contexts remain backend/API-only and file-backed in the organization aggregate, and this does not yet claim frontend context management UI, SQL-backed context durability, richer grammar, full relevance tuning, or full stable pagination parity across changing indexes/revisions.

## Edge cases
- Very large repositories must remain searchable without loading full trees into memory.
- Binary files are excluded by default.
- Queries that hit generated/minified/vendor content must be suppressible by policy.
- Branches missing an index should surface partial availability rather than silent omission.

## Black-box examples
- Opening `#/search` shows a dedicated code-search page with the existing query input, repository filter, bounded result-page summary, previous/next controls backed by `/api/v1/search?limit=20&offset=...`, and direct links into `#/repos/:repoId?path=...&from=search` so a user can jump from a match into repository source without losing obvious search-route context.
- Query `router` across all accessible repos returns matches from multiple repos.
- Query `lang:rust path:crates/api healthz` returns matches only under `crates/api` in Rust files.
- Query with invalid regex `([a-z` returns a validation error with no server crash.
