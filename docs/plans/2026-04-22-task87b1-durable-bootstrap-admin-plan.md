# Task 87b1 Durable Bootstrap Admin Metadata Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Ship one bounded Task 87b closure where first-admin bootstrap state becomes durable in PostgreSQL whenever `DATABASE_URL` is configured, while preserving the existing file-backed bootstrap fallback when it is not.

**Architecture:** Reuse the existing `BootstrapStore` abstraction instead of widening all organization/auth aggregates at once. Add a narrow migration for durable bootstrap password hashes on `local_accounts`, implement a PostgreSQL-backed bootstrap store that maps the bootstrap admin onto the existing `local_accounts` table, wire builder selection off `DATABASE_URL`, and prove the real auth/bootstrap routes can initialize, report status, and log in after restart without depending on the bootstrap-state JSON file.

**Tech Stack:** Rust, Axum, Tokio, SQLx/PostgreSQL, Argon2, serde, repo-local acceptance/docs.

---

### Task 1: Add failing PostgreSQL bootstrap-store tests

**Objective:** Define the durable bootstrap-admin contract before changing production code.

**Files:**
- Modify: `crates/api/src/auth.rs`
- Verify: `TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api pg_bootstrap_store_ -- --nocapture --test-threads=1`

**Step 1: Write failing tests**

Add focused SQLx-backed tests near the existing file bootstrap-store coverage:
- `pg_bootstrap_store_requires_bootstrap_when_admin_row_is_missing`
- `pg_bootstrap_store_initializes_bootstrap_admin_once`
- `pg_bootstrap_store_reads_persisted_bootstrap_state`

Each test should:
1. obtain `TEST_DATABASE_URL` from the environment,
2. reset/migrate the test database,
3. construct a PostgreSQL bootstrap store,
4. assert `bootstrap_status`, `initialize_bootstrap`, and `bootstrap_state` behavior,
5. assert on the persisted `local_accounts` row, including `password_hash`.

**Step 2: Run test to verify failure**

Run:
```bash
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api pg_bootstrap_store_ -- --nocapture --test-threads=1
```
Expected: FAIL because the migration column and/or `PgBootstrapStore` do not exist yet.

**Step 3: Commit nothing yet**

Do not commit at RED.

---

### Task 2: Add durable bootstrap password-hash storage and minimal PostgreSQL bootstrap implementation

**Objective:** Make the PostgreSQL bootstrap-store tests pass with the smallest production slice.

**Files:**
- Create: `crates/api/migrations/0008_local_account_password_hash.up.sql`
- Create: `crates/api/migrations/0008_local_account_password_hash.down.sql`
- Modify: `crates/api/src/auth.rs`

**Step 1: Write minimal implementation**

In the migration files:
- add nullable `password_hash TEXT` to `local_accounts` in the up migration,
- drop that column in the down migration.

In `crates/api/src/auth.rs`:
- add `PgBootstrapStore { pool: sqlx::PgPool }`,
- add a constructor consistent with the existing SQLx store pattern,
- implement `BootstrapStore` for it by treating `LOCAL_BOOTSTRAP_ADMIN_USER_ID` in `local_accounts` as the durable bootstrap record,
- return `bootstrap_required = true` only when that bootstrap-admin row is absent,
- map the stored row back into `BootstrapState`,
- fail closed on a second initialization attempt,
- change `build_bootstrap_store(...)` to accept both the state path and `database_url: Option<&str>` and choose PostgreSQL when configured, file store otherwise.

**Step 2: Run focused tests to verify GREEN**

Run:
```bash
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api pg_bootstrap_store_ -- --nocapture --test-threads=1
```
Expected: PASS.

**Step 3: Run file-store regression smoke**

Run:
```bash
cargo test -p sourcebot-api --bin sourcebot-api file_bootstrap_store_ -- --nocapture
```
Expected: PASS.

---

### Task 3: Add router-level durable bootstrap regressions

**Objective:** Prove the live app wiring uses the PostgreSQL bootstrap store when `DATABASE_URL` is configured.

**Files:**
- Modify: `crates/api/src/main.rs`

**Step 1: Write failing route regressions**

Add focused tests that build the app with:
- a real `database_url`,
- a throwaway bootstrap-state file path that should remain unused for durable bootstrap behavior,
- a local-session path (file or PostgreSQL-backed session path as already supported).

Required tests:
- `bootstrap_status_uses_postgres_backed_bootstrap_admin_when_database_url_is_configured`
- `bootstrap_create_persists_bootstrap_admin_in_postgres_when_database_url_is_configured`
- `bootstrap_login_uses_postgres_backed_bootstrap_admin_when_database_url_is_configured`

The coverage should prove:
1. `GET /api/v1/auth/bootstrap` reports `bootstrap_required: true` before initialization and `false` after,
2. `POST /api/v1/auth/bootstrap` persists the admin in PostgreSQL with a hashed password,
3. `POST /api/v1/auth/login` succeeds afterward without relying on a bootstrap-state JSON file,
4. the configured bootstrap-state file path stays absent/unused in the PostgreSQL path.

**Step 2: Run targeted tests to verify RED then GREEN**

Run:
```bash
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api postgres_backed_bootstrap_ -- --nocapture --test-threads=1
```
Expected: first FAIL, then PASS after wiring is complete.

**Step 3: Run broader auth confidence checks**

Run:
```bash
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api bootstrap_ -- --nocapture --test-threads=1
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api login_returns_created_session_and_persists_hashed_secret_for_bootstrap_admin -- --exact --nocapture --test-threads=1
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api auth_me_returns_bootstrap_admin_for_valid_bearer_local_session -- --exact --nocapture --test-threads=1
```
Expected: PASS.

---

### Task 4: Update truthful docs and verification entrypoints

**Objective:** Ground the narrower shipped baseline without overclaiming broader durable org/auth parity.

**Files:**
- Modify: `Makefile`
- Modify: `README.md`
- Modify: `specs/acceptance/auth.md`
- Modify: `docs/reports/2026-04-18-parity-gap-report.md`

**Step 1: Patch docs and verification wiring**

Update `make sqlx-test` so it exercises the new PostgreSQL bootstrap tests in addition to the existing durable local-session coverage.

Document exactly this truth:
- when `DATABASE_URL` is configured, bootstrap-admin initialization/status/login now use PostgreSQL-backed durable metadata,
- durable local sessions remain supported from Task 87a,
- broader organization/auth aggregates (memberships, invites, API keys, OAuth clients, analytics, audit, etc.) are still follow-up work inside Task 87b.

**Step 2: Run raw-content truth checks**

Run:
```bash
python3 - <<'PY'
from pathlib import Path
checks = {
    'README.md': ['bootstrap admin metadata now persists in PostgreSQL when DATABASE_URL is configured'],
    'specs/acceptance/auth.md': ['When DATABASE_URL is configured, first-admin bootstrap status and login now restore from PostgreSQL-backed durable metadata'],
    'docs/reports/2026-04-18-parity-gap-report.md': ['bootstrap-admin durability now exists on PostgreSQL while broader durable organization/auth aggregates remain follow-up work'],
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
- Review: `crates/api/src/auth.rs`, `crates/api/src/main.rs`, `crates/api/migrations/0008_local_account_password_hash.up.sql`, `crates/api/migrations/0008_local_account_password_hash.down.sql`, `Makefile`, `README.md`, `specs/acceptance/auth.md`, `docs/reports/2026-04-18-parity-gap-report.md`, `docs/plans/2026-04-22-task87b1-durable-bootstrap-admin-plan.md`

**Step 1: Run verification**

Run:
```bash
TEST_DATABASE_URL="$TEST_DATABASE_URL" make sqlx-test
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api bootstrap_ -- --nocapture --test-threads=1
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api auth_ -- --nocapture --test-threads=1
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
git add crates/api/src/auth.rs crates/api/src/main.rs crates/api/migrations/0008_local_account_password_hash.up.sql crates/api/migrations/0008_local_account_password_hash.down.sql Makefile README.md specs/acceptance/auth.md docs/reports/2026-04-18-parity-gap-report.md docs/plans/2026-04-22-task87b1-durable-bootstrap-admin-plan.md
git commit -m "feat: add durable postgres bootstrap admin store"
```
