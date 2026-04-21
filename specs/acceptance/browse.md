# Acceptance Spec: Browse

## Scope
- Repository list page
- Repository detail page
- File tree explorer
- File source view
- Commit list, commit detail, and diff view

## Inputs
- Repository identifier
- Optional branch, tag, or revision selector where supported
- Optional file path
- Authenticated user context

## Expected behavior
1. Repository list returns only repositories visible to the caller.
2. Repository detail shows key metadata, default branch, sync/index status, and a branch/tag/revision control that can keep the repo route stable while reloading browse and commit panels for the selected revision.
3. Repository detail loading failures keep the route stable with a clear retry affordance and a contextual way back to the repository list or dedicated search results.
4. File tree browsing supports nested directories without requiring a full repository clone in the browser.
5. Browse loading failures keep repository detail visible and offer a focused retry affordance instead of collapsing the page.
6. Empty directories render an explicit empty-state message without pretending a file is selected.
7. File source view renders syntax-highlighted text for supported languages and a safe fallback for unknown text formats.
8. Commit list is ordered consistently and supports pagination.
9. Commit detail view exposes changed files and summary metadata.
10. Diff view renders additions/deletions and handles renamed files.

## Permission behavior
- Inaccessible repositories resolve as not found or forbidden according to product policy.
- File contents and diffs for unauthorized repositories must never leak through direct URL access.

## Edge cases
- Large directories must be progressively loaded or paginated.
- Missing files at a selected revision return a clear not-found response.
- Binary files should expose metadata/download behavior instead of broken text rendering.
- Huge diffs may be truncated with an explicit truncation indicator.

## Black-box examples
- Opening a repo page shows repository metadata, default branch, latest sync state, and browse/commit panels, and applying a branch/tag/revision updates the route plus reloads tree/blob and commit data for that selected revision without losing the surrounding repo shell.
- Selecting a file in a nested folder opens its highlighted source view.
- Opening a commit detail page shows author, timestamp, message, and changed files.
