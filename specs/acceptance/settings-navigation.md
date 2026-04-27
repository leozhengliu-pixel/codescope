# Acceptance Spec: Settings Navigation Shell

## Purpose
This document defines the current clean-room acceptance contract for the authenticated settings navigation shell in `web/src/App.tsx`. It covers discoverability plus the currently shipped subsection baselines: users should be able to open a general `#/settings` landing page, discover the currently exposed authenticated admin API surfaces, navigate into a shared settings shell for each shipped subsection, use the minimal API-key inventory/create/revoke panel, inspect a minimal members panel for admin-visible organization member and invite inventory plus bounded pending-invite creation backed by the configured auth metadata store, inspect a minimal read-only access panel for currently visible repositories, inspect a minimal read-only linked-accounts panel for the current local account identity, same-user external identities, and visible organization memberships, use a minimal OAuth-client inventory-plus-create panel, inspect a minimal observability panel for authenticated audit/analytics visibility, and inspect a minimal review-automation visibility panel for authenticated review-webhook, delivery-attempt, and review-agent-run inventories without overstating richer admin UX that is still follow-up work.

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
- `#/settings/linked-accounts`
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
   - Linked accounts
   - OAuth clients
   - Audit & analytics
   - Review automation
4. The shell should make clear that these routes are discoverability and navigation work, not a claim of fully complete management UX.

### 2. Shared subsection shell
1. Each shipped settings subsection route should render inside the same shared settings navigation shell.
2. The currently selected subsection should remain visually identifiable within the shared shell.
3. The connections route should preserve the existing authenticated connection-management experience instead of regressing into a dead placeholder.
4. The API keys route should render a real authenticated inventory panel instead of a dead placeholder.
5. The members route should render a real authenticated inventory-plus-invite-create/cancel panel instead of a dead placeholder.
6. The access route should render a real authenticated read-only repository-visibility panel instead of a dead placeholder.
7. The linked-accounts route should render a real authenticated read-only identity-and-membership panel instead of a dead placeholder.
8. The OAuth clients route should render a real authenticated inventory-plus-create panel instead of a dead placeholder.
9. The observability route should render a real authenticated audit/analytics panel instead of a dead placeholder.
10. The review-automation route should render a real authenticated visibility panel instead of a dead placeholder.

### 3. Honest grounding in existing authenticated API surfaces
1. The API keys subsection should fetch `GET /api/v1/auth/api-keys`, show loading/error/empty/populated states truthfully, expose a minimal authenticated create form that POSTs name plus optional newline-delimited repository scope to `/api/v1/auth/api-keys`, append the newly created key in the visible inventory, reveal the returned plaintext `secret` only in the immediate creation-success area, and surface a focused revoke control through `POST /api/v1/auth/api-keys/{api_key_id}/revoke` for active keys only.
2. The members subsection should fetch `GET /api/v1/auth/members`, show loading/error/empty/populated states truthfully, render only organizations the authenticated local-session user administers, join member roster entries to visible account identity fields, show invite rows with truthful accepted-vs-pending status, expose a minimal authenticated create form that POSTs an administered `organization_id`, trimmed email, and role to `POST /api/v1/auth/members/invites`, expose a pending-invite cancel control that calls `DELETE /api/v1/auth/members/invites/{invite_id}` only for pending invites, expose bounded member-role selects that call `PATCH /api/v1/auth/members/{user_id}/role` for visible members in administered organizations, and expose a bounded member-removal control that calls `DELETE /api/v1/auth/members/{user_id}` with `organization_id` for visible active members without deleting the local account. The backend must reject self-removal plus last-admin removal/demotion without mutation. It must not expose hidden organizations, hidden accounts, email delivery, invite resend, broad audit analytics/filtering/export, external provider identity events, retention/export policy, broader admin policy UX beyond those fail-closed gates, broad org/member CRUD, or full lifecycle controls that do not exist yet.
3. The access subsection should fetch `GET /api/v1/repos`, show loading/error/empty/populated states truthfully, render only repositories already visible to the authenticated user, and stay explicit that the route is a read-only visibility baseline rather than shipped permission-sync management or role-edit UX.
4. The linked-accounts subsection should fetch `GET /api/v1/auth/linked-accounts`, require an authenticated local session with a real local-account record, show loading/error/empty/populated states truthfully, surface only the current local identity, external provider identities persisted for that same local account, and organization memberships visible to that identity, and stay explicit that external provider linking/callback exchange, SSO login, and account-merge workflows remain follow-up work.
5. The OAuth clients subsection should fetch `GET /api/v1/auth/oauth-clients`, show loading/error/empty/populated states truthfully, expose a minimal authenticated create form that POSTs organization, name, and redirect URIs to `/api/v1/auth/oauth-clients`, append or refresh the newly created client in the visible inventory, reveal the returned plaintext `client_secret` only in the immediate creation-success area, and must not expose any secret hash or persist plaintext secret material into the inventory.
6. The observability subsection should fetch `GET /api/v1/auth/audit-events` and `GET /api/v1/auth/analytics`, show loading/error/empty/populated states truthfully for each dataset, and present the returned audit-event / analytics-record details. In `DATABASE_URL` mode, local member invite create/cancel, member-role update, and member-removal audit events are durable PostgreSQL rows scoped to organizations visible to the current user; filtering/export, broad audit analytics, external-provider identity events, retention/export, and full audit parity remain follow-up work.
7. The review-automation subsection should fetch `GET /api/v1/auth/review-webhooks`, `GET /api/v1/auth/review-webhook-delivery-attempts`, and `GET /api/v1/auth/review-agent-runs`, show loading/error/empty/populated states truthfully for each dataset, present the returned visibility fields without exposing secret hashes or secret material, and stay honest that webhook management/retry/run control UX remains follow-up work.
8. Repo-scope copy must stay truthful: empty scope means the key is not repo-bound and can reach the repos currently visible to the authenticated user, not arbitrary hidden repositories.
9. Grounding for each current non-connections section is:
   - API keys → authenticated inventory from `/api/v1/auth/api-keys` plus revoke through `/api/v1/auth/api-keys/{api_key_id}/revoke`
   - Members → authenticated admin-visible inventory from `/api/v1/auth/members`, minimal invite creation through `POST /api/v1/auth/members/invites`, pending-invite cancellation through `DELETE /api/v1/auth/members/invites/{invite_id}`, bounded role updates through `PATCH /api/v1/auth/members/{user_id}/role`, and bounded member removal through `DELETE /api/v1/auth/members/{user_id}`
   - Access → authenticated read-only visible-repository inventory from `/api/v1/repos`
   - Linked accounts → authenticated current-identity, same-user external identity, and visible-membership inventory from `/api/v1/auth/linked-accounts`
   - OAuth clients → authenticated inventory plus minimal create workflow from `GET/POST /api/v1/auth/oauth-clients`
   - Audit & analytics / observability → authenticated inventory from `/api/v1/auth/audit-events` and `/api/v1/auth/analytics`
   - Review automation → authenticated inventory from `/api/v1/auth/review-webhooks`, `/api/v1/auth/review-webhook-delivery-attempts`, and `/api/v1/auth/review-agent-runs`

### 4. Boundaries and non-goals
1. This slice does **not** claim shipped onboarding, login, org membership CRUD, invite redemption UX, external provider linking/callback exchange, SSO login, or account-merge UX.
2. This slice does **not** claim CRUD-complete UX for API keys, members, access/permission-sync management, linked accounts, OAuth clients, audit analytics filtering/export, or review automation; API keys currently have only a minimal inventory/create/revoke panel with one-time secret reveal and basic newline-delimited scope entry, members currently expose only admin-visible roster/invite inventory plus minimal pending-invite creation/cancellation backed by file state or PostgreSQL auth metadata with PostgreSQL audit durability for local invite/create/cancel/remove/role-update events when configured, access currently exposes only a read-only visible-repository inventory, linked accounts currently exposes only a read-only current-identity, same-user external-identity, and visible-membership inventory, OAuth clients currently expose only a minimal inventory-plus-create baseline, and observability currently has only a minimal read-only inventory panel.
3. Aside from the focused authenticated members inventory route, the read-only access visibility route, and the read-only linked-accounts identity route, this settings shell should not be read as a claim of broader new backend admin surface area or full org/invite/permission-sync/external-identity management parity.

## Minimum verification evidence
1. Focused frontend tests prove:
   - the settings landing page renders
   - the API-key subsection renders authenticated inventory details, create behavior with one-time plaintext secret reveal, revoke behavior, and fail-closed load-error management controls inside the shared shell
   - the members subsection renders authenticated organization member and invite inventory details, minimal invite-create and pending-invite cancel behavior, and fail-closed load-error management controls inside the shared shell
   - the access subsection renders authenticated visible-repository inventory details plus the truthful empty state inside the shared shell
   - the linked-accounts subsection renders authenticated current-identity, same-user external-identity, and visible-membership details plus truthful failure handling inside the shared shell
   - the OAuth-clients subsection renders authenticated inventory details, the minimal create workflow, and empty/error states inside the shared shell without exposing secret material in the inventory
   - the observability subsection renders authenticated audit/analytics inventory details and endpoint-scoped failure states inside the shared shell
   - the review-automation subsection renders authenticated review-webhook, delivery-attempt, and review-agent-run visibility details plus endpoint-scoped failure states inside the shared shell without exposing secret material
   - the existing connections route remains reachable within the shared shell
2. Broader `web/src/App.test.tsx` coverage still passes after the route-family change.
3. Production build still passes after the navigation-shell change.
