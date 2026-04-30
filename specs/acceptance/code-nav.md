# Acceptance Spec: Code Navigation

## Scope
- Definitions lookup
- References lookup
- Symbol-aware navigation from file views

## Inputs
- Repository identifier
- Revision identifier
- File path
- Cursor position or symbol token
- Authenticated user context

## Expected behavior
1. A supported language returns one or more symbol definitions with file path and range. The current bounded extractor supports top-level Rust plus TypeScript/JavaScript declarations for functions, classes, interfaces, type aliases, enums, and constants.
2. References lookup returns a deduplicated, navigable list of usages; the bounded text-reference scanner covers Rust, TypeScript/JavaScript source extensions (`.ts`, `.tsx`, `.js`, `.jsx`, `.mts`, `.cts`, `.mjs`, `.cjs`), Python (`.py`), Go (`.go`), and JVM source files (`.java`, `.kt`, `.kts`), and revision-backed scans skip common generated/dependency directories (`.git`, `target`, `node_modules`, `dist`) before loading candidate blobs. Blank, control-character-bearing, and oversized symbol queries fail closed before browse/code-navigation lookup or local-sync fallback grep scans, and identifier-like matches require non-identifier boundaries so substring-only hits such as `prefix_symbol` are not returned for `symbol`.
3. Navigation requests are revision-aware and stable for a given indexed revision.
4. Unsupported languages fail gracefully with a capability message instead of a server error.
5. Symbol results link back to browseable source locations using the effective source revision; generated links percent-encode repository identifiers plus path/revision values before embedding them in URLs.
6. When no explicit revision is requested and the primary browse/code-navigation store has no blob for an otherwise visible repository, definitions and references may fall back to the latest successful authorized local sync snapshot, using that terminal sync revision in response metadata and generated browse links. Snapshot blob reads fail closed for symlinks that resolve outside the repository root, so fallback definitions cannot expose out-of-tree files; fallback reference scans keep the same bounded source-file extension scope as primary text-reference scans rather than widening into docs or arbitrary text files.

## Permission behavior
- Definitions and references only return locations inside repositories the caller can access.
- Blank present `revision` query values are rejected as bad requests instead of being treated as omitted and triggering default-HEAD or local-sync snapshot fallback behavior.
- Cross-repo references must honor the same permission boundary as search.

## Edge cases
- Multiple definitions should surface an ordered candidate list.
- Stale indexes should show degraded/stale status instead of silently mixing revisions.
- Generated files may be excluded from navigation indexes based on policy.
- Revision reference scans preserve Git paths containing embedded newlines by consuming NUL-delimited tree output before loading candidate source files, and non-UTF-8 Git paths fail closed for that path without aborting references from other valid UTF-8 source files in the same revision.

## Black-box examples
- Clicking a function symbol in a Rust or TypeScript file opens definition candidates.
- Requesting references for a helper function returns usages across accessible repositories.
- Requesting navigation in an unsupported file type returns a non-fatal capability response.
