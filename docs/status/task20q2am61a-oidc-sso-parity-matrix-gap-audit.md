# task20q2am61a — OIDC/SSO parity-matrix gap audit

## Scope
Audit the next smallest OIDC/SSO documentation drift after the generic/local parity-matrix follow-up closed `task20q2am60b`.

## Grounded evidence
- `specs/FEATURE_PARITY.md:33` still leaves the `OIDC / SSO providers` row at `Needs audit | _TBD_ |`.
- `specs/acceptance/integrations.md:24` already keeps the contract conservative: OIDC/SSO login can be enabled with provider metadata and mapped to local users/orgs only once the dedicated identity-provider slices land.
- `docs/reports/2026-04-18-parity-gap-report.md:66` already records the live rewrite as `Missing` for OIDC / SSO provider login and external account mapping, grounding that the current auth surface is still local bootstrap/login/session management plus OAuth-client admin endpoints.
- `docs/reports/2026-04-18-parity-gap-report.md:74` further summarizes the integration-domain gap by noting the codebase still has no OIDC/SSO login path.
- `docs/reports/2026-04-18-parity-gap-report.md:107` separately records linked external accounts and SSO/OIDC identity mapping as `Missing`, with the persisted account model still limited to `LocalAccount`.
- Repo-wide code search during this audit found no `oidc`, `sso`, `openid`, `linked account`, or `external account` implementation paths under `crates/**/*.rs` or `web/src/**/*.tsx`.
- `crates/api/src/main.rs:236-238` and `crates/api/src/main.rs:1298-1315` show the live authenticated integration-adjacent auth/admin surface currently includes OAuth-client management endpoints, not an OIDC/SSO login or callback flow.
- `crates/api/src/auth.rs:872-900` seeds `LocalAccount` plus `OAuthClient` fixture state, but does not introduce an external identity or provider-linked account model.

## Finding
The next smallest truthful drift is the parity-matrix row itself, not the acceptance spec or canonical gap report.

The acceptance and gap-report docs already agree that OIDC/SSO parity is still missing: login/callback handling, provider metadata wiring, and external-account mapping are deferred to later identity-provider slices. The parity matrix is now the stale artifact because it still says `Needs audit | _TBD_ |` instead of reflecting the already-grounded `Missing` status and conservative evidence.

## Smallest follow-up slice
`task20q2am61b` should tighten only the `specs/FEATURE_PARITY.md` `OIDC / SSO providers` row so it records the current status as missing and cites the already-grounded acceptance/gap-report evidence, without changing product behavior or broadening other integration rows.

## Out of scope
- Implementing Task 60 identity-provider behavior.
- Editing `docs/reports/2026-04-18-parity-gap-report.md` or `specs/acceptance/integrations.md`, which already express the conservative missing-parity state.
- Claiming linked-account, provider metadata, login callback, or frontend SSO UX parity that the repo still does not ship.
