# Task 79b Chat Route Thread History Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Ship one bounded Task 79 user-visible closure by adding a dedicated `#/chat` route that can list, reopen, and continue authenticated ask threads instead of leaving thread history entirely deferred.

**Architecture:** Reuse the existing ask-thread store and ask-completions backend instead of inventing a separate chat model. Add two authenticated thread-read APIs (`GET /api/v1/ask/threads`, `GET /api/v1/ask/threads/{thread_id}`), then build a focused frontend chat route that loads thread history, keeps hash-restored thread selection truthful, and continues a selected thread with the existing completions endpoint.

**Tech Stack:** Rust, Axum, sourcebot ask-thread store, React, TypeScript, Vitest.

---

### Task 1: Add authenticated ask-thread read APIs

**Objective:** Expose one focused backend contract for listing a caller's threads and loading a selected thread.

**Files:**
- Modify: `crates/api/src/main.rs`
- Test: `crates/api/src/main.rs` (existing API tests)

**Step 1: Write failing tests**
- Add one test proving `GET /api/v1/ask/threads` returns only the authenticated caller's thread summaries.
- Add one test proving `GET /api/v1/ask/threads/{thread_id}` returns the caller-owned thread with messages and rejects hidden/missing thread ids with `404`.

**Step 2: Run targeted tests to verify RED**
- Run: `cargo test -p sourcebot-api ask_threads_ -- --nocapture`
- Expected: FAIL because the routes do not exist yet.

**Step 3: Write minimal implementation**
- Add the two routes.
- Reuse `ask_request_context` for auth/scope grounding.
- Reuse `AskThreadSummary` / `AskThreadResponse` conversions from `crates/api/src/ask.rs`.

**Step 4: Run targeted tests to verify GREEN**
- Run: `cargo test -p sourcebot-api ask_threads_ -- --nocapture`
- Expected: PASS.

### Task 2: Add the dedicated chat route with thread history and continuation

**Objective:** Turn Task 79 from ask-only into ask+chat by shipping a dedicated `#/chat` surface that can browse and continue thread history.

**Files:**
- Modify: `web/src/App.tsx`
- Test: `web/src/App.test.tsx`

**Step 1: Write failing tests**
- Add one test proving `#/chat` loads thread summaries, opens the selected thread from the hash, renders prior messages, and keeps the top-level route on chat instead of ask/home.
- Add one test proving submitting from `#/chat` continues the selected thread and appends the new answer while keeping the hash pinned to the selected thread.

**Step 2: Run targeted tests to verify RED**
- Run: `npx vitest run src/App.test.tsx -t "chat route"`
- Expected: FAIL because the route and thread-history UI do not exist yet.

**Step 3: Write minimal implementation**
- Add a `#/chat` route parser and top-level nav link.
- Add frontend types for ask-thread summaries/details.
- Fetch `/api/v1/ask/threads` and the selected thread detail.
- Let the user start a fresh thread or continue a selected one with `/api/v1/ask/completions`.
- Keep repo-scope and thread hash state truthful when switching threads or starting a fresh chat.

**Step 4: Run targeted tests to verify GREEN**
- Run: `npx vitest run src/App.test.tsx -t "chat route"`
- Expected: PASS.

### Task 3: Ground the shipped baseline and close the slice

**Objective:** Update acceptance/report/state truthfully for the new chat-route baseline and close the run.

**Files:**
- Modify: `specs/acceptance/ask.md`
- Modify: `specs/acceptance/index.md`
- Modify: `docs/reports/2026-04-18-parity-gap-report.md`
- Modify: `docs/status/roadmap-state.yaml`

**Step 1: Update docs narrowly**
- Change the ask acceptance doc from ask-only baseline to ask+chat baseline.
- Keep rename/delete/visibility/agents work explicitly deferred.

**Step 2: Verify raw-content truthfulness**
- Use exact text checks for the new chat-route wording and the still-deferred follow-up scope.

**Step 3: Run broader validation**
- Run: `cargo test -p sourcebot-api`
- Run: `npx vitest run src/App.test.tsx`
- Run: `NODE_OPTIONS=--max-old-space-size=4096 npm run build`
- Run: `git diff --check`

**Step 4: Independent review, commit, state update, push**
- Run the requesting-code-review pipeline.
- Commit the substantive slice.
- Update `docs/status/roadmap-state.yaml` to record the shipped Task 79b closure and select the next meaningful Task 79 follow-up.
- Push and stop.
