# Acceptance Spec: Settings Navigation Shell

## Purpose
This document defines the current clean-room acceptance contract for the authenticated settings navigation shell in `web/src/App.tsx`. It covers discoverability and route-shell behavior only: users should be able to open a general `#/settings` landing page, discover the currently exposed authenticated admin API surfaces, and navigate into a shared settings shell for each shipped subsection.

## Grounding
- Frontend route shell: `web/src/App.tsx`
- Focused frontend verification: `web/src/App.test.tsx`
- Related auth/admin baseline: `specs/acceptance/auth.md`
- Surface inventory and journey map: `specs/acceptance/index.md`, `specs/acceptance/journeys.md`

## Current route family
The shipped settings shell currently covers these hash routes:
- `#/settings`
- `#/settings/connections`
- `#/settings/api-keys`
- `#/settings/oauth-clients`
- `#/settings/observability`
- `#/settings/review-automation`

## Acceptance requirements

### 1. Shared entry and discoverability
1. The top app header should expose a general `Settings` link rather than a deep link to one subsection.
2. Opening `#/settings` should render a dedicated landing page within a shared settings shell.
3. The shared shell should expose navigation cards/links for the currently shipped settings sections:
   - Connections
   - API keys
   - OAuth clients
   - Audit & analytics
   - Review automation
4. The shell should make clear that these routes are discoverability and navigation work, not a claim of fully complete management UX.

### 2. Shared subsection shell
1. Each shipped settings subsection route should render inside the same shared settings navigation shell.
2. The currently selected subsection should remain visually identifiable within the shared shell.
3. The connections route should preserve the existing authenticated connection-management experience instead of regressing into a dead placeholder.

### 3. Honest grounding in existing authenticated API surfaces
1. Non-connections subsections may be placeholders, but they must explicitly point to already exposed authenticated API surfaces.
2. The placeholder copy must stay honest about scope and state that richer management UX is follow-up work.
3. Grounding for each current placeholder section is:
   - API keys → `/api/v1/auth/api-keys` and revoke
   - OAuth clients → `/api/v1/auth/oauth-clients`
   - Audit & analytics / observability → `/api/v1/auth/audit-events`, `/api/v1/auth/analytics`
   - Review automation → authenticated review-webhook, delivery-attempt, and review-agent-run visibility endpoints

### 4. Boundaries and non-goals
1. This slice does **not** claim shipped onboarding, login, org membership, invite, or linked-account UX.
2. This slice does **not** claim CRUD-complete UX for API keys, OAuth clients, audit analytics filtering, or review automation.
3. This slice does **not** add new backend routes; it only improves frontend route-shell discoverability over surfaces that already exist.

## Minimum verification evidence
1. Focused frontend tests prove:
   - the settings landing page renders
   - at least one non-connections subsection route renders inside the shared shell
   - the existing connections route remains reachable within the shared shell
2. Broader `web/src/App.test.tsx` coverage still passes after the route-family change.
3. Production build still passes after the navigation-shell change.
