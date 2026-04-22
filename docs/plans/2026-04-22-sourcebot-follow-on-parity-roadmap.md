# Sourcebot Follow-On Full-Parity Roadmap

> **For Hermes:** Use `stateful-roadmap-autopilot-anti-microslice`, `writing-plans`, `test-driven-development`, `requesting-code-review`, and `subagent-driven-development` to execute this roadmap one smallest meaningful closure at a time.

**Goal:** Close the blockers identified by the final parity audit so `sourcebot-rewrite` can truthfully ship as a practical Sourcebot replacement rather than a bounded clean-room subset.

**Architecture:** Preserve the current Rust + React clean-room architecture, but replace the remaining stubbed or file-backed parity gaps with durable stores, real indexing/runtime paths, richer auth/provider/admin behavior, and production-grade operator controls. Each execution unit should end in a user-visible capability, operator-visible capability, or a real implementation+contract closure instead of another paperwork-only fragment.

**Tech Stack:** Rust, Axum, Tokio, SQLx, PostgreSQL, React, TypeScript, Vite, Tantivy, tree-sitter, git plumbing, background workers, webhook processing, OAuth/OIDC, and repo-local smoke fixtures.

---

## Why this follow-on roadmap exists

The original 85-task parity roadmap is complete as a truthful audit-driven roadmap: Task 85 produced a final matrix-wide parity audit and release checklist, and that audit explicitly concluded the rewrite is **not yet ready** for a full-parity release claim. The remaining work is now concentrated in a smaller set of substantive blockers:

1. durable metadata and migration parity
2. real indexing/search depth beyond the bounded API/UI baseline
3. browse/source/commit/code-nav polish beyond the bounded shell
4. ask/chat lifecycle parity
5. richer auth/admin lifecycle parity plus real external-provider identity flows
6. provider/runtime parity across the supported host matrix
7. MCP runtime/auth parity
8. worker scheduling/retries/supervision parity
9. operator/runtime parity beyond the current local stub-backed baseline
10. one final release-candidate smoke + parity re-audit closure

This roadmap narrows the remaining work to those blockers so future runs can resume truthfully from one state file without pretending the original roadmap still contains unfinished in-file tasks.

---

## Follow-on definition of done

This roadmap is complete only when all of the following are true:

1. Durable metadata, migrations, and startup compatibility are the default truthful baseline for catalog, auth, org, ask-thread, and review-agent state.
2. Search, browse, commit, and code-navigation flows are backed by real index/runtime behavior rather than only bounded local shells.
3. Ask/chat, auth/admin, provider, and MCP surfaces support the lifecycle behavior needed for practical Sourcebot interchangeability.
4. Worker and operator flows include real scheduling, retry, supervision, observability, and recovery behavior.
5. A final release-candidate smoke matrix and parity re-audit can honestly mark the rewrite as a practical Sourcebot replacement.

---

## Execution rules for the follow-on roadmap

- `docs/status/roadmap-state.yaml` remains the single source of truth.
- Execute exactly one **smallest meaningful closure** per run.
- Prefer vertical slices that end in user-visible behavior, operator-visible behavior, or real implementation + focused verification + truthful docs.
- Do not default to audit-then-patch or test-only/docs-only micro-slices when one bounded implementation-backed closure is still realistic.
- When a task is too large, split it toward a shippable vertical slice inside the same blocker domain.
- Stop after one closure completes its full loop: implementation/docs/tests/review/commit/state/push.

---

## Phase A — Durable metadata and migration backbone

### Task 86: SQL metadata schema and migration harness
Introduce the durable PostgreSQL schema, migration commands, dev bootstrap path, and compatibility checks needed to make durable metadata the default roadmap direction instead of a deferred aspiration.

### Task 87: Durable catalog, auth, and organization state
Move catalog, bootstrap/local-session, API-key, OAuth-client, membership, invite, and organization aggregates onto durable metadata with explicit fallback/upgrade behavior for local development.

### Task 88: Durable ask-thread and review-agent persistence
Move ask threads/messages, review-agent runs, delivery attempts, and related status transitions onto durable metadata with concurrency-safe writes and focused migration coverage.

### Task 89: Migration, backward-compatibility, and fail-closed recovery smokes
Prove startup, empty-state bootstrap, local-dev migration, and failure recovery behavior across the newly durable metadata surfaces so later feature slices can build on a truthful persisted baseline.

---

## Phase B — Search, browse, and ask lifecycle parity

### Task 90: Real indexing pipeline and index-status baseline
Replace the current bounded filesystem search baseline with a real indexing pipeline plus truthful index-status state that later search/browse/code-nav surfaces can depend on.

### Task 91: Search contract hardening and saved-context lifecycle
Close the remaining search contract gaps around filter grammar, pagination/relevance guarantees, saved search contexts, and user-visible/index-aware status handling.

### Task 92: Browse/source/commit/code-navigation parity polish
Finish the remaining bounded-shell gaps across syntax highlighting, large/binary-file handling, richer commit UX, branch/revision behavior, and multi-language code-navigation correctness.

### Task 93: Ask/chat lifecycle parity
Finish thread rename/delete/visibility lifecycle, richer ask/chat UX, source-preview behavior, and the remaining API/runtime gaps needed for practical ask/chat parity.

---

## Phase C — Auth, admin, and provider parity

### Task 94: Auth/admin lifecycle parity on durable state
Close the remaining admin lifecycle gaps across onboarding, org/member/invite management, API-key lifecycle, connection/repo status management, analytics/audit surfaces, and durable permission-sync behavior.

### Task 95: External-provider sign-in and linked-account parity
Implement real OIDC/SSO login and callback exchange, account linking/mapping, and the fail-closed identity flows needed so `#/auth` and linked-account surfaces stop being callback-status placeholders.

### Task 96: Provider connection runtime parity
Ship truthful provider/runtime flows for GitHub, GitLab, and generic/local Git host import/sync behavior, then close the remaining Gitea/Gerrit/Bitbucket/Azure DevOps parity through the same durable connection/runtime model.

---

## Phase D — Runtime, interoperability, and release closure

### Task 97: MCP runtime, auth, and end-to-end contract parity
Add the missing MCP transport/runtime/auth wiring plus permission-scoped end-to-end verification so the shipped MCP surface is more than a manifest-and-tool-definition baseline.

### Task 98: Worker scheduling, retries, and supervision parity
Replace the current one-shot/stub-oriented worker baseline with truthful scheduling, retry, claim/recovery, and supervisor-visible runtime behavior.

### Task 99: Operator runtime, observability, and recovery parity
Close the remaining operator/runtime gaps around readiness, durable deployment/upgrade safety, richer observability, backup/recovery, and production-grade local-to-real deployment guidance.

### Task 100: Release-candidate smoke matrix and parity re-audit
Run the final end-to-end release-candidate smoke matrix, update the parity audit from blocker-driven to release-verdict evidence, and either (a) truthfully certify practical Sourcebot replacement parity or (b) fail closed with a new explicitly scoped roadmap.

---

## Initial execution priority

The first meaningful closure after creating this roadmap should target **Task 86**. Durable schema/migration groundwork unlocks multiple blocked domains at once — auth/admin, org state, ask/chat durability, review-agent persistence, and later operator recovery — and is therefore a higher-value next slice than another docs-only or audit-only follow-up.
