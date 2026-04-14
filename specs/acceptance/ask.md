# Acceptance Spec: Ask

## Scope
- Ask-the-codebase experience
- Inline citations
- Chat threads/history/rename/visibility

## Inputs
- User prompt
- Repository or search context scope
- Optional conversation/thread identifier
- Authenticated user context

## Expected behavior
1. A new ask request creates or appends to a thread under the caller's identity.
2. Responses include machine-readable citations to source locations used in the answer.
3. Thread history can be listed and reopened later.
4. Users can rename a thread and change visibility according to org policy.
5. Ask retrieval only uses content from repositories in the active scope and visible to the caller.
6. Tool failures or provider failures surface partial results or actionable error states, not silent empty answers.

## Permission behavior
- Citations must never link to repositories or files outside the caller's access.
- Shared thread visibility must not widen access to underlying repository content.

## Edge cases
- Empty retrieval results may still produce a safe fallback answer that clearly states missing evidence.
- Citation rendering must survive moved or deleted files by pinning revision context.
- Long-running asks should support streaming or visible progress states.

## Black-box examples
- Asking "where is healthz implemented?" returns an answer with file citations.
- Reopening a previous thread shows earlier messages and preserved citations.
- Renaming a thread changes display name without changing citations or scope.
