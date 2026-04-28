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
2. References lookup returns a deduplicated, navigable list of usages; the bounded text-reference scanner covers Rust plus TypeScript/JavaScript source extensions (`.ts`, `.tsx`, `.js`, `.jsx`, `.mts`, `.cts`, `.mjs`, `.cjs`) so references parity matches the declaration extractor's supported language set.
3. Navigation requests are revision-aware and stable for a given indexed revision.
4. Unsupported languages fail gracefully with a capability message instead of a server error.
5. Symbol results link back to browseable source locations using the effective source revision.
6. When no explicit revision is requested and the primary browse/code-navigation store has no blob for an otherwise visible repository, definitions and references may fall back to the latest successful authorized local sync snapshot, using that terminal sync revision in response metadata and generated browse links.

## Permission behavior
- Definitions and references only return locations inside repositories the caller can access.
- Cross-repo references must honor the same permission boundary as search.

## Edge cases
- Multiple definitions should surface an ordered candidate list.
- Stale indexes should show degraded/stale status instead of silently mixing revisions.
- Generated files may be excluded from navigation indexes based on policy.

## Black-box examples
- Clicking a function symbol in a Rust or TypeScript file opens definition candidates.
- Requesting references for a helper function returns usages across accessible repositories.
- Requesting navigation in an unsupported file type returns a non-fatal capability response.
