# Task 87a Durable Session Store Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Ship one bounded Task 87 closure where authenticated local sessions become durable in PostgreSQL whenever `DATABASE_URL` is configured, while preserving the existing file-backed fallback when it is not.

**Architecture:** Keep bootstrap and the broader organization/catalog aggregates on their current stores for now, but move the local-session store onto a real SQLx-backed implementation because the `sessions` table already exists in the migration harness from Task 86. Wire the API/router to choose the Postgres store only when `DATABASE_URL` is present, then prove the user-visible auth lifecycle (`login -> /auth/me -> logout/revoke`) still works through the durable path and update the docs so they no longer claim the entire durable-metadata story is still only a skeleton.

**Tech Stack:** Rust, Axum, Tokio, SQLx/PostgreSQL, serde, Argon2, repo-local docs/specs.

---

### Task 1: Add a failing Postgres local-session store test scaffold

**Objective:** Define the desired durable session-store behavior before writing production code.

**Files:**
- Modify: `crates/api/src/auth.rs`
- Verify: `make sqlx-test`

**Step 1: Write failing tests**

Add focused SQLx-backed tests near the existing file-store tests:
- `pg_local_session_store_persists_and_reads_local_sessions`
- `pg_local_session_store_rewrites_existing_session_with_same_id`
- `pg_local_session_store_deletes_only_requested_session`

Each test should:
1. obtain `DATABASE_URL` from the test environment,
2. connect to Postgres,
3. run `catalog_migrator().run(&pool).await`,
4. create a Postgres-backed session store,
5. exercise the behavior and assert on both the API result and the persisted `sessions` table rows.

**Step 2: Run test to verify failure**

Run:
```bash
DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api pg_local_session_store_ -- --nocapture
```
Expected: FAIL because `PgLocalSessionStore` and/or its builders do not exist yet.

**Step 3: Commit nothing yet**

Do not commit at RED.

---

### Task 2: Implement the Postgres local-session store and builder selection

**Objective:** Make the new durable session-store tests pass with minimal production code.

**Files:**
- Modify: `crates/api/src/auth.rs`
- Modify: `crates/api/src/main.rs`

**Step 1: Write minimal implementation**

In `crates/api/src/auth.rs`:
- add `PgLocalSessionStore { pool: sqlx::PgPool }`
- add async constructors `connect` and/or `connect_lazy` consistent with the existing storage patterns
- implement `LocalSessionStore` for it using SQL queries against `sessions(id, user_id, secret_hash, created_at)`
- keep `FileLocalSessionStore` untouched as the fallback path
- change `build_local_session_store(...)` to accept both the state path and `database_url: Option<&str>` and return the Postgres store when configured, file store otherwise

In `crates/api/src/main.rs`:
- pass `config.database_url.as_deref()` into `build_local_session_store(...)`
- update helper/test-app construction sites to compile with the new builder signature

**Step 2: Run focused tests to verify GREEN**

Run:
```bash
DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api pg_local_session_store_ -- --nocapture
```
Expected: PASS.

**Step 3: Run broader auth/session regressions**

Run:
```bash
cargo test -p sourcebot-api --bin sourcebot-api login_returns_created_session_and_persists_hashed_secret_for_bootstrap_admin -- --nocapture
cargo test -p sourcebot-api --bin sourcebot-api auth_me_returns_bootstrap_admin_for_valid_bearer_local_session -- --nocapture
cargo test -p sourcebot-api --bin sourcebot-api logout_revokes_only_the_current_bearer_session -- --nocapture
cargo test -p sourcebot-api --bin sourcebot-api revoke_explicitly_targets_requested_local_session_and_fails_closed -- --nocapture
```
Expected: PASS.

---

### Task 3: Add one router-level durable-session regression

**Objective:** Prove the real app wiring uses the Postgres session store when `DATABASE_URL` is present.

**Files:**
- Modify: `crates/api/src/main.rs`

**Step 1: Write a failing app-level regression**

Add a test that builds the app with:
- a real `database_url`,
- a bootstrap-state file for the seeded bootstrap admin,
- a throwaway local-session file path that should stay unused for persistence,

then verifies:
1. `POST /api/v1/auth/login` returns `201`,
2. the returned session authenticates against `GET /api/v1/auth/me`,
3. the `sessions` table contains the new row,
4. the file-backed session path was not required to read the durable session back.

**Step 2: Run targeted test to verify RED then GREEN**

Run:
```bash
DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api database_url_config_uses_postgres_backed_local_sessions_for_login_and_auth_me -- --nocapture
```
Expected: first FAIL, then PASS after wiring is complete.

**Step 3: Run auth suite confidence signal**

Run:
```bash
DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api auth_ -- --nocapture
```
Expected: PASS.

---

### Task 4: Update operator/docs truthfulness for the shipped baseline

**Objective:** Make the acceptance and README wording match the narrower durable-session reality.

**Files:**
- Modify: `README.md`
- Modify: `specs/acceptance/auth.md`
- Modify: `docs/reports/2026-04-18-parity-gap-report.md`

**Step 1: Patch the docs**

Document exactly this truth:
- when `DATABASE_URL` is configured, local-session persistence now uses PostgreSQL,
- bootstrap state and broader organization/auth aggregates are **not** fully durable yet,
- Task 87 remains open beyond this slice.

**Step 2: Run raw-content truth checks**

Use a repo-local Python assertion command that checks the exact wording you added, for example:
```bash
python3 - <<'PY'
from pathlib import Path
checks = {
    'README.md': ['local sessions now persist in PostgreSQL when DATABASE_URL is configured'],
    'specs/acceptance/auth.md': ['Users can authenticate and receive a durable session when DATABASE_URL-backed session storage is configured'],
    'docs/reports/2026-04-18-parity-gap-report.md': ['local-session durability now exists on PostgreSQL while broader durable auth/org state remains follow-up work'],
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

### Task 5: Run full verification, review, and close the slice

**Objective:** Verify the slice end-to-end, get independent review, and prepare it for commit/state/push.

**Files:**
- Modify: `Makefile`
- Review: `crates/api/src/auth.rs`, `crates/api/src/main.rs`, `README.md`, `specs/acceptance/auth.md`, `docs/reports/2026-04-18-parity-gap-report.md`

**Step 1: Keep the Makefile truthful**

If `make sqlx-test` should now cover the new durable-session tests, extend it so the command actually exercises them instead of only `storage::tests`.

**Step 2: Run verification**

Run:
```bash
make sqlx-test
DATABASE_URL="$TEST_DATABASE_URL" cargo test -p sourcebot-api --bin sourcebot-api auth_ -- --nocapture
cargo test -p sourcebot-api --bin sourcebot-api login_returns_created_session_and_persists_hashed_secret_for_bootstrap_admin -- --nocapture
cargo test -p sourcebot-api --bin sourcebot-api logout_revokes_only_the_current_bearer_session -- --nocapture
git diff --check
```
Expected: PASS.

**Step 3: Run pre-commit review pipeline**

Use the requesting-code-review workflow:
1. inspect `git diff --cached` / `git diff`,
2. run the added-lines security scan,
3. run independent review,
4. fix any blocking findings,
5. re-run verification until clean.

**Step 4: Commit**

```bash
git add crates/api/src/auth.rs crates/api/src/main.rs Makefile README.md specs/acceptance/auth.md docs/reports/2026-04-18-parity-gap-report.md docs/plans/2026-04-22-task87a-durable-session-store-plan.md
git commit -m "feat: add durable postgres local session store"
```
