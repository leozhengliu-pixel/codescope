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
3. A valid query returns ranked matches across all repositories the user can access.
4. Each result includes repository, revision or branch context, file path, line-oriented snippet, and enough metadata to open the file view.
5. Filters narrow the result set without leaking inaccessible repositories.
6. Empty-result queries return a successful response with zero hits, not an internal error.
7. Invalid regex queries return a user-safe validation error.
8. Pagination is stable for the same repository revision and query parameters.

## Permission behavior
- Results from repositories outside the caller's permissions must not appear in counts, snippets, suggestions, or facets.
- Search contexts must only expand to repositories visible to the caller.

## Edge cases
- Very large repositories must remain searchable without loading full trees into memory.
- Binary files are excluded by default.
- Queries that hit generated/minified/vendor content must be suppressible by policy.
- Branches missing an index should surface partial availability rather than silent omission.

## Black-box examples
- Opening `#/search` shows a dedicated code-search page with the existing query input, repository filter, result list, and direct links into `#/repos/:repoId?path=...&from=search` so a user can jump from a match into repository source without losing obvious search-route context.
- Query `router` across all accessible repos returns matches from multiple repos.
- Query `lang:rust path:crates/api healthz` returns matches only under `crates/api` in Rust files.
- Query with invalid regex `([a-z` returns a validation error with no server crash.
