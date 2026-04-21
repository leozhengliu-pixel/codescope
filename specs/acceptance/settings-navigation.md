# Acceptance Spec: Settings Navigation Shell

## Purpose
This document defines the current clean-room acceptance contract for the authenticated settings navigation shell in `web/src/App.tsx`. It covers discoverability plus the currently shipped subsection baselines: users should be able to open a general `#/settings` landing page, discover the currently exposed authenticated admin API surfaces, navigate into a shared settings shell for each shipped subsection, use the minimal API-key inventory/revoke panel, inspect a minimal read-only members panel for admin-visible organization member and invite inventory, inspect a minimal read-only access panel for currently visible repositories, inspect a minimal read-only OAuth-client inventory panel, inspect a minimal observability panel for authenticated audit/analytics visibility, and inspect a minimal review-automation visibility panel for authenticated review-webhook, delivery-attempt, and review-agent-run inventories without overstating richer admin UX that is still follow-up work.

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
- `#/settings/members`
- `#/settings/access`
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
   - Members
   - Access
   - OAuth clients
   - Audit & analytics
   - Review automation
4. The shell should make clear that these routes are discoverability and navigation work, not a claim of fully complete management UX.

### 2. Shared subsection shell
1. Each shipped settings subsection route should render inside the same shared settings navigation shell.
2. The currently selected subsection should remain visually identifiable within the shared shell.
3. The connections route should preserve the existing authenticated connection-management experience instead of regressing into a dead placeholder.
4. The API keys route should render a real authenticated inventory panel instead of a dead placeholder.
5. The members route should render a real authenticated read-only inventory panel instead of a dead placeholder.
6. The access route should render a real authenticated read-only repository-visibility panel instead of a dead placeholder.
7. The OAuth clients route should render a real authenticated read-only inventory panel instead of a dead placeholder.
8. The observability route should render a real authenticated audit/analytics panel instead of a dead placeholder.
9. The review-automation route should render a real authenticated visibility panel instead of a dead placeholder.

### 3. Honest grounding in existing authenticated API surfaces
1. The API keys subsection should fetch `GET /api/v1/auth/api-keys`, show loading/error/empty/populated states truthfully, and surface a focused revoke control through `POST /api/v1/auth/api-keys/{api_key_id}/revoke` for active keys only.
2. The members subsection should fetch `GET /api/v1/auth/members`, show loading/error/empty/populated states truthfully, render only organizations the authenticated local-session user administers, join member roster entries to visible account identity fields, and show invite rows with truthful accepted-vs-pending status without exposing hidden organizations, hidden accounts, or invite-management controls that do not exist yet.
3. The access subsection should fetch `GET /api/v1/repos`, show loading/error/empty/populated states truthfully, render only repositories already visible to the authenticated user, and stay explicit that the route is a read-only visibility baseline rather than shipped permission-sync management or role-edit UX.
4. The OAuth clients subsection should fetch `GET /api/v1/auth/oauth-clients`, show loading/error/empty/populated states truthfully, surface only the visible inventory fields returned by the backend (`name`, `client_id`, `organization_id`, `created_by_user_id`, `created_at`, `revoked_at`, and `redirect_uris`), and must not expose any secret hash or plaintext secret material.
5. The observability subsection should fetch `GET /api/v1/auth/audit-events` and `GET /api/v1/auth/analytics`, show loading/error/empty/populated states truthfully for each dataset, and present the returned audit-event / analytics-record details without claiming filtering/export workflows that do not exist yet.
6. The review-automation subsection should fetch `GET /api/v1/auth/review-webhooks`, `GET /api/v1/auth/review-webhook-delivery-attempts`, and `GET /api/v1/auth/review-agent-runs`, show loading/error/empty/populated states truthfully for each dataset, present the returned visibility fields without exposing secret hashes or secret material, and stay honest that webhook management/retry/run control UX remains follow-up work.
7. Repo-scope copy must stay truthful: empty scope means the key is not repo-bound and can reach the repos currently visible to the authenticated user, not arbitrary hidden repositories.
8. Grounding for each current non-connections section is:
   - API keys → authenticated inventory from `/api/v1/auth/api-keys` plus revoke through `/api/v1/auth/api-keys/{api_key_id}/revoke`
   - Members → authenticated admin-visible inventory from `/api/v1/auth/members`
   - Access → authenticated read-only visible-repository inventory from `/api/v1/repos`
   - OAuth clients → authenticated read-only inventory from `/api/v1/auth/oauth-clients`
   - Audit & analytics / observability → authenticated inventory from `/api/v1/auth/audit-events` and `/api/v1/auth/analytics`
   - Review automation → authenticated inventory from `/api/v1/auth/review-webhooks`, `/api/v1/auth/review-webhook-delivery-attempts`, and `/api/v1/auth/review-agent-runs`

### 4. Boundaries and non-goals
1. This slice does **not** claim shipped onboarding, login, org membership, invite, or linked-account UX.
2. This slice does **not** claim CRUD-complete UX for API keys, members, access/permission-sync management, OAuth clients, audit analytics filtering/export, or review automation; API keys currently have only a minimal inventory plus revoke panel, members currently exposes only a read-only roster/invite inventory, access currently exposes only a read-only visible-repository inventory, and observability currently has only a minimal read-only inventory panel.
3. Aside from the focused authenticated members inventory route and the read-only access visibility route, this settings shell should not be read as a claim of broader new backend admin surface area or full org/invite/permission-sync management parity.

## Minimum verification evidence
1. Focused frontend tests prove:
   - the settings landing page renders
   - the API-key subsection renders authenticated inventory details and revoke behavior inside the shared shell
   - the members subsection renders authenticated organization member and invite inventory details inside the shared shell
   - the access subsection renders authenticated visible-repository inventory details plus the truthful empty state inside the shared shell
   - the OAuth-clients subsection renders authenticated inventory details plus empty/error states inside the shared shell without exposing secret material
   - the observability subsection renders authenticated audit/analytics inventory details and endpoint-scoped failure states inside the shared shell
   - the review-automation subsection renders authenticated review-webhook, delivery-attempt, and review-agent-run visibility details plus endpoint-scoped failure states inside the shared shell without exposing secret material
   - the existing connections route remains reachable within the shared shell
2. Broader `web/src/App.test.tsx` coverage still passes after the route-family change.
3. Production build still passes after the navigation-shell change.
