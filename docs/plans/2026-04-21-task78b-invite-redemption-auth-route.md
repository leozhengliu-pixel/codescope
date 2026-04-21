# Task 78b Invite Redemption Auth Route Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Extend `#/auth` with one real invite-redemption flow so an invited email can finish account setup, gain membership, and land in an authenticated session.

**Architecture:** Keep the slice bounded to the existing file-backed auth/org stores and current hash-routed React shell. Add one backend redeem endpoint that consumes an invite id, validates the invite/email state, creates or reuses the invited local account plus membership, marks the invite accepted, issues a local session, and lets the auth route switch between bootstrap, local login, and redeem flows.

**Tech Stack:** Rust + Axum backend, React + TypeScript frontend, Vitest, cargo test.

---

### Task 1: Add failing backend tests for invite redemption

**Objective:** Lock the API contract for redeeming a pending invite into a local account + membership + session.

**Files:**
- Modify: `crates/api/src/main.rs`
- Test: `crates/api/src/main.rs`

**Step 1: Write failing tests**
- Add one success test covering `POST /api/v1/auth/invite-redeem` with invite id, invited email, name, and password.
- Add one fail-closed test proving already-accepted or mismatched invites do not redeem.

**Step 2: Run test to verify failure**
Run: `cargo test -p sourcebot-api invite_redeem -- --nocapture`
Expected: FAIL because the route/handler does not exist yet.

**Step 3: Write minimal implementation**
- Add request/response structs.
- Add route wiring.
- Validate invite state, create or reuse the invited account, add membership if missing, mark invite accepted, persist state, and create a local session.

**Step 4: Run backend tests to verify pass**
Run: `cargo test -p sourcebot-api invite_redeem -- --nocapture`
Expected: PASS.

**Step 5: Commit**
`git add crates/api/src/main.rs && git commit -m "feat: add invite redemption auth endpoint"`

### Task 2: Add failing frontend tests for the auth-route redeem flow

**Objective:** Lock the user-visible `#/auth` invite redemption flow and its session bootstrap behavior.

**Files:**
- Modify: `web/src/App.test.tsx`
- Modify: `web/src/App.tsx`

**Step 1: Write failing test**
- Add a test that opens `#/auth?invite=...&email=...`, renders an invite redemption form, submits it to `/api/v1/auth/invite-redeem`, stores the returned local session, reloads `/api/v1/auth/me`, and renders the signed-in state.
- Add a focused failure-state assertion if the redeem request rejects.

**Step 2: Run test to verify failure**
Run: `cd web && npx vitest run src/App.test.tsx -t "invite redemption"`
Expected: FAIL because the route state/UI are missing.

**Step 3: Write minimal implementation**
- Extend auth-route parsing/state to understand invite query parameters.
- Render a redeem form when invite context is present and bootstrap is not required.
- Submit the redeem request, store the issued session, restore identity, and show truthful loading/error copy.

**Step 4: Run frontend tests to verify pass**
Run: `cd web && npx vitest run src/App.test.tsx -t "invite redemption"`
Expected: PASS.

**Step 5: Commit**
`git add web/src/App.tsx web/src/App.test.tsx && git commit -m "feat: add auth invite redemption flow"`

### Task 3: Update acceptance/report wording for the shipped slice

**Objective:** Ground only the newly shipped invite-redemption baseline without overclaiming invite creation or broader org management parity.

**Files:**
- Modify: `specs/acceptance/auth.md`
- Modify: `specs/acceptance/index.md`
- Modify: `docs/reports/2026-04-18-parity-gap-report.md`
- Modify: `docs/status/roadmap-state.yaml`

**Step 1: Patch docs after behavior is green**
- Update auth acceptance examples so they mention the new `#/auth` invite redemption baseline.
- Keep invite creation/admin CRUD and external provider auth explicitly out of scope.

**Step 2: Run focused doc truthfulness checks**
- Raw-text checks for the updated wording.
- `git diff --check`

**Step 3: Commit after review/state update if appropriate**
Use one substantive commit for product/docs, then a truthful roadmap-state commit only if separate state bookkeeping is warranted.
