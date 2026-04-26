# Acceptance Spec: Ask

## Scope
- Dedicated `#/ask` ask-the-codebase baseline
- Dedicated `#/chat` thread-history baseline
- Dedicated `#/agents` review-agent visibility baseline
- Repository-scoped ask submission
- Inline rendered citations
- Hash-restored active thread continuity for follow-up asks
- Authenticated thread list/detail/reopen flow on the dedicated chat route
- Dedicated agent-run detail restoration plus related webhook/delivery-attempt visibility
- Authenticated backend thread-title and thread-visibility update flow
- Dedicated chat-route thread title and private/shared visibility controls backed by the lifecycle API
- Authenticated visible-thread delete flow for the caller's own ask threads, exposed from the dedicated chat route
- Deferred: archive controls, richer conversation/source-preview UX, streaming/progress states, full chat parity, and richer review-agent management/retries/orchestration

## Inputs
- User prompt
- Repository scope selected on the dedicated ask route
- Optional conversation/thread identifier restored from the hash route
- Authenticated user context

## Expected behavior
1. The dedicated `#/ask` route lets a user select a visible repository scope and submit a prompt to `/api/v1/ask/completions`.
2. A new ask request creates or appends to a thread under the caller's identity, and the returned `thread_id` is reflected back into the hash route so refresh/follow-up asks stay on the same active thread.
3. Responses include machine-readable citations plus rendered inline citation labels/links to the cited repository location.
4. Ask retrieval only uses content from repositories in the active scope and visible to the caller.
5. The ask route shows truthful loading, empty, and error states instead of silently hiding failures.
6. Changing the selected repository scope clears any restored `thread_id` before the next submission so follow-up asks do not silently reuse a thread created under a different scope.
7. The dedicated `#/chat` route lists the caller's visible repo-scoped ask threads via authenticated thread-summary reads, restores a selected thread from the hash route, shows its prior messages, and lets the user continue that thread with `/api/v1/ask/completions` while keeping the route on `#/chat`. When `DATABASE_URL` is configured, the API-backed ask-thread store persists thread metadata and messages in PostgreSQL rather than process-local memory.
8. Authenticated thread-detail reads fail closed for missing, hidden, or out-of-scope threads, and returned thread citations remain limited to repositories/files visible to the caller.
9. Authenticated `PATCH /api/v1/ask/threads/{thread_id}` lets the caller update a visible thread's title and private/shared visibility metadata, trims titles, rejects empty titles or unknown visibility labels, and fails closed for hidden or out-of-scope threads without widening repository access.
10. The dedicated `#/chat` route exposes focused title and private/shared visibility controls for the selected visible thread, calls the authenticated PATCH lifecycle API, refreshes the visible detail/header/summary state from the sanitized response, and keeps the hash pinned to the same chat thread.
11. Authenticated `DELETE /api/v1/ask/threads/{thread_id}` removes the caller's own visible thread and returns `204 No Content`; hidden, out-of-scope, missing, or other-user threads fail closed as `404` and are not removed.
12. The dedicated `#/chat` route exposes a selected-thread delete control, calls the authenticated DELETE lifecycle API, removes the deleted summary/detail from local state, clears `thread_id` from the hash while preserving the repository scope, and ignores stale delete outcomes if the user has switched threads before the request resolves.
13. The dedicated `#/agents` route lists visible review-agent runs from `/api/v1/auth/review-agent-runs`, restores an optional `run_id` from the hash, loads `/api/v1/auth/review-agent-runs/{run_id}` when selected, and then loads the related `/api/v1/auth/review-webhook-delivery-attempts/{attempt_id}` plus `/api/v1/auth/review-webhooks/{webhook_id}` detail views on the same page.
14. Restored agent-run detail reads fail closed: if `/api/v1/auth/review-agent-runs/{run_id}` returns 404, the UI clears the selection and resets the route to `#/agents` instead of leaving a stale hidden run selected.
15. Archive controls, richer conversation/source-preview UX, and richer review-agent management, retries, and orchestration remain explicitly deferred follow-up work.

## Permission behavior
- Citations must never link to repositories or files outside the caller's access.
- Thread list/detail reads must never expose another caller's threads or hidden repository scope.
- Shared thread visibility must not widen access to underlying repository content.
- Thread metadata updates must reuse the same caller/repository visibility gate as thread detail reads before mutating title or visibility.
- Thread deletes must reuse the same caller/repository visibility gate as thread detail reads before removing a thread.

## Edge cases
- Empty retrieval results may still produce a safe fallback answer that clearly states missing evidence.
- Citation rendering should preserve stable source location labels, even when richer revision-pinned source-preview behavior remains follow-up work.
- Changing repository scope after restoring a `thread_id` must start a fresh active-thread context instead of silently reusing the old thread across scopes.
- Restoring `#/chat?thread_id=...` should recover the selected thread's repository scope and fail closed back to a fresh chat baseline if that thread can no longer be read.
- Deleting the selected chat thread should leave the chat route on a fresh repo-scoped baseline instead of retaining a stale `thread_id`.
- Long-running asks should support streaming or visible progress states.

## Black-box examples
- Asking "where is healthz implemented?" returns an answer with file citations.
- Refreshing `#/ask?repo_id=repo-1&thread_id=thread-9` keeps the route on the same active thread for a follow-up ask.
- Switching the ask route from `repo-1` to `repo-2` clears the restored active thread before the next submission.
- Refreshing `#/chat?thread_id=thread-9` reopens the selected thread, shows its prior messages, and keeps later follow-up asks pinned to `#/chat?...&thread_id=thread-9`.
