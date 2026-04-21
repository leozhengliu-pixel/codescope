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
- Backend auth/API-key surface: `crates/api/src/main.rs`
- Persisted auth/org models: `crates/models/src/lib.rs`
- Frontend API-key inventory/revoke panel: `web/src/App.tsx`
- Focused frontend verification: `web/src/App.test.tsx`
- Related settings-shell contract: `specs/acceptance/settings-navigation.md`

## Expected behavior
1. First-run onboarding can initialize the first admin under a self-hosted deployment.
2. Users can authenticate and receive a durable session.
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
- First boot allows creating one admin account, then closes bootstrap flow.
- A viewer can search and browse allowed repos but cannot manage connections.
- An authenticated user can open `#/settings/api-keys`, load their current API-key inventory, distinguish active vs revoked keys, and see repo-scope wording that stays truthful when a key is not repo-bound.
- An authenticated user can revoke an active key from that minimal inventory panel, after which the key is no longer shown as active even though richer creation/scoping UX remains follow-up work.
- Revoking a user's org membership removes search results from previously visible repos.
