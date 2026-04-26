# Task 87c Durable API-key and OAuth-client Metadata Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Ship one bounded Task 87c closure where API-key and OAuth-client inventory/auth lifecycle become durable in PostgreSQL whenever `DATABASE_URL` is configured, while keeping the existing file-backed `OrganizationState` fallback for local development without `DATABASE_URL`.

**Architecture:** Add one focused SQL migration plus a narrow PostgreSQL auth-metadata helper/store for `api_keys` and `oauth_clients`, then wire only the directly coupled auth/admin routes (`/api/v1/auth/api-keys`, `/api/v1/auth/api-keys/{id}/revoke`, `/api/v1/auth/oauth-clients`) and API-key bearer authentication through that durable path when `DATABASE_URL` is configured. Leave broader catalog, analytics/audit, connections, and remaining organization aggregates explicitly deferred inside Task 87c.

**Tech Stack:** Rust, Axum, Tokio, SQLx/PostgreSQL, Argon2, serde, repo-local acceptance/docs.

---

### Task 1: Add failing migration and PostgreSQL metadata-store tests

**Objective:** Define the durable API-key/OAuth-client contract before changing production code.

**Files:**
- Modify: `crates/api/migrations/0009_api_key_oauth_client_metadata.up.sql`
- Modify: `crates/api/migrations/0009_api_key_oauth_client_metadata.down.sql`
- Modify: `crates/api/src/storage.rs`
- Modify: `crates/api/src/auth.rs`

**Step 1: Write failing migration contract checks**
- Extend the migration-order assertions in `crates/api/src/storage.rs` so they expect the new `0008` migration and verify that `api_keys` and `oauth_clients` are still absent from earlier migrations but present with the right columns/constraints in the new one.

**Step 2: Write failing PostgreSQL metadata helper tests**
- Add focused SQLx-backed tests in `crates/api/src/auth.rs` that cover:
  - listing one user’s API keys from PostgreSQL,
  - authenticating a PostgreSQL-backed API key with repo-scope validation,
  - revoking a PostgreSQL-backed API key,
  - listing visible OAuth clients for admin-visible organizations,
  - creating a PostgreSQL-backed OAuth client with durable redirect URIs.
- Each test should reset/migrate the test database, seed only the required rows directly with SQLx, and assert exact persisted rows/fields.

**Step 3: Run RED verification**
Run:
```bash
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api pg_api_key_ -- --nocapture --test-threads=1
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api pg_oauth_client_ -- --nocapture --test-threads=1
cargo test -p sourcebot-api --lib migration_inventory -- --nocapture
```
Expected: FAIL because the migration/store does not exist yet.

---

### Task 2: Implement the minimal PostgreSQL API-key/OAuth-client metadata store

**Objective:** Make the focused PostgreSQL metadata tests pass with the narrowest durable implementation.

**Files:**
- Modify: `crates/api/src/auth.rs`
- Modify: `crates/api/migrations/0009_api_key_oauth_client_metadata.up.sql`
- Modify: `crates/api/migrations/0009_api_key_oauth_client_metadata.down.sql`

**Step 1: Add the migration**
- Create `api_keys` and `oauth_clients` tables in `0009` with only the fields already modeled and exercised by the current API:
  - `api_keys`: id, user_id, name, secret_hash, created_at, revoked_at, repo_scope
  - `oauth_clients`: id, organization_id, name, client_id, client_secret_hash, redirect_uris, created_by_user_id, created_at, revoked_at
- Use foreign keys back to `local_accounts` / `organizations` and array columns for repo scope / redirect URIs where appropriate.

**Step 2: Add the PostgreSQL helper/store**
- Extend `crates/api/src/auth.rs` with narrowly scoped methods to:
  - list API keys by user id,
  - fetch one API key by id,
  - create/revoke an API key,
  - list OAuth clients visible to a set of organization ids,
  - create an OAuth client,
  - validate referenced users/orgs fail closed.
- Reuse the existing timestamp formatting and row-to-model parsing patterns already used by the PostgreSQL bootstrap/session/account helpers.

**Step 3: Run GREEN verification**
Run:
```bash
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api pg_api_key_ -- --nocapture --test-threads=1
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api pg_oauth_client_ -- --nocapture --test-threads=1
cargo test -p sourcebot-api --lib migration_inventory -- --nocapture
```
Expected: PASS.

---

### Task 3: Add router-level PostgreSQL API-key/OAuth-client regressions and wire the bounded routes

**Objective:** Prove the live app uses durable PostgreSQL metadata for API-key and OAuth-client lifecycle when `DATABASE_URL` is configured.

**Files:**
- Modify: `crates/api/src/main.rs`

**Step 1: Write failing route regressions**
- Add focused route tests that build the app with:
  - a real `database_url`,
  - PostgreSQL-backed bootstrap/session/account/org state,
  - a stale/empty file-backed organization-state path that should no longer be authoritative for the targeted routes.
- Required coverage:
  1. `/api/v1/auth/api-keys` lists PostgreSQL-backed keys for the logged-in user,
  2. `POST /api/v1/auth/api-keys` persists a durable PostgreSQL API key,
  3. `POST /api/v1/auth/api-keys/{id}/revoke` durably revokes that key,
  4. API-key Bearer auth succeeds/fails closed against PostgreSQL-backed records,
  5. `/api/v1/auth/oauth-clients` lists PostgreSQL-backed clients visible to the admin user,
  6. `POST /api/v1/auth/oauth-clients` durably creates a PostgreSQL-backed client.

**Step 2: Implement the bounded route wiring**
- In `crates/api/src/main.rs`, when `state.config.database_url` is configured:
  - route API-key inventory/create/revoke and API-key bearer authentication through the new PostgreSQL helper/store,
  - route OAuth-client inventory/create through the new PostgreSQL helper/store,
  - preserve the current file-backed `OrganizationState` behavior when `DATABASE_URL` is absent,
  - avoid widening this slice to analytics, audit events, connections, review webhooks, or broader catalog work.

**Step 3: Run focused verification**
Run:
```bash
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api auth_api_key_ -- --nocapture --test-threads=1
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api auth_oauth_clients_ -- --nocapture --test-threads=1
```
Expected: FAIL first, then PASS once the wiring is complete.

---

### Task 4: Update truthful docs and verification entrypoints

**Objective:** Ground the narrower shipped baseline without overclaiming broader task87c parity.

**Files:**
- Modify: `Makefile`
- Modify: `README.md`
- Modify: `specs/acceptance/auth.md`
- Modify: `docs/reports/2026-04-18-parity-gap-report.md`

**Step 1: Patch verification/docs**
- Extend `make sqlx-test` so it exercises the new PostgreSQL API-key/OAuth-client regressions alongside the existing durable auth coverage.
- Document exactly this truth:
  - when `DATABASE_URL` is configured, API-key inventory/create/revoke, API-key bearer auth, and OAuth-client inventory/create now restore from PostgreSQL-backed durable metadata,
  - bootstrap/local-session/local-account/membership/invite durability remains covered by Tasks 87a/87b1/87b2,
  - catalog persistence, analytics/audit/runtime aggregates, and remaining task87c work still remain follow-up slices.

**Step 2: Run raw-content truth checks**
Run:
```bash
python3 - <<'PY'
from pathlib import Path
checks = {
    'README.md': ['API-key inventory/create/revoke and OAuth-client inventory/create now restore from PostgreSQL-backed durable metadata when DATABASE_URL is configured'],
    'specs/acceptance/auth.md': ['When DATABASE_URL is configured, API-key inventory/create/revoke, API-key bearer auth, and OAuth-client inventory/create now restore from PostgreSQL-backed durable metadata'],
    'docs/reports/2026-04-18-parity-gap-report.md': ['Catalog persistence, analytics/audit aggregates, and remaining organization-state durability still remain follow-up work after the PostgreSQL API-key/OAuth-client slice'],
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
- Review: `crates/api/src/auth.rs`, `crates/api/src/main.rs`, `crates/api/src/storage.rs`, `crates/api/migrations/0009_api_key_oauth_client_metadata.up.sql`, `crates/api/migrations/0009_api_key_oauth_client_metadata.down.sql`, `Makefile`, `README.md`, `specs/acceptance/auth.md`, `docs/reports/2026-04-18-parity-gap-report.md`, `docs/plans/2026-04-26-task87c-durable-api-key-oauth-client-metadata-plan.md`

**Step 1: Run verification**
Run:
```bash
TEST_DATABASE_URL="$TEST_DATABASE_URL" make sqlx-test
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api auth_api_key_ -- --nocapture --test-threads=1
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api auth_oauth_clients_ -- --nocapture --test-threads=1
git diff --check
git diff --cached --check
```
Expected: PASS.

**Step 2: Run pre-commit review pipeline**
- Inspect the diff.
- Run an added-lines security scan.
- Run one independent reviewer.
- Fix any blocking findings and re-verify.

**Step 3: Commit**
```bash
git add crates/api/src/auth.rs crates/api/src/main.rs crates/api/src/storage.rs crates/api/migrations/0009_api_key_oauth_client_metadata.up.sql crates/api/migrations/0009_api_key_oauth_client_metadata.down.sql Makefile README.md specs/acceptance/auth.md docs/reports/2026-04-18-parity-gap-report.md docs/plans/2026-04-26-task87c-durable-api-key-oauth-client-metadata-plan.md
git commit -m "feat: add durable postgres api key metadata"
```
