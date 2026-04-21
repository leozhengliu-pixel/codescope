# Task 79a Ask Route Baseline Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Ship one meaningful Task 79 closure by adding a dedicated `#/ask` route with real ask completions, inline citation rendering, repo-scope controls, and route-restorable thread continuity.

**Architecture:** Keep the current single-file React shell but add a dedicated ask route/page, backed by the existing `/api/v1/ask/completions` API. Extend the ask completion response so frontend callers receive the persisted thread identity needed to continue the same conversation and restore it from the hash route.

**Tech Stack:** Rust/Axum backend, React + TypeScript frontend, Vitest, Rust integration tests.

---

### Task 1: Return persisted thread identity from ask completions

**Objective:** Make `/api/v1/ask/completions` return the thread/session identifiers needed for frontend thread continuity.

**Files:**
- Modify: `crates/api/src/ask.rs`
- Modify: `crates/api/src/main.rs`

**Step 1: Write failing backend test**
- Add/extend an integration test in `crates/api/src/main.rs` asserting a new completion response includes the persisted `thread_id` and `session_id`, both for new-thread creation and append-to-existing-thread cases.

**Step 2: Run test to verify failure**
- Run: `cargo test -p sourcebot-api ask_completions_ -- --nocapture`
- Expected: FAIL because the response does not yet include thread identity.

**Step 3: Write minimal implementation**
- Extend `AskCompletionResponse` to include `thread_id` and `session_id`.
- In `create_ask_completion`, capture the created/appended thread and return its identifiers.

**Step 4: Run backend tests to verify pass**
- Run: `cargo test -p sourcebot-api ask_completions_ -- --nocapture`
- Expected: PASS.

**Step 5: Commit**
- Commit together with the ask-route UI once the full slice is complete.

### Task 2: Add dedicated ask route with repo scope, cited answers, and hash-restored thread continuity

**Objective:** Expose a real `#/ask` page instead of no dedicated ask/chat frontend.

**Files:**
- Modify: `web/src/App.tsx`
- Modify: `web/src/App.test.tsx`

**Step 1: Write failing frontend tests**
- Add focused Vitest coverage for:
  - rendering a top-level Ask route
  - submitting a prompt with selected repo scope
  - rendering answer text plus citations
  - persisting/restoring the current `thread_id` from the hash route for a follow-up ask

**Step 2: Run test to verify failure**
- Run: `npx vitest run src/App.test.tsx -t "dedicated ask route|restores the ask thread from the hash route"`
- Expected: FAIL because `#/ask` does not exist yet.

**Step 3: Write minimal implementation**
- Add ask route parsing/build helpers in `web/src/App.tsx`.
- Add top-level Ask navigation.
- Build `AskPage` using repo inventory + `/api/v1/ask/completions`.
- Keep the route truthful: show loading/error/empty states, current repo scope, returned provider/model, answer body, and inline rendered citations.
- When a response returns `thread_id`, update the hash so a refresh or follow-up on the same route keeps the active thread context.

**Step 4: Run focused frontend verification**
- Run: `npx vitest run src/App.test.tsx -t "dedicated ask route|restores the ask thread from the hash route"`
- Expected: PASS.

**Step 5: Run broader frontend verification**
- Run: `npx vitest run src/App.test.tsx`
- Expected: PASS.

### Task 3: Truthful docs/report updates for the new ask-route baseline

**Objective:** Update acceptance/report wording so the roadmap records the newly shipped ask/chat frontend baseline without overclaiming full thread-management or agents parity.

**Files:**
- Modify: `specs/acceptance/ask.md`
- Modify: `specs/acceptance/index.md`
- Modify: `docs/reports/2026-04-18-parity-gap-report.md`

**Step 1: Patch only the wording justified by the shipped slice**
- Change the ask/chat frontend status from “no dedicated page/component yet” to a narrower partial baseline describing `#/ask`, cited answers, repo scope, and hash-restored active-thread continuity.
- Keep rename/delete/full history/agents parity explicitly deferred.

**Step 2: Verify raw content**
- Use raw-content assertions or targeted greps to confirm the exact updated phrases are present.

**Step 3: Commit**
- Commit together with the substantive implementation after review passes.

### Task 4: Full closure verification and review

**Objective:** Close the slice honestly with tests, review, state update, and push.

**Files:**
- Modify: `docs/status/roadmap-state.yaml`

**Step 1: Run targeted verification**
- `cargo test -p sourcebot-api ask_completions_ -- --nocapture`
- `npx vitest run src/App.test.tsx -t "dedicated ask route|restores the ask thread from the hash route"`

**Step 2: Run broader confidence checks**
- `cargo test -p sourcebot-api`
- `npx vitest run src/App.test.tsx`
- `NODE_OPTIONS=--max-old-space-size=4096 npm run build`
- `git diff --check`
- Python added-lines security scan over the diff

**Step 3: Independent review**
- One spec-compliance review against this plan/slice.
- One code-quality/security review against the diff.

**Step 4: Commit and state update**
- Substantive commit: `git commit -m "[verified] feat: add ask route baseline"`
- Truthful state update commit only if justified after the substantive commit.

**Step 5: Push**
- Push the completed closure, then stop.
