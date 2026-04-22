# Task 83 Credential Verification Burn Hardening Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Harden one bounded Task 83 security slice by making missing-or-corrupt local sessions, API keys, and public review-webhook credentials fail closed after the same expensive secret-verification burn used for valid records.

**Architecture:** Keep the existing Argon2-backed credential model and current HTTP status contracts, but centralize a reusable dummy verification burn so auth/session, API-key, and review-webhook paths do not skip secret verification work just because the persisted record is missing or malformed. Lock the behavior with focused Rust tests, then update the acceptance/report wording to describe the narrower security baseline truthfully without overclaiming broad production hardening.

**Tech Stack:** Rust, Axum, Tokio, Argon2/password-hash, cargo test, markdown acceptance/report docs.

---

### Task 1: Add RED tests for fail-closed credential-burn behavior

**Objective:** Prove the current code should treat missing or malformed persisted credential records as unauthorized only after executing the same secret-verification burn path.

**Files:**
- Modify: `crates/api/src/main.rs`
- Test: `crates/api/src/main.rs` (existing unit-test module)

**Step 1: Write failing tests**

Add focused tests for:
- `authenticate_local_session_record(...)` with a missing session id
- `authenticate_local_session_record(...)` with an invalid persisted session hash
- `authenticate_api_key_record(...)` with a missing API key id
- `authenticate_api_key_record(...)` with an invalid persisted API-key hash
- `intake_review_webhook_event(...)` with a malformed persisted webhook secret hash

Each test should assert the existing `401/UNAUTHORIZED` outcome and exercise the path that currently risks skipping or inconsistently handling the burn.

**Step 2: Run test to verify failure**

Run: `cargo test -p sourcebot-api auth_me_returns_401_for_missing_session_record review_webhook_event_intake_returns_401_for_invalid_persisted_secret_hash auth_api_key_helper_fails_closed_for_unknown_api_key -- --nocapture`
Expected: at least one FAIL showing the missing/malformed-record cases are not yet normalized through the shared burn logic.

**Step 3: Do not implement yet**

Stop after confirming RED.

**Step 4: Commit**

Do not commit in RED.

### Task 2: Implement the shared credential-burn helper minimally

**Objective:** Add one reusable helper that burns Argon2 verification work for session, API-key, and webhook-secret miss/corruption paths, then thread it through the three auth surfaces without changing public status codes.

**Files:**
- Modify: `crates/api/src/main.rs`
- Test: `crates/api/src/main.rs`

**Step 1: Write minimal implementation**

Add a small helper such as `verify_secret_or_burn_failure(...)` / `burn_secret_verification_check(...)` that:
- accepts the presented secret plus optional persisted hash text
- verifies against the real hash when parseable
- otherwise verifies against a deterministic dummy Argon2 hash
- returns `UNAUTHORIZED` on every failure path

Then use it from:
- `authenticate_local_session_record(...)`
- `authenticate_api_key_record(...)`
- `verify_review_webhook_secret(...)` / `intake_review_webhook_event(...)`

**Step 2: Run focused tests to verify GREEN**

Run: `cargo test -p sourcebot-api auth_me_returns_401_for_missing_session_record auth_me_returns_401_for_invalid_session_secret auth_api_key_helper_fails_closed_for_unknown_api_key auth_api_key_helper_fails_closed_for_invalid_persisted_hash review_webhook_event_intake_returns_401_for_invalid_persisted_secret_hash -- --nocapture`
Expected: PASS.

**Step 3: Run broader auth/integration regression coverage**

Run:
- `cargo test -p sourcebot-api auth_ -- --nocapture`
- `cargo test -p sourcebot-api review_webhook -- --nocapture`

Expected: PASS.

**Step 4: Refactor only if still green**

Keep helper naming/placement small and local to `main.rs`. Do not widen scope into unrelated auth redesign.

**Step 5: Commit**

```bash
git add crates/api/src/main.rs
git commit -m "feat: harden credential verification burn paths"
```

### Task 3: Ground the shipped slice in docs and report

**Objective:** Update the acceptance and parity docs so they truthfully describe this narrower Task 83 security baseline.

**Files:**
- Modify: `specs/acceptance/auth.md`
- Modify: `specs/acceptance/integrations.md`
- Modify: `docs/reports/2026-04-18-parity-gap-report.md`
- Modify: `docs/status/roadmap-state.yaml`

**Step 1: Patch auth acceptance wording**

Add one focused black-box note that local-session and API-key credential checks fail closed even when persisted records are missing/corrupt, without claiming broader session hardening.

**Step 2: Patch integrations acceptance wording**

Add one focused note that public review-webhook intake rejects invalid or corrupted secret material without exposing stored secret hashes.

**Step 3: Patch parity gap report wording**

Mention the newly landed credential-verification burn baseline under auth/admin or integrations evidence, while keeping the broader Task 83 hardening areas still partial.

**Step 4: Verify doc truthfulness**

Run targeted raw-content assertions and `git diff --check`.

**Step 5: Commit**

```bash
git add specs/acceptance/auth.md specs/acceptance/integrations.md docs/reports/2026-04-18-parity-gap-report.md docs/status/roadmap-state.yaml
git commit -m "chore: update task83 credential hardening state"
```
