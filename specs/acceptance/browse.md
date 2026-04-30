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
4. File tree browsing supports nested directories without requiring a full repository clone in the browser; for a visible repository with a successful local sync snapshot and no startup browse-store tree, no-revision tree reads may fall back to that latest caller-authorized snapshot.
5. Browse loading failures keep repository detail visible and offer a focused retry affordance instead of collapsing the page.
6. Empty directories render an explicit empty-state message without pretending a file is selected.
7. File source view renders syntax-highlighted text for supported languages and a safe fallback for unknown text formats; for a visible repository with a successful local sync snapshot and no startup browse-store blob, no-revision blob reads may fall back to that latest caller-authorized snapshot; local worktree and revision-backed blob reads are bounded by the same 8 MiB content budget and fail closed instead of reading or returning oversized source payloads; binary blobs expose metadata with an explicit binary flag, code-navigation endpoints report binary blobs as unsupported inputs, and the repository detail source panel shows a binary preview-unavailable notice instead of attempting lossy UTF-8 source rendering or code-navigation controls. When a user opens a definition/reference target from code navigation, the source panel switches to the target file and clears the prior symbol query/results instead of leaving stale navigation output attached to the previous file.
8. Commit list is ordered consistently and the repository-detail UI renders previous/next history controls from explicit `limit`/`offset` request controls plus response `page_info` (`limit`, `offset`, `has_next_page`, `next_offset`); backend commit-list reads clamp requested page sizes to 1..100 before invoking Git so oversized client requests cannot force unbounded history scans, and bounded Git capture now fails closed instead of parsing silently truncated commit stdout/stderr; commit list revision selectors plus commit detail/diff selectors must be non-empty, must not look like command-line options, and must be free of ASCII control characters and Git revision-expression/metacharacter syntax (`..`, `@{`, `^`, `~`, `:`, `?`, `*`, `[`, or backslash) before Git invocation so malformed NUL-bearing request values and parent/range selectors fail closed as not found instead of surfacing subprocess argument errors or widening the requested commit boundary; authenticated `GET /api/v1/repos/{repo_id}/refs` exposes bounded branch/tag ref summaries for visible repositories from the same commit store, sorted with branches before tags and the current branch first, without claiming full frontend branch-picker parity; for a visible repository with a successful local sync job and no seeded commit-store history, commit list/detail/diff reads may fall back to the latest caller-authorized local Git working tree, anchored to that job's recorded `synced_revision` so later unsynced local commits are not exposed by no-revision fallback reads, and explicit commit-list revision selectors outside that synced snapshot are rejected instead of widening to newer local history.
9. Commit detail view exposes changed files and summary metadata.
10. Diff view renders additions/deletions and handles renamed/copied files; backend diff-file metadata rejects malformed or dangerous Git-reported paths (empty, absolute, parent-directory, backslash-containing, or control-character-containing current or old paths) before using those paths for per-file patch loading or response metadata; backend diff-file patch text is loaded with literal pathspec semantics so metacharacter-bearing filenames such as `literal*.txt` cannot pull patches for sibling paths, is bounded to 64 KiB per file, and returns `patch_truncated: true` plus an inline truncation marker when that cap is reached, and the UI renders that state as an explicit truncation notice instead of displaying the marker as if it were the complete patch.

## Permission behavior
- Inaccessible repositories resolve as not found or forbidden according to product policy.
- File contents and diffs for unauthorized repositories must never leak through direct URL access.

## Edge cases
- Large directories must be progressively loaded or paginated, and oversized blob content must fail closed before unbounded local file reads or revision-backed Git stdout capture.
- Missing files at a selected revision return a clear not-found response; tree/blob paths with absolute, parent-directory, or NUL-byte components are rejected as bad requests before filesystem or Git lookup; local tree/blob reads and glob results must fail closed for symlinks that resolve outside the repository root, and local-sync snapshot fallback must fail closed when the selected snapshot root itself is a symlink; explicit browse/source/code-navigation revision selectors must be non-blank, free of control characters, resolve to a commit object, and fail closed before Git invocation as bad request for blank present values or as not found for control-character selectors and raw tree/blob object IDs, including treating blob-as-tree revision browse requests as not found instead of surfacing backend Git errors; revision-backed tree/blob browsing preserves Git paths containing embedded newlines by consuming NUL-delimited tree output where applicable and fails closed when bounded Git stdout/stderr capture caps are exceeded instead of returning partially captured revision data.
- Directories or symlinks selected through a revisioned file-source/blob read fail closed as not found instead of returning a textual `git show` tree listing or symlink target payload as file contents.
- Binary files should expose metadata/download behavior instead of broken text rendering; the current backend/API baseline returns path and size metadata with `is_binary: true` and empty text content for non-UTF-8 or NUL-containing local or revisioned blobs, and the frontend source panel now renders a binary metadata notice while suppressing text/code-navigation controls for that blob.
- Per-file textual diff patch extraction treats Git pathspec metacharacters literally before patch loading, and huge patches are truncated at the backend's 64 KiB response cap with an explicit `patch_truncated` indicator, while oversized Git subprocess output and oversized changed-file metadata fail closed before partial commit/diff metadata or revision-backed browse/blob data is parsed; the UI shows a truncation notice and does not render the backend marker as full patch content. Binary patches remain unavailable as patch text.

## Black-box examples
- Opening a repo page shows repository metadata, default branch, latest sync state, and browse/commit panels; applying a branch/tag/revision updates the route plus reloads tree/blob and commit data for that selected revision without losing the surrounding repo shell, and commit history previous/next controls request the backend-provided offsets from `page_info`.
- Selecting a file in a nested folder opens its highlighted source view.
- Opening a commit detail page shows author, timestamp, message, and changed files.
