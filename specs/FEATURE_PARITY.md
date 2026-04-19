# Feature Parity Matrix

This document preserves the existing feature inventory while converting it into a
stable row-per-feature matrix with explicit status and acceptance-evidence
placeholder columns for later parity-audit updates.

| Domain | Feature | Status | Acceptance evidence |
| --- | --- | --- | --- |
| Core user features | Cross-repo and cross-branch code search | Needs audit | _TBD_ |
| Core user features | Regex, literal, boolean, repo/language/path filters | Needs audit | _TBD_ |
| Core user features | File explorer with tree browsing and syntax highlighting | Needs audit | _TBD_ |
| Core user features | Repository page and repo list page | Needs audit | _TBD_ |
| Core user features | File source view | Needs audit | _TBD_ |
| Core user features | Commit list, commit detail, and diff view | Needs audit | _TBD_ |
| Core user features | Code navigation: definitions and references | Needs audit | _TBD_ |
| Core user features | Ask the codebase with inline citations | Needs audit | _TBD_ |
| Core user features | Chat threads, history, rename, visibility | Needs audit | _TBD_ |
| Core user features | Search contexts / saved scopes | Needs audit | _TBD_ |
| Admin and org features | First-run onboarding | Needs audit | _TBD_ |
| Admin and org features | Organizations, membership, invites, roles | Needs audit | _TBD_ |
| Admin and org features | API keys | Needs audit | _TBD_ |
| Admin and org features | Connection management | Needs audit | _TBD_ |
| Admin and org features | Sync state and indexing status | Needs audit | _TBD_ |
| Admin and org features | Linked external accounts | Needs audit | _TBD_ |
| Admin and org features | Access / permission sync | Needs audit | _TBD_ |
| Integrations | GitHub | Needs audit | _TBD_ |
| Integrations | GitLab | Needs audit | _TBD_ |
| Integrations | Gitea | Needs audit | _TBD_ |
| Integrations | Gerrit | Needs audit | _TBD_ |
| Integrations | Bitbucket | Needs audit | _TBD_ |
| Integrations | Azure DevOps | Needs audit | _TBD_ |
| Integrations | Generic Git host / local Git | Partial | `specs/acceptance/generic-local-git.md` now grounds the current baseline: shared `generic_git`/`local` connection kinds and configs, authenticated `/api/v1/auth/connections` CRUD, repo-detail connection metadata, and the limited `#/settings/connections` shell with local `repo_path` handling and read-only sync-history visibility. Real host enumeration/import/index parity remains open. |
| Integrations | OIDC / SSO providers | Needs audit | _TBD_ |
| Integrations | MCP server | Needs audit | _TBD_ |
| Integrations | Public REST API | Needs audit | _TBD_ |
| Later-phase advanced features | Audit logs | Needs audit | _TBD_ |
| Later-phase advanced features | Analytics | Needs audit | _TBD_ |
| Later-phase advanced features | OAuth client / token flows | Needs audit | _TBD_ |
| Later-phase advanced features | Review agent / webhook automation | Needs audit | _TBD_ |
| Later-phase advanced features | Enterprise entitlement controls | Needs audit | _TBD_ |
