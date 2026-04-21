# Settings Navigation Shell Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Add a broader auth/admin settings route family so users can discover the existing connections page and the already-exposed admin API surfaces from a single settings shell.

**Architecture:** Extend `web/src/App.tsx` hash routing with a settings landing page plus small section shells for existing admin surfaces, then ground the new UI with focused frontend tests and a dedicated settings-navigation acceptance spec. Keep scope to discoverability and route-shell parity only; do not implement new backend CRUD flows.

**Tech Stack:** React, TypeScript, Vitest, Testing Library, markdown acceptance docs.

---

### Task 1: Add route-level tests for the settings landing page and section navigation

**Objective:** Prove the app exposes a broader settings shell instead of only a deep-linked connections page.

**Files:**
- Modify: `web/src/App.test.tsx`
- Verify: `web/src/App.tsx`

**Step 1: Write failing tests**
- Add one test for `#/settings` that expects a settings landing page with links/cards for Connections, API keys, OAuth clients, Audit & analytics, and Review automation.
- Add one test for a non-connections settings route (for example `#/settings/api-keys`) that expects the shared settings navigation shell plus a focused placeholder panel grounded in the existing authenticated API surface.

**Step 2: Run targeted tests to verify RED**
Run: `npx vitest run src/App.test.tsx -t "settings landing"`
Expected: FAIL because the route does not exist yet.

Run: `npx vitest run src/App.test.tsx -t "api keys settings section"`
Expected: FAIL because the route does not exist yet.

### Task 2: Implement the settings route family in the app shell

**Objective:** Add the smallest user-visible settings navigation shell that meaningfully expands discoverability.

**Files:**
- Modify: `web/src/App.tsx`

**Step 1: Add minimal implementation**
- Extend the hash router to support `#/settings` and a small set of subsection routes.
- Replace the header deep link with a general Settings link.
- Add a shared settings-shell navigation component used by the landing page, the existing connections page, and the new subsection placeholders.
- Keep non-connections sections honest: describe the existing authenticated API surfaces and explicitly note that richer management flows remain follow-up work.

**Step 2: Run targeted tests to verify GREEN**
Run: `npx vitest run src/App.test.tsx -t "settings landing|api keys settings section"`
Expected: PASS

### Task 3: Ground the new shell in acceptance docs

**Objective:** Give the broader settings shell its own acceptance home and update the surface index/journey map to reference it.

**Files:**
- Create: `specs/acceptance/settings-navigation.md`
- Modify: `specs/acceptance/index.md`
- Modify: `specs/acceptance/journeys.md`
- Modify: `docs/reports/2026-04-18-parity-gap-report.md`

**Step 1: Write the acceptance doc**
- Describe the current route family, discoverability goals, admin/user boundaries, and explicit non-goals.

**Step 2: Update index/journey/report grounding**
- Mark settings navigation as having a dedicated acceptance home.
- Update the auth/admin/settings frontend shells gap-report row from “missing” to “partial” if the new route family truthfully justifies that status.

**Step 3: Verify docs**
Run targeted raw-content checks confirming the new settings-navigation acceptance doc and updated references exist.

### Task 4: Verify the whole slice and prepare for review/commit

**Objective:** Confirm the new settings shell works and docs match the shipped behavior.

**Files:**
- Modify: `web/src/App.tsx`
- Modify: `web/src/App.test.tsx`
- Modify/create docs above

**Step 1: Run focused frontend verification**
Run: `npx vitest run src/App.test.tsx -t "settings landing|api keys settings section|Authenticated connections"`
Expected: PASS

**Step 2: Run broader confidence checks**
Run: `npx vitest run src/App.test.tsx`
Expected: PASS

Run: `NODE_OPTIONS=--max-old-space-size=4096 npm run build`
Expected: PASS

**Step 3: Commit**
Use a commit message like:
`feat: add settings navigation shell`
