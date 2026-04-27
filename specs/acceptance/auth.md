# Acceptance Spec: Auth and Permissions

## Scope
- First-run onboarding
- Local auth baseline
- Organizations, membership, invites, roles
- API keys
- Permission enforcement across browse/search/ask

## Inputs
- User signup/login credentials or SSO assertions
- Organization membership and role assignments
- API key creation requests

## Current rewrite grounding
- Backend auth/API-key/admin surface: `crates/api/src/main.rs`
- Persisted auth/org models: `crates/models/src/lib.rs`
- Frontend auth route baseline plus API-key inventory/create/revoke, members inventory/create-invite, access visibility inventory, OAuth-client inventory, observability, and review-automation settings panels: `web/src/App.tsx`
- Focused frontend verification: `web/src/App.test.tsx`
- Related settings-shell contract: `specs/acceptance/settings-navigation.md`

## Expected behavior
1. First-run onboarding can initialize the first admin under a self-hosted deployment.
2. Users can authenticate and receive a durable session; when `DATABASE_URL` is configured, first-admin bootstrap status/login and local sessions persist in PostgreSQL across API restarts.
3. Org admins can invite users and assign roles.
4. Role changes propagate to repository access checks.
5. API keys are creatable, listable, revocable, and scoped according to product policy.
6. Authorization is enforced consistently across UI views and API endpoints.

## Permission behavior
- Unauthorized requests to protected resources return policy-consistent errors.
- Revoked access takes effect without requiring re-index of unrelated repositories.
- API keys inherit or explicitly scope permissions; they cannot exceed the owner's access.

## Edge cases
- First admin bootstrap must be disabled after initialization.
- Invite acceptance should handle duplicate emails and expired tokens.
- Deleted users must not retain active API keys or sessions.

## Black-box examples
- First boot allows creating one admin account from `#/auth`, then closes bootstrap flow and falls through to local login.
- A local admin can sign in from `#/auth`, the frontend persists the returned `session_id:session_secret` token client-side, restores `/api/v1/auth/me`, and reuses that bearer token on later protected `/api/v1/auth/...` requests until logout clears it.
- When DATABASE_URL is configured, first-admin bootstrap status and login now restore from PostgreSQL-backed durable metadata, so a fresh API process still sees bootstrap closed and can authenticate the bootstrap admin even if the configured bootstrap-state file path is empty, stale, or invalid.
- When DATABASE_URL is configured, API-key inventory/create/revoke, API-key bearer auth, OAuth-client inventory/create, and members invite creation/cancellation/member-role update/member removal now restore from PostgreSQL-backed durable metadata. A fresh API process can still restore `/api/v1/auth/me`, `/api/v1/auth/members`, `/api/v1/auth/linked-accounts`, `/api/v1/auth/api-keys`, and `/api/v1/auth/oauth-clients` even if the configured bootstrap/local-session/organization-state files are empty or stale; the same bounded PostgreSQL-backed slice also covers the invited/local-account identity, linked organization memberships, durable member invite rows and membership deletes, API-key bearer verification, and OAuth-client inventories behind those routes, while connections, analytics, durable audit-event storage beyond the shipped file-backed visibility aggregate, repo-permission durability, and broader whole-aggregate organization state remain follow-up work.
- If a local-session or API-key bearer token references missing or corrupted persisted secret material, the corresponding protected auth request still fails closed with `401` after the same Argon2 verification burn path instead of shortcutting on the missing/malformed stored record.
- An invited email can open `#/auth?invite=<invite_id>&email=<invited_email>`, submit name and password to `POST /api/v1/auth/invite-redeem`, receive a new local session, and immediately land in the signed-in auth state without claiming that broader invite creation or admin invite-management UX already exists.
- A user landing on `#/auth` with OAuth callback params such as `provider`, `error`, `error_description`, `code`, or `state` still gets the local login form plus a truthful provider-aware callback-status callout that acknowledges the redirect, surfaces any returned error/code details, and explicitly says that this rewrite does not finish external-provider sign-in/callback exchange there yet.
- A viewer can search and browse allowed repos but cannot manage connections.
- An authenticated user can open `#/settings/api-keys`, load their current API-key inventory from durable PostgreSQL metadata when `DATABASE_URL` is configured, distinguish active vs revoked keys, create a minimal new key by submitting a name plus optional newline-delimited repository scope, see the returned plaintext `secret` only in the immediate creation-success area, and see repo-scope wording that stays truthful when a key is not repo-bound.
- An authenticated user can revoke an active key from that minimal inventory panel, after which the key is no longer shown as active even after an API restart when `DATABASE_URL` is configured, though richer rotation, bulk management, and advanced scoping UX remain follow-up work.
- An authenticated local-session user who administers at least one organization can open `#/settings/members`, load the admin-visible organization member and invite inventory from `/api/v1/auth/members`, inspect joined member account details plus truthful accepted-vs-pending invite status, create a minimal pending local invite through `POST /api/v1/auth/members/invites`, cancel a pending invite through `DELETE /api/v1/auth/members/invites/{invite_id}`, update a visible member role through `PATCH /api/v1/auth/members/{user_id}/role`, and remove a visible active member through `DELETE /api/v1/auth/members/{user_id}` with `organization_id` for an administered organization. The baseline records a local redemption invite in the file-backed fallback, creates/deletes durable PostgreSQL invite rows when `DATABASE_URL` is configured, updates member roles or deletes membership rows in file-backed or PostgreSQL auth metadata, rejects accepted/hidden invite/member targets without mutation, rejects self-removal plus last-admin removal/demotion without mutation, never deletes the local account when removing membership, and keeps shipped audit visibility in the file-backed audit aggregate consumed by audit APIs with `auth.member.removed` metadata. This remains bounded: no email delivery, invite resend workflow, durable audit-event storage, broader admin policy UX beyond those fail-closed last-admin/self-removal gates, broad org/member CRUD, or full org/member lifecycle parity is claimed.
- An authenticated user can open `#/settings/access`, load the repositories currently visible to their account from `/api/v1/repos`, inspect truthful loading/empty/populated states plus visible repository metadata, and confirm that the panel stays read-only rather than claiming shipped permission-sync management or role-edit workflows.
- An authenticated user can open `#/settings/linked-accounts`, load the current local account identity plus visible organization memberships from `/api/v1/auth/linked-accounts`, confirm the route fails closed when the local session lacks a real local-account record, and see explicit copy that external provider linking and SSO remain follow-up work.
- An authenticated user can open `#/settings/oauth-clients`, load the admin-visible OAuth client inventory from `/api/v1/auth/oauth-clients`, submit a minimal authenticated create request with organization, name, and redirect URIs, receive the returned plaintext `client_secret` exactly once in a focused success callout, and confirm that no secret hash or plaintext secret material is persisted into the inventory while the PostgreSQL-backed client metadata remains durable across API restarts when `DATABASE_URL` is configured and richer OAuth authorization/token/revocation workflows plus broader manage UX remain follow-up work.
- An authenticated user can open `#/settings/observability`, load visible audit-event and analytics inventories from `/api/v1/auth/audit-events` and `/api/v1/auth/analytics`, and see truthful per-endpoint loading/failure states without claiming filtering/export workflows that do not exist yet.
- An authenticated user can open `#/settings/review-automation`, load visible review-webhook, delivery-attempt, and review-agent-run inventories from `/api/v1/auth/review-webhooks`, `/api/v1/auth/review-webhook-delivery-attempts`, and `/api/v1/auth/review-agent-runs`, and confirm that the UI shows endpoint-scoped loading/failure states without exposing webhook secret hashes or claiming richer retry/manage/run-control workflows that do not exist yet.
- Revoking a user's org membership removes search results from previously visible repos.
