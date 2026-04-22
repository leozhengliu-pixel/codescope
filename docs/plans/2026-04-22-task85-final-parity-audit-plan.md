# Task 85 Final Parity Audit and Release Checklist Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Close Task 85 with one truthful docs-only audit closure by producing the final parity audit and release checklist, grounding every `specs/FEATURE_PARITY.md` row in shipped evidence, and explicitly recording why release parity is still not complete.

**Architecture:** Reuse the existing acceptance corpus and canonical parity gap report as the evidence base, then add one new final audit document that maps each feature row to code/tests/docs and release blockers. Update `specs/FEATURE_PARITY.md` in the same run so the matrix stops saying `Needs audit` / `_TBD_`, then verify the edited rows with raw-text checks, run the bounded smoke/tests backing the audit wording, and close the roadmap state truthfully.

**Tech Stack:** Markdown docs, raw-content verification, bash smoke scripts, git.

---

### Task 1: Create the final parity audit document

**Objective:** Add one audit document that maps every feature-parity row to shipped evidence and release blockers.

**Files:**
- Create: `docs/reports/2026-04-22-final-parity-audit-and-release-checklist.md`
- Read: `specs/FEATURE_PARITY.md`
- Read: `docs/reports/2026-04-18-parity-gap-report.md`
- Read: `specs/acceptance/*.md`

**Step 1: Gather evidence**

Read the current parity matrix, acceptance specs, and canonical parity gap report. Group the rows into the same domains used by the matrix.

**Step 2: Write the audit document**

Include:
- audit purpose and release verdict
- one table per matrix domain
- columns for feature, status, code/tests/docs evidence, and remaining blockers
- an explicit release checklist proving why parity is not yet releasable

**Step 3: Verify structure**

Run exact raw-text checks ensuring each major domain heading and release verdict exists.

**Step 4: Commit later with the matrix update**

Do not commit yet; this task closes together with the matrix update.

### Task 2: Replace matrix placeholders with grounded evidence

**Objective:** Update `specs/FEATURE_PARITY.md` so every row has a truthful status and evidence pointer instead of `Needs audit` / `_TBD_`.

**Files:**
- Modify: `specs/FEATURE_PARITY.md`
- Read: `docs/reports/2026-04-22-final-parity-audit-and-release-checklist.md`

**Step 1: Write failing truth check**

Use a raw-content assertion that fails if any matrix row still contains `Needs audit` or `_TBD_`.

**Step 2: Update the matrix**

Set each row to `Partial`, `Missing`, or `Complete` based on existing shipped evidence only. Point evidence cells at the new final audit doc plus the relevant acceptance/gap-report anchors.

**Step 3: Run truth check to verify pass**

Confirm the matrix no longer contains audit placeholders.

### Task 3: Verify, review, and close Task 85

**Objective:** Run bounded evidence verification, one independent review, then commit/push and update roadmap state.

**Files:**
- Modify: `docs/status/roadmap-state.yaml`

**Step 1: Run targeted verification**

Run:
- `python3` raw-content checks for the new audit doc, the matrix, and the task85 plan path
- `bash scripts/check_end_to_end_smoke_matrix_contract.sh /opt/data/projects/sourcebot-rewrite`
- `bash scripts/check_end_to_end_smoke_matrix.sh /opt/data/projects/sourcebot-rewrite`
- `git diff --check`

**Step 2: Run independent review**

Use one independent reviewer to check that the audit is truthful, does not overclaim parity, and that the matrix/audit/release checklist align with the live docs and roadmap.

**Step 3: Commit and close state**

Create a substantive docs commit, update `docs/status/roadmap-state.yaml`, optionally create a separate state commit only if justified, push, and stop.
