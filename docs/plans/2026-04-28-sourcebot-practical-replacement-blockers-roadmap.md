# Sourcebot Practical Replacement Blockers Roadmap

> **For Hermes:** Use `stateful-roadmap-autopilot-anti-microslice`, `test-driven-development`, `requesting-code-review`, and the relevant Sourcebot skills to execute one smallest meaningful release-blocker closure at a time.

**Goal:** Close the remaining blockers from the Task 100 release-candidate audit so `sourcebot-rewrite` can eventually be certified as a practical Sourcebot replacement.

**Architecture:** Keep the current Rust + React clean-room rewrite, but prioritize product/runtime capabilities that the Task 100 audit still marks as missing or partial. Each closure should add user-visible behavior, operator-visible behavior, or implementation-backed acceptance evidence; docs-only closures are allowed only when they resolve a concrete audit drift exposed by existing evidence.

**Tech Stack:** Rust, Axum, SQLx, PostgreSQL, Tokio workers, Git provider integrations, local Git/runtime artifacts, React, TypeScript, Vite, MCP, and repo-local smoke/contract scripts.

---

## Why this roadmap exists

Task 100 ran the release-candidate smoke/audit path and failed closed: the repo-local evidence still cannot honestly support a practical Sourcebot replacement claim. The previous follow-on roadmap produced substantial bounded baselines, but the final checklist still has release blockers in durable metadata breadth, real provider and identity flows, production search/indexing depth, richer ask/admin lifecycles, MCP protocol interoperability, worker supervision/retry orchestration, and production-grade operator runtime behavior.

This roadmap intentionally starts from those blockers instead of reopening the completed Task 86-100 sequence or pretending task-count completion equals release parity.

---

## Definition of done

This roadmap is complete only when a fresh release-candidate audit can truthfully mark all of these as closed:

1. Durable metadata is the default for catalog, auth/org, permissions, ask/chat, review-agent, audit/analytics, provider/runtime, and migration/upgrade paths.
2. Search, browse, commit, and code-navigation are backed by practical indexed/runtime behavior with production-shaped status and failure semantics.
3. Ask/chat and admin/org/auth lifecycles cover the practical Sourcebot workflows rather than only bounded local baselines.
4. Provider integrations include real GitHub/GitLab/generic-host identity, discovery/import, sync, and enough multi-host support to justify the replacement claim.
5. OIDC/SSO login/callback/linking flows exist end to end, not just linked-account inventory and callback-status copy.
6. MCP covers practical client/session interoperability and permission-scoped tool execution beyond the current HTTP/API bridge.
7. Worker/runtime behavior includes real scheduling, retry/backoff, supervision, observability, and recovery rather than one-shot or explicitly bounded local invocations.
8. Operator runbooks, readiness, backup/restore, upgrade, and deployment evidence are production-shaped rather than local-development-only.
9. The final parity audit and smoke matrix can certify practical replacement parity without fail-closed caveats.

---

## Execution rules

- `docs/status/roadmap-state.yaml` remains the single source of truth.
- Execute exactly one smallest meaningful release-blocker closure per run.
- Prefer implementation-backed vertical slices over paperwork.
- Keep release claims bounded and fail closed whenever evidence remains partial.
- Run PostgreSQL preflight before SQLx/PostgreSQL verification.
- Long smoke/test/build commands must use background execution with completion notification.
- Every closure must include focused verification, broader confidence appropriate to the changed surface, independent review, substantive commit, truthful state update, push, and then stop.

---

## Task 101: Durable metadata breadth and migration hardening

Close the remaining durable metadata gaps that still force file-backed or split-backend semantics across permissions, broader organization aggregates, audit/analytics, connection/runtime state, ask/chat state, review-agent state, and migration/upgrade safety. Start with the highest-value vertical slice that moves an actually used runtime surface to PostgreSQL with SQLx coverage and fail-closed compatibility behavior.

## Task 102: Production search/indexing runtime parity

Replace bounded local/snapshot artifact fallback semantics with practical search/indexing behavior, status reporting, failure recovery, pagination/relevance guarantees, and operator-visible index lifecycle evidence.

## Task 103: Browse, source, commit, and code-navigation polish

Close practical repository-navigation gaps: branch/ref discovery, large and binary file UX, syntax highlighting, commit pagination/diff hardening, multi-language symbol quality, and durable/runtime status alignment.

## Task 104: Ask/chat and review-agent lifecycle parity

Finish richer conversation lifecycle behavior, archive/source-preview/progress states, review-agent orchestration, retries, delivery status, and durable recovery semantics needed for practical day-to-day use.

## Task 105: Auth/admin/org and external identity parity

Finish org/member/invite/admin flows, permission-sync management, API-key/OAuth token lifecycle, audit/analytics depth, and end-to-end OIDC/SSO login/callback/linking/account-merge behavior.

## Task 106: Provider integration and sync runtime parity

Move beyond bounded local Git and generic metadata probes into real GitHub/GitLab/generic-host discovery/import/sync execution, provider-specific credentials, multi-host coverage, retry/recovery behavior, and visible sync/index progress.

## Task 107: MCP protocol/client interoperability parity

Extend the current authenticated HTTP/API MCP bridge into practical MCP protocol/session/client interoperability with permission-scoped retrieval and end-to-end client evidence.

## Task 108: Worker supervision and operator runtime parity

Close real scheduling, retry/backoff, supervision, heartbeat/status, metrics/logging, readiness, backup/restore, upgrade, and deployment/runbook gaps so operators can run the rewrite beyond a local bounded smoke baseline.

## Task 109: Final practical-replacement release audit

Run the full release-candidate smoke matrix and a fresh parity audit. Certify practical replacement only if all blockers above are closed by repo-local evidence; otherwise fail closed again with an updated blocker roadmap.
