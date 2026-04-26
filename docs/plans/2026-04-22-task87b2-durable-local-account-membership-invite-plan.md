# Task 87b2 Durable Local-Account, Membership, and Invite Metadata Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Ship one bounded Task 87b2 closure where invited/local-account auth identity, organization memberships, and invite acceptance state become durable in PostgreSQL whenever `DATABASE_URL` is configured, while keeping the existing file-backed organization-state fallback for local development without `DATABASE_URL`.

**Architecture:** Reuse the existing durable `local_accounts`, `organization_memberships`, and `organization_invites` tables instead of widening all remaining organization/auth aggregates at once. Add a narrow PostgreSQL-backed organization-auth metadata helper/store for local accounts, memberships, invites, and organizations; wire only the directly coupled auth/org routes (`/api/v1/auth/login`, `/api/v1/auth/invite-redeem`, `/api/v1/auth/me`, `/api/v1/auth/members`, `/api/v1/auth/linked-accounts`) to use PostgreSQL when configured; and leave API keys, OAuth clients, analytics, audit events, connections, and other organization-state aggregates explicitly deferred.

**Tech Stack:** Rust, Axum, Tokio, SQLx/PostgreSQL, Argon2, serde, repo-local acceptance/docs.

---

### Task 1: Add failing PostgreSQL organization-auth metadata tests

**Objective:** Define the durable local-account/membership/invite contract before changing production code.

**Files:**
- Modify: `crates/api/src/auth.rs`
- Verify: `TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api pg_org_auth_metadata_ -- --nocapture --test-threads=1`

**Step 1: Write failing tests**

Add focused SQLx-backed tests near the existing PostgreSQL auth-store coverage for behavior like:
- `pg_org_auth_metadata_reads_local_account_by_email_and_id`
- `pg_org_auth_metadata_lists_admin_organizations_and_membership_rosters`
- `pg_org_auth_metadata_accepts_invite_and_persists_account_membership_and_invite_acceptance`

Each test should:
1. obtain `TEST_DATABASE_URL` from the environment,
2. reset/migrate the test database,
3. seed the minimal `organizations`, `local_accounts`, `organization_memberships`, and `organization_invites` rows directly with SQLx,
4. exercise the new PostgreSQL metadata helper/store,
5. assert exact durable rows/fields rather than indirect behavior only.

**Step 2: Run test to verify failure**

Run:
```bash
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api pg_org_auth_metadata_ -- --nocapture --test-threads=1
```
Expected: FAIL because the PostgreSQL organization-auth metadata helper/store does not exist yet.

**Step 3: Commit nothing yet**

Do not commit at RED.

---

### Task 2: Implement the minimal PostgreSQL organization-auth metadata helper/store

**Objective:** Make the focused PostgreSQL metadata tests pass with the smallest production slice.

**Files:**
- Modify: `crates/api/src/auth.rs`

**Step 1: Write minimal implementation**

In `crates/api/src/auth.rs`:
- add a PostgreSQL-backed helper/store for the already-migrated organization-auth tables,
- provide narrowly scoped methods needed by the target slice, for example:
  - look up local account by email/id,
  - list admin organization ids for a user,
  - build member/invite rosters for organizations,
  - list memberships for one user,
  - redeem/accept an invite by atomically persisting account + membership + invite acceptance,
- preserve fail-closed validation for missing/malformed rows,
- keep file-backed builders/traits untouched for broader aggregates that still depend on whole `OrganizationState`.

**Step 2: Run focused tests to verify GREEN**

Run:
```bash
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api pg_org_auth_metadata_ -- --nocapture --test-threads=1
```
Expected: PASS.

**Step 3: Run regression smoke for existing PostgreSQL auth helpers**

Run:
```bash
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api pg_local_session_store_ -- --nocapture --test-threads=1
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api pg_bootstrap_store_ -- --nocapture --test-threads=1
```
Expected: PASS.

---

### Task 3: Add router-level durable local-account/membership/invite regressions

**Objective:** Prove the live app wiring uses durable PostgreSQL metadata for the bounded auth/org slice when `DATABASE_URL` is configured.

**Files:**
- Modify: `crates/api/src/main.rs`

**Step 1: Write failing route regressions**

Add focused tests that build the app with:
- a real `database_url`,
- a seeded PostgreSQL metadata state for organizations/accounts/memberships/invites,
- a throwaway organization-state JSON path that should no longer be authoritative for the targeted PostgreSQL-backed auth/org routes,
- PostgreSQL-backed local sessions as already supported.

Required coverage should prove:
1. `/api/v1/auth/login` authenticates a non-bootstrap invited/local account from PostgreSQL-backed durable metadata,
2. `/api/v1/auth/me` restores that account after restart without relying on file-backed `OrganizationState`,
3. `/api/v1/auth/members` returns durable member + pending/accepted invite rosters for admin-visible organizations,
4. `/api/v1/auth/linked-accounts` returns durable local identity + memberships for the logged-in user,
5. `/api/v1/auth/invite-redeem` durably persists the accepted account, membership, and invite acceptance in PostgreSQL.

Name the tests with clear `postgres_backed_...` / `auth_..._postgres_...` prefixes so they can be targeted from `make sqlx-test`.

**Step 2: Run targeted tests to verify RED then GREEN**

Run commands like:
```bash
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api postgres_backed_local_account_ -- --nocapture --test-threads=1
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api auth_members_ -- --nocapture --test-threads=1
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api auth_linked_accounts_ -- --nocapture --test-threads=1
```
Expected: targeted failures first, then PASS once the route wiring is complete.

**Step 3: Wire the bounded live routes**

In `crates/api/src/main.rs`:
- keep bootstrap-admin logic on the existing bootstrap store,
- when `state.config.database_url` is configured, route the following through the new PostgreSQL helper/store:
  - local-account login lookup for non-bootstrap users,
  - auth-me account lookup for non-bootstrap sessions,
  - members roster assembly,
  - linked-accounts membership lookup,
  - invite redemption persistence + response hydration,
- preserve the existing file-backed `OrganizationState` path when `DATABASE_URL` is absent,
- do not widen this slice to API keys/OAuth clients/connections/audit/search contexts.

**Step 4: Run broader auth confidence checks**

Run:
```bash
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api invite_redeem_ -- --nocapture --test-threads=1
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api auth_me_ -- --nocapture --test-threads=1
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api auth_members_ -- --nocapture --test-threads=1
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api auth_linked_accounts_ -- --nocapture --test-threads=1
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api auth_login_ -- --nocapture --test-threads=1
```
Expected: PASS.

---

### Task 4: Update truthful docs and verification entrypoints

**Objective:** Ground the narrower shipped baseline without overclaiming broader durable organization/auth parity.

**Files:**
- Modify: `Makefile`
- Modify: `README.md`
- Modify: `specs/acceptance/auth.md`
- Modify: `docs/reports/2026-04-18-parity-gap-report.md`

**Step 1: Patch docs and verification wiring**

Update `make sqlx-test` so it exercises the new PostgreSQL local-account/membership/invite regressions in addition to the existing bootstrap/local-session coverage.

Document exactly this truth:
- when `DATABASE_URL` is configured, local invited-account login, auth-me identity restoration, member roster visibility, linked-account memberships, and invite acceptance now restore from PostgreSQL-backed durable metadata,
- bootstrap-admin durability remains covered from Task 87b1,
- API keys, OAuth clients, connections, analytics, audit events, and the remaining whole-aggregate organization state are still follow-up work inside Task 87.

**Step 2: Run raw-content truth checks**

Run:
```bash
python3 - <<'PY'
from pathlib import Path
checks = {
    'README.md': ['local-account, membership, and invite auth metadata now restore from PostgreSQL when DATABASE_URL is configured'],
    'specs/acceptance/auth.md': ['When DATABASE_URL is configured, local invited-account login, auth me, member rosters, linked-account memberships, and invite acceptance now restore from PostgreSQL-backed durable metadata'],
    'docs/reports/2026-04-18-parity-gap-report.md': ['API-key, OAuth-client, and broader organization aggregate durability remain follow-up work after the PostgreSQL local-account/membership/invite slice'],
}
for rel, snippets in checks.items():
    text = Path(rel).read_text()
    for snippet in snippets:
        assert snippet in text, f'{rel} missing: {snippet}'
print('doc truth checks passed')
PY
```
Expected: PASS.

---

### Task 5: Run verification, independent review, and close the slice

**Objective:** Verify the closure end-to-end, get independent review, and prepare it for commit/state/push.

**Files:**
- Review: `crates/api/src/auth.rs`, `crates/api/src/main.rs`, `Makefile`, `README.md`, `specs/acceptance/auth.md`, `docs/reports/2026-04-18-parity-gap-report.md`, `docs/plans/2026-04-22-task87b2-durable-local-account-membership-invite-plan.md`

**Step 1: Run verification**

Run:
```bash
TEST_DATABASE_URL="$TEST_DATABASE_URL" make sqlx-test
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api invite_redeem_ -- --nocapture --test-threads=1
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api auth_me_ -- --nocapture --test-threads=1
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api auth_members_ -- --nocapture --test-threads=1
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api auth_linked_accounts_ -- --nocapture --test-threads=1
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api auth_login_ -- --nocapture --test-threads=1
git diff --check
git diff --cached --check
```
Expected: PASS.

**Step 2: Run pre-commit review pipeline**

Use the requesting-code-review workflow:
1. inspect `git diff --cached` / `git diff`,
2. run the added-lines security scan,
3. run one independent reviewer,
4. fix any blocking findings,
5. re-run verification until clean.

**Step 3: Commit**

```bash
git add crates/api/src/auth.rs crates/api/src/main.rs Makefile README.md specs/acceptance/auth.md docs/reports/2026-04-18-parity-gap-report.md docs/plans/2026-04-22-task87b2-durable-local-account-membership-invite-plan.md
git commit -m "feat: add durable postgres local account metadata"
```
