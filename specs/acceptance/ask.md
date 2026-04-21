# Acceptance Spec: Ask

## Scope
- Dedicated `#/ask` ask-the-codebase baseline
- Repository-scoped ask submission
- Inline rendered citations
- Hash-restored active thread continuity for follow-up asks
- Deferred: full thread history/list/reopen, rename/delete/visibility management, and review-agent/agents parity

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
7. Full thread-history browsing/reopen UX, rename/delete/visibility controls, and agents management remain explicitly deferred follow-up work.

## Permission behavior
- Citations must never link to repositories or files outside the caller's access.
- Shared thread visibility must not widen access to underlying repository content.

## Edge cases
- Empty retrieval results may still produce a safe fallback answer that clearly states missing evidence.
- Citation rendering should preserve stable source location labels, even when richer revision-pinned source-preview behavior remains follow-up work.
- Changing repository scope after restoring a `thread_id` must start a fresh active-thread context instead of silently reusing the old thread across scopes.
- Long-running asks should support streaming or visible progress states.

## Black-box examples
- Asking "where is healthz implemented?" returns an answer with file citations.
- Refreshing `#/ask?repo_id=repo-1&thread_id=thread-9` keeps the route on the same active thread for a follow-up ask.
- Switching the ask route from `repo-1` to `repo-2` clears the restored active thread before the next submission.
