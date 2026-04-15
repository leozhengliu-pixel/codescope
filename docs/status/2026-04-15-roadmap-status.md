# Sourcebot Rewrite Roadmap Status

> This document reconciles the informal execution-stage names used during implementation with the formal roadmap in `docs/plans/2026-04-14-sourcebot-rewrite-plan.md`.

## Why numbering looked inconsistent

During implementation, work was sometimes described with informal sequential "phase7/phase8/phase9" labels for execution chunks. Those labels were **not** the canonical roadmap.

The canonical roadmap is the plan document, which defines **Phase 0–6** only.

## Canonical roadmap status

### Phase 0 — Spec and boundaries
Status: done
- Clean-room rules committed.
- Acceptance specs committed for search, browse, code-nav, ask, auth, integrations.

Key commit:
- `b013b50 feat: add acceptance specs and bootstrap api`

### Phase 1 — Monorepo and runtime bootstrap
Status: done
- Rust workspace created.
- React/Vite web app created.
- Local dev orchestration added (`docker-compose.yml`, `Makefile`, `.env.example`).
- Seed API/bootstrap landed.

Key commits:
- `5b3fbd0 chore: bootstrap clean-room sourcebot rewrite`
- `b013b50 feat: add acceptance specs and bootstrap api`
- `8d78888 feat: add repo api and local dev tooling`

### Phase 2 — Search and browse MVP
Status: functionally complete for the current MVP slice

Delivered so far:
- Repository catalog abstraction and seeded repo metadata.
- Repository list/detail API and web UI.
- Browse tree/blob API and file explorer UI.
- Minimal local text search API and UI.
- Commit list/detail API and UI.
- Commit diff API and UI.
- Verified exclusion of non-source directories in local search (`.git`, `target`, `node_modules`, `dist`).

Key commits:
- `67ae3df feat: add repo dashboard web ui`
- `83aa27b feat: add catalog store abstraction and web ui tests`
- `c221864 feat: add repository browse api and file explorer`
- `556fb45 feat: add minimal search api and web ui`
- `fe7f21f feat: add commit history api and web ui`
- `760b54a feat: add commit diff api and web ui`

### Phase 3 — Code navigation
Status: in progress

Planned task order from the canonical roadmap:
1. Task 15: Symbol extraction baseline ✅ complete
2. Task 16: Definitions API
3. Task 17: References API
4. Task 18: Navigation UI interactions

Current focus:
- Landed a minimal symbol extraction baseline that produces stable symbol definition candidates for supported languages, starting with Rust source files.
- Next up: expose symbol definitions through the API layer.

### Phase 4 — Ask and tool orchestration
Status: not started

### Phase 5 — Auth, orgs, permissions
Status: not started

### Phase 6 — Advanced parity
Status: not started

## Informal-to-canonical mapping

The previously mentioned informal execution stages roughly map like this:

- Informal bootstrap stages → Canonical Phase 0–1
- Informal repo/catalog/browse/search/commits stages (including the previously mentioned phase7/phase8/phase9 wording) → Canonical Phase 2
- Current upcoming work → Canonical Phase 3

## Practical summary

- We are **not regressing**.
- The apparent mismatch came from mixing **informal execution-stage labels** with the **formal roadmap phases**.
- The repository should now use the canonical roadmap terms from `docs/plans/2026-04-14-sourcebot-rewrite-plan.md` in future status updates.
