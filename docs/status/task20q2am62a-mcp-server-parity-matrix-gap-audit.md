# task20q2am62a — MCP server parity-matrix gap audit

## Scope
Audit the next smallest MCP-server documentation drift after the OIDC/SSO parity-matrix follow-up closed `task20q2am61b`.

## Grounded evidence
- `specs/FEATURE_PARITY.md:34` still leaves the `MCP server` row at `Needs audit | _TBD_ |`.
- `specs/acceptance/integrations.md:25` already keeps the acceptance contract conservative and specific: the MCP server should expose repository-aware tools under the caller's permission scope.
- `specs/acceptance/integrations.md:30` separately states that API and MCP requests must enforce the same repository visibility model as the web app.
- `docs/reports/2026-04-18-parity-gap-report.md:67` already records the current rewrite as `Partial`, grounding that `crates/mcp/src/lib.rs` ships an MCP manifest plus retrieval tool definitions and `execute_tool_call(...)` support for `list_repos`, `list_tree`, `read_file`, `glob`, and `grep`.
- `docs/reports/2026-04-18-parity-gap-report.md:67` also already records the real remaining gap: transport/runtime/auth wiring and explicit permission-scoped acceptance evidence are still missing.
- `crates/mcp/src/lib.rs:40-75` defines the serialized MCP server manifest and stable retrieval tool definitions.
- `crates/mcp/src/lib.rs:154-245` implements `execute_tool_call(...)` over retrieval-tool context and store traits, proving crate-local MCP tool execution exists.
- Repo-wide code search during this audit found no MCP transport/runtime wiring under `crates/api/**/*.rs` or frontend MCP surfaces under `web/src/**/*.tsx`; the live code references stay inside `crates/mcp/src/lib.rs` plus its crate-local tests.

## Finding
The next smallest truthful drift is the parity-matrix row itself, not the acceptance spec or canonical gap report.

The acceptance spec and gap report already agree that MCP parity is only partial: the rewrite has a real library-level MCP manifest/tool-execution contract, but it still lacks transport/runtime/auth wiring and explicit permission-scoped end-to-end acceptance evidence. The parity matrix is now the stale artifact because it still says `Needs audit | _TBD_ |` instead of reflecting that already-grounded `Partial` status and evidence.

## Smallest follow-up slice
`task20q2am62b` should tighten only the `specs/FEATURE_PARITY.md` `MCP server` row so it records the current status as `Partial` and cites the already-grounded acceptance/gap-report evidence, without changing product behavior or broadening other integration rows.

## Out of scope
- Implementing roadmap Task 46 MCP transport/runtime/auth behavior.
- Editing `docs/reports/2026-04-18-parity-gap-report.md` or `specs/acceptance/integrations.md`, which already express the conservative partial-parity state.
- Claiming end-to-end permission-scoped MCP parity, frontend MCP UX, or API/runtime wiring that the repo still does not ship.
