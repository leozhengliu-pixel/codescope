# Sourcebot Rewrite Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Build a clean-room, permissively licensed, Sourcebot-compatible product using a simpler and more efficient architecture.

**Architecture:** Rust services own API, background jobs, and search/index coordination. A separate React frontend consumes those APIs. PostgreSQL stores metadata and permissions; search artifacts live in object storage or local disk in dev.

**Tech Stack:** Rust, Axum, Tokio, SQLx, PostgreSQL, React, TypeScript, Vite, Tantivy, regex-automata, tree-sitter.

---

## Phase 0: Spec and boundaries

### Task 1: Freeze clean-room rules
**Objective:** Ensure implementation uses only local specs and plans.

**Files:**
- Verify: `specs/CLEAN_ROOM_RULES.md`
- Verify: `specs/FEATURE_PARITY.md`

**Steps:**
1. Review clean-room constraints.
2. Expand feature parity into concrete acceptance criteria per feature.
3. Do not implement before criteria are written.

### Task 2: Convert parity matrix into acceptance specs
**Objective:** Turn feature bullets into testable product contracts.

**Files:**
- Create: `specs/acceptance/*.md`

**Steps:**
1. Write one acceptance file per capability area: search, browse, code-nav, ask, auth, integrations.
2. For each file, define inputs, expected output, edge cases, and permission behavior.
3. Add black-box examples only.

---

## Phase 1: Monorepo and runtime bootstrap

### Task 3: Create Rust workspace skeleton
**Objective:** Establish crates for api, worker, search, core, git, config, and models.

**Files:**
- Create: `Cargo.toml`
- Create: `crates/api/`
- Create: `crates/worker/`
- Create: `crates/search/`
- Create: `crates/core/`
- Create: `crates/git/`
- Create: `crates/config/`
- Create: `crates/models/`

### Task 4: Create web app skeleton
**Objective:** Establish the React frontend boundary early.

**Files:**
- Create: `web/package.json`
- Create: `web/src/`

### Task 5: Add local dev orchestration
**Objective:** Run API, DB, and frontend locally with one command.

**Files:**
- Create: `docker-compose.yml`
- Create: `Makefile`
- Create: `.env.example`

---

## Phase 2: Search and browse MVP

### Task 6: Repository catalog and connection model
### Task 7: Git mirror/fetch worker
### Task 8: Branch/revision metadata model
### Task 9: Initial file tree extraction
### Task 10: Text search indexing pipeline
### Task 11: Search API with filters and snippets
### Task 12: Repo list / repo detail UI
### Task 13: File tree + file source UI
### Task 14: Commit list / diff APIs and UI

---

## Phase 3: Code navigation

### Task 15: Symbol extraction baseline
### Task 16: Definitions API
### Task 17: References API
### Task 18: Navigation UI interactions

---

## Phase 4: Ask and tool orchestration

### Task 19: LLM provider abstraction
### Task 20: Retrieval tools (grep, glob, read file, list tree, list repos)
### Task 21: Ask session and chat thread persistence
### Task 22: Citation model and renderer
### Task 23: Repo-scoped ask experience
### Task 24: MCP server

---

## Phase 5: Auth, orgs, permissions

### Task 25: Local auth + onboarding
### Task 26: Org/member/role model
### Task 27: Invite flow and account linking
### Task 28: Repo permission sync model
### Task 29: Search/file/ask permission enforcement
### Task 30: API keys

---

## Phase 6: Advanced parity

### Task 31: Search contexts
### Task 32: Audit logs
### Task 33: Analytics
### Task 34: OAuth client flows
### Task 35: Review agent / webhook automation

---

## Verification gates
- Every phase must have API contracts and UI acceptance criteria before implementation.
- No upstream source code may be copied.
- Every feature must be validated against `specs/FEATURE_PARITY.md`.
- Keep the first release self-host-friendly: minimal dependencies, simple deployment, observability built in.
