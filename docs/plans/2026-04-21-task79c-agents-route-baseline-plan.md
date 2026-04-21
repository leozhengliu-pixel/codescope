# Task 79c Agents Route Baseline Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Ship a dedicated `#/agents` route with truthful operator-visible review-agent run navigation on top of the existing authenticated visibility APIs.

**Architecture:** Reuse the already-shipped authenticated review automation APIs, but move the operator journey onto a dedicated route instead of keeping it buried inside settings. The route should restore a selected run from the hash, show the related webhook and delivery-attempt context, fail closed when a restored run is missing, and keep the settings review-automation surface as a narrower admin inventory.

**Tech Stack:** React, TypeScript, Vitest, Testing Library, existing hash-router in `web/src/App.tsx`

---

### Task 1: Add failing route-level regression for dedicated agents baseline

**Objective:** Prove the desired user-visible behavior before implementation.

**Files:**
- Modify: `web/src/App.test.tsx`
- Inspect: `web/src/App.tsx`

**Step 1: Write failing test**
- Add one focused test that starts at `#/agents?run_id=run-visible`.
- Mock:
  - `GET /api/v1/auth/review-agent-runs`
  - `GET /api/v1/auth/review-agent-runs/run-visible`
  - `GET /api/v1/auth/review-webhook-delivery-attempts/delivery-visible`
  - `GET /api/v1/auth/review-webhooks/review-webhook-visible`
- Assert:
  - top navigation includes an `Agents` link to `#/agents`
  - dedicated route heading/subtitle render
  - restored run selection loads the matching detail
  - operator-visible related webhook + delivery-attempt cards render on the same page
  - hash stays pinned to `#/agents?run_id=run-visible`
  - settings shell is not rendered for this route

**Step 2: Run test to verify failure**
Run: `npx vitest run src/App.test.tsx -t "dedicated agents route"`
Expected: FAIL because `#/agents` does not exist yet.

**Step 3: Commit nothing yet**
- Keep the tree dirty for implementation.

### Task 2: Implement the dedicated agents route baseline

**Objective:** Add the smallest meaningful operator-visible closure in one vertical slice.

**Files:**
- Modify: `web/src/App.tsx`
- Test: `web/src/App.test.tsx`

**Step 1: Add route parsing and top-level navigation**
- Introduce `#/agents` hash parsing plus optional `run_id` restoration.
- Add an `Agents` top-nav link without regressing existing routes.

**Step 2: Implement `AgentsPage`**
- Fetch visible run summaries from `/api/v1/auth/review-agent-runs`.
- Restore a selected run from the hash.
- Load the selected run detail from `/api/v1/auth/review-agent-runs/{run_id}`.
- Use the returned `webhook_id` and `delivery_attempt_id` to load the related resources from their detail endpoints.
- Show truthful loading, empty, error, and populated states.
- If a restored run fails with 404, clear the selection and reset the hash to `#/agents`.

**Step 3: Re-run the focused test**
Run: `npx vitest run src/App.test.tsx -t "dedicated agents route"`
Expected: PASS.

**Step 4: Run a broader frontend confidence signal**
Run: `npx vitest run src/App.test.tsx -t "review automation|dedicated agents route"`
Expected: PASS for both the new route and the pre-existing settings surface.

### Task 3: Ground the shipped slice truthfully in acceptance/report docs

**Objective:** Update docs only after the behavior exists and tests pass.

**Files:**
- Modify: `specs/acceptance/ask.md`
- Modify: `specs/acceptance/index.md`
- Modify: `docs/reports/2026-04-18-parity-gap-report.md`

**Step 1: Narrowly update acceptance wording**
- Expand ask/chat acceptance to include the dedicated `#/agents` baseline only.
- Keep richer agent management, retries, and broader automation orchestration explicitly deferred.

**Step 2: Update the acceptance index + parity gap report**
- Mention the dedicated `#/agents` route in the relevant frontend/API summaries.
- Do not overclaim worker orchestration or full automation parity.

**Step 3: Verify raw text truthfulness**
Run: `git diff -- specs/acceptance/ask.md specs/acceptance/index.md docs/reports/2026-04-18-parity-gap-report.md`
Expected: only scoped wording for the shipped route baseline.

### Task 4: Final verification and closure

**Objective:** Prove the slice is safe to land.

**Files:**
- Verify current diff only

**Step 1: Run the route-focused frontend suite**
Run: `npx vitest run src/App.test.tsx -t "dedicated agents route|review automation"`
Expected: PASS.

**Step 2: Run the full frontend suite**
Run: `npx vitest run src/App.test.tsx`
Expected: PASS.

**Step 3: Run build and mechanical checks**
Run: `NODE_OPTIONS=--max-old-space-size=4096 npm run build`
Expected: PASS.
Run: `git diff --check`
Expected: no output.

**Step 4: Commit after independent review**
```bash
git add web/src/App.tsx web/src/App.test.tsx specs/acceptance/ask.md specs/acceptance/index.md docs/reports/2026-04-18-parity-gap-report.md docs/plans/2026-04-21-task79c-agents-route-baseline-plan.md
git commit -m "feat: add agents route baseline"
```
