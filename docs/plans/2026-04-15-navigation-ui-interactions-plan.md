# Phase 3 Task 18 — Navigation UI interactions

## Goal
Deliver the smallest end-to-end UI slice for code navigation on the repository file view, consuming the already-landed definitions and references APIs.

## Inputs considered
- `specs/acceptance/code-nav.md`
- Existing repository browse/source UI in `web/src/App.tsx`
- Existing backend contracts in `crates/api/src/main.rs`

## Minimum acceptable closure
1. Navigation starts from the file source panel for the currently selected file.
2. The user can provide a symbol token from that file and trigger:
   - definitions lookup
   - references lookup
3. The UI calls:
   - `GET /api/v1/repos/:repo_id/definitions?path=...&symbol=...`
   - `GET /api/v1/repos/:repo_id/references?path=...&symbol=...`
4. Results render as navigable source locations.
5. Clicking a result updates the browse/source view to the target file.
6. Supported responses show ordered result lists.
7. Unsupported responses show the backend capability message as a non-fatal notice.
8. Empty results render a friendly no-results state.
9. The visible UI surfaces returned revision metadata when available, preserving revision-aware semantics in the presentation.

## Intentional non-goals for this slice
- True token click/selection extraction from rendered source text.
- Cross-repo references UI.
- Scroll-to-line highlighting in the source viewer.
- Advanced stale-index handling beyond showing the revision/capability returned by the API.

## Suggested UI shape
Add a compact "Code navigation" panel beside/under the source viewer with:
- symbol input prefilled from selected text state or blank manual entry
- "Find definitions" button
- "Find references" button
- loading/error state
- capability notice for unsupported responses
- result cards linking back into the current repo browse view

## Result-link behavior
Use backend-provided `browse_url` as the canonical location source, but parse enough of it in the frontend to:
- switch to the correct repository detail route
- select the target file in browse view
- keep revision if present
- optionally display line/range metadata in the results list

## Testing scope
Add frontend tests that prove:
1. definitions lookup from a selected file issues the expected request and renders definition candidates.
2. references lookup renders usages and lets the user navigate to a referenced file.
3. unsupported responses render a capability notice without breaking the source view.
4. empty supported responses render a no-results message.

## Verification target
- `cd web && npm test -- --run`
- `cd web && npm run build`
- controller-level live smoke after implementation
