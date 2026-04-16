# Code Navigation References API Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Deliver Phase 3 / Task 17 by adding a minimal References API that returns navigable symbol usages while honoring revision-aware lookup semantics from the code-nav acceptance spec.

**Architecture:** Keep the first slice intentionally narrow. Reuse the existing definitions contract shape where practical, but implement references lookup against git content at a requested revision instead of the mutable working tree. Start with repository-scoped Rust-oriented symbol references using text matches and explicit unsupported-language handling so later indexing work can replace the internals without breaking the API contract.

**Scope**
- Add `GET /api/v1/repos/{repo_id}/references`
- Query params:
  - `path` required
  - `symbol` required
  - `revision` optional
- For supported Rust files:
  - verify file support using existing symbol extraction baseline
  - search the requested repo revision for text matches of the requested symbol
  - return a deduplicated, ordered list of navigable reference hits
  - include `browse_url` links back to blob locations
- For unsupported file extensions:
  - return non-fatal capability response with empty references
- Invalid relative paths must return `400`
- Unknown repo/path/revision-targeted file must return `404`

**Out of scope**
- Cross-repo references
- Semantic/AST-accurate references
- UI changes
- Background indexing / stale-index status
- Permission model beyond existing seeded repo boundary

**Design decisions**
1. **Revision-aware first:** do not use the current local search store for references because it only searches the working tree and would violate stable revision semantics.
2. **Minimal supported-language gate:** use `extract_symbols(path, blob_content)` to determine whether the source file type is supported before attempting repo-wide references lookup.
3. **Repo-scoped text references:** use git-backed revision content lookup to find lines containing the symbol token across files in the same repo revision.
4. **Navigable response contract:** each reference hit must include repo/path/line and a browse URL with encoded query params and optional revision.
5. **Deterministic ordering:** sort by path, then line number, then line text; deduplicate identical hits.

**Suggested response shape**
- `status: "supported"`
  - `repo_id`
  - `path`
  - `revision`
  - `symbol`
  - `references: [{ path, line_number, line, browse_url }]`
- `status: "unsupported"`
  - same envelope fields
  - `capability`
  - `references: []`

**TDD slices**
1. Missing required params -> `400`
2. Unknown repo/path -> `404`
3. Unsupported extension -> graceful unsupported response
4. Supported Rust file -> returns at least one reference hit for a known symbol
5. Requested revision changes results for a symbol introduced later
6. Browse URLs encode path/revision correctly
7. Parent directory traversal rejected with `400`
8. Results are deduplicated and ordered

**Implementation notes**
- Prefer adding reusable git-backed helpers in Rust rather than shelling out ad hoc from the handler.
- Reuse existing path normalization and git error mapping patterns from browse/commits code.
- Keep the public contract narrow enough that a future semantic index can replace the internals without changing callers.
