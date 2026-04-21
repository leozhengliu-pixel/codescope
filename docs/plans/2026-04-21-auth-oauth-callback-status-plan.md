# Auth OAuth Callback Status Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Add one bounded `#/auth` OAuth-facing callback-status closure that truthfully handles provider redirects without overclaiming full external-provider sign-in.

**Architecture:** Extend the hash-route parser so `#/auth` can carry focused OAuth callback context, then teach `AuthPage` to render a provider-aware callback status card above the existing local-login flow. Keep the slice frontend-only and explicitly truthful: callback parameters produce clear user guidance while local login remains available and broader external-provider login/callback exchange stays follow-up work.

**Tech Stack:** React, TypeScript, Vitest, Testing Library, Vite markdown acceptance docs.

---

### Task 1: Add failing auth-route tests for OAuth callback status

**Objective:** Lock the desired `#/auth` callback behavior before implementation.

**Files:**
- Modify: `web/src/App.test.tsx`
- Test: `web/src/App.test.tsx`

**Step 1: Write failing test**
- Add one test covering an OAuth callback error hash such as `#/auth?provider=github&error=access_denied&error_description=Org%20SSO%20required`.
- Add one test covering an OAuth callback code/state hash such as `#/auth?provider=github&code=callback-code&state=opaque-state`.
- Assert the route still shows the local login form, plus a truthful callback-status callout that names the provider and distinguishes error vs callback-received states.

**Step 2: Run test to verify failure**
Run: `npx vitest run src/App.test.tsx -t "oauth callback"`
Expected: FAIL because the current `#/auth` route ignores those params and renders no callback-status content.

**Step 3: Commit**
Do not commit yet; proceed to Task 2 after the red test is observed.

### Task 2: Implement minimal auth-route callback-status UI

**Objective:** Parse focused OAuth callback params and render a truthful status card without claiming working external sign-in.

**Files:**
- Modify: `web/src/App.tsx`
- Modify: `web/src/App.test.tsx`

**Step 1: Write minimal implementation**
- Extend the `#/auth` route parser to capture `provider`, `error`, `error_description`, `code`, and `state` query params.
- Extend `AuthPage` props/state derivation to compute whether an OAuth callback context is present.
- Render a compact callback-status section above the existing local login form when callback params are present.
- For error callbacks, show provider-aware failure copy and surface the OAuth error code/description when present.
- For code/state callbacks, show a truthful “callback received but external sign-in is not wired here yet” notice while keeping local login available.

**Step 2: Run targeted test to verify pass**
Run: `npx vitest run src/App.test.tsx -t "oauth callback"`
Expected: PASS with only the intended callback tests matching.

**Step 3: Run broader frontend verification**
Run: `npx vitest run src/App.test.tsx`
Expected: PASS

Run: `NODE_OPTIONS=--max-old-space-size=4096 npm run build`
Expected: PASS

### Task 3: Ground acceptance/docs for the narrower auth callback baseline

**Objective:** Update the auth acceptance/report wording so it truthfully reflects the shipped callback-status slice.

**Files:**
- Modify: `specs/acceptance/auth.md`
- Modify: `specs/acceptance/index.md`
- Modify: `docs/reports/2026-04-18-parity-gap-report.md`

**Step 1: Patch the docs minimally**
- Add one auth acceptance bullet describing the new `#/auth` callback-status behavior.
- Update the auth/admin inventory wording in `specs/acceptance/index.md` so it mentions callback-status handling but still keeps external-provider sign-in/callback exchange out of scope.
- Update the parity gap report auth-domain wording to reflect that the auth route now handles provider callback states truthfully while external-provider login remains partial/missing.

**Step 2: Verify raw text**
Run exact raw-content checks (e.g. `python3` assertions or `grep`-equivalent Hermes file checks) confirming the new callback-status wording appears in each edited doc.

**Step 3: Final hygiene**
Run: `git diff --check`
Expected: PASS
