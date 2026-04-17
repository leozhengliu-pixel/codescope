use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionKind {
    GitHub,
    GitLab,
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
    Pending,
    Ready,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AskThreadVisibility {
    Private,
    Shared,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AskMessageRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Connection {
    pub id: String,
    pub name: String,
    pub kind: ConnectionKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Repository {
    pub id: String,
    pub name: String,
    pub default_branch: String,
    pub connection_id: String,
    pub sync_state: SyncState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositorySummary {
    pub id: String,
    pub name: String,
    pub default_branch: String,
    pub sync_state: SyncState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositoryDetail {
    pub repository: Repository,
    pub connection: Connection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BootstrapStatus {
    pub bootstrap_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BootstrapState {
    pub initialized_at: String,
    pub admin_email: String,
    pub admin_name: String,
    pub password_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalSession {
    pub id: String,
    pub user_id: String,
    pub secret_hash: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalSessionState {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sessions: Vec<LocalSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationRole {
    Admin,
    Viewer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Organization {
    pub id: String,
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrganizationMembership {
    pub organization_id: String,
    pub user_id: String,
    pub role: OrganizationRole,
    pub joined_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalAccount {
    pub id: String,
    pub email: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrganizationInvite {
    pub id: String,
    pub organization_id: String,
    pub email: String,
    pub role: OrganizationRole,
    pub invited_by_user_id: String,
    pub created_at: String,
    pub expires_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_by_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiKey {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub secret_hash: String,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repo_scope: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositoryPermissionBinding {
    pub organization_id: String,
    pub repository_id: String,
    pub synced_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrganizationState {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub organizations: Vec<Organization>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memberships: Vec<OrganizationMembership>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accounts: Vec<LocalAccount>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invites: Vec<OrganizationInvite>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub api_keys: Vec<ApiKey>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repo_permissions: Vec<RepositoryPermissionBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AskCitation {
    pub repo_id: String,
    pub path: String,
    pub revision: String,
    pub line_start: usize,
    pub line_end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AskRenderedCitation {
    pub repo_id: String,
    pub path: String,
    pub revision: String,
    pub line_start: usize,
    pub line_end: usize,
    pub display_label: String,
    pub pinned_location: String,
    pub line_fragment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AskMessage {
    pub id: String,
    pub role: AskMessageRole,
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub citations: Vec<AskCitation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AskThread {
    pub id: String,
    pub session_id: String,
    pub user_id: String,
    pub title: String,
    pub repo_scope: Vec<String>,
    pub visibility: AskThreadVisibility,
    pub created_at: String,
    pub updated_at: String,
    pub messages: Vec<AskMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AskThreadSummary {
    pub id: String,
    pub session_id: String,
    pub title: String,
    pub repo_scope: Vec<String>,
    pub visibility: AskThreadVisibility,
    pub updated_at: String,
    pub message_count: usize,
}

impl Repository {
    pub fn summary(&self) -> RepositorySummary {
        RepositorySummary {
            id: self.id.clone(),
            name: self.name.clone(),
            default_branch: self.default_branch.clone(),
            sync_state: self.sync_state.clone(),
        }
    }
}

impl OrganizationMembership {
    pub fn can_manage_members(&self) -> bool {
        matches!(self.role, OrganizationRole::Admin)
    }
}

impl AskThread {
    pub fn summary(&self) -> AskThreadSummary {
        AskThreadSummary {
            id: self.id.clone(),
            session_id: self.session_id.clone(),
            title: self.title.clone(),
            repo_scope: self.repo_scope.clone(),
            visibility: self.visibility.clone(),
            updated_at: self.updated_at.clone(),
            message_count: self.messages.len(),
        }
    }
}

impl AskCitation {
    pub fn rendered(&self) -> AskRenderedCitation {
        AskRenderedCitation {
            repo_id: self.repo_id.clone(),
            path: self.path.clone(),
            revision: self.revision.clone(),
            line_start: self.line_start,
            line_end: self.line_end,
            display_label: self.display_label(),
            pinned_location: self.pinned_location(),
            line_fragment: self.line_fragment(),
        }
    }

    pub fn line_fragment(&self) -> String {
        if self.line_start == self.line_end {
            format!("L{}", self.line_start)
        } else {
            format!("L{}-L{}", self.line_start, self.line_end)
        }
    }

    pub fn display_label(&self) -> String {
        if self.line_start == self.line_end {
            format!("{}:{}", self.path, self.line_start)
        } else {
            format!("{}:{}-{}", self.path, self.line_start, self.line_end)
        }
    }

    pub fn pinned_location(&self) -> String {
        format!("{}:{}#{}", self.revision, self.path, self.line_fragment())
    }
}

pub fn seed_connections() -> Vec<Connection> {
    vec![
        Connection {
            id: "conn_github".into(),
            name: "GitHub Cloud".into(),
            kind: ConnectionKind::GitHub,
        },
        Connection {
            id: "conn_local".into(),
            name: "Local Mirrors".into(),
            kind: ConnectionKind::Local,
        },
    ]
}

pub fn seed_repositories() -> Vec<Repository> {
    vec![
        Repository {
            id: "repo_sourcebot_rewrite".into(),
            name: "sourcebot-rewrite".into(),
            default_branch: "main".into(),
            connection_id: "conn_local".into(),
            sync_state: SyncState::Ready,
        },
        Repository {
            id: "repo_demo_docs".into(),
            name: "demo-docs".into(),
            default_branch: "main".into(),
            connection_id: "conn_github".into(),
            sync_state: SyncState::Pending,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ask_thread_summary_counts_messages_and_preserves_scope_metadata() {
        let thread = AskThread {
            id: "thread_1".into(),
            session_id: "session_1".into(),
            user_id: "user_1".into(),
            title: "Healthz".into(),
            repo_scope: vec!["repo_sourcebot_rewrite".into()],
            visibility: AskThreadVisibility::Private,
            created_at: "2026-04-16T08:00:00Z".into(),
            updated_at: "2026-04-16T08:01:00Z".into(),
            messages: vec![
                AskMessage {
                    id: "msg_1".into(),
                    role: AskMessageRole::User,
                    content: "where is healthz implemented?".into(),
                    citations: Vec::new(),
                },
                AskMessage {
                    id: "msg_2".into(),
                    role: AskMessageRole::Assistant,
                    content: "src/main.rs".into(),
                    citations: Vec::new(),
                },
            ],
        };

        assert_eq!(
            thread.summary(),
            AskThreadSummary {
                id: "thread_1".into(),
                session_id: "session_1".into(),
                title: "Healthz".into(),
                repo_scope: vec!["repo_sourcebot_rewrite".into()],
                visibility: AskThreadVisibility::Private,
                updated_at: "2026-04-16T08:01:00Z".into(),
                message_count: 2,
            }
        );
    }

    #[test]
    fn ask_message_serialization_includes_machine_readable_citations() {
        let message = AskMessage {
            id: "msg_1".into(),
            role: AskMessageRole::Assistant,
            content: "healthz lives in crates/api/src/main.rs".into(),
            citations: vec![AskCitation {
                repo_id: "repo_sourcebot_rewrite".into(),
                path: "crates/api/src/main.rs".into(),
                revision: "main".into(),
                line_start: 10,
                line_end: 18,
            }],
        };

        let serialized = serde_json::to_value(&message).unwrap();

        assert_eq!(
            serialized,
            json!({
                "id": "msg_1",
                "role": "assistant",
                "content": "healthz lives in crates/api/src/main.rs",
                "citations": [{
                    "repo_id": "repo_sourcebot_rewrite",
                    "path": "crates/api/src/main.rs",
                    "revision": "main",
                    "line_start": 10,
                    "line_end": 18
                }]
            })
        );
    }

    #[test]
    fn ask_citation_renders_display_labels_for_single_lines_and_ranges() {
        let multi_line = AskCitation {
            repo_id: "repo_sourcebot_rewrite".into(),
            path: "crates/api/src/main.rs".into(),
            revision: "main".into(),
            line_start: 10,
            line_end: 18,
        };
        let single_line = AskCitation {
            repo_id: "repo_sourcebot_rewrite".into(),
            path: "crates/api/src/main.rs".into(),
            revision: "main".into(),
            line_start: 42,
            line_end: 42,
        };

        assert_eq!(multi_line.display_label(), "crates/api/src/main.rs:10-18");
        assert_eq!(single_line.display_label(), "crates/api/src/main.rs:42");
    }

    #[test]
    fn ask_citation_renders_pinned_locations_with_revision_and_line_fragments() {
        let citation = AskCitation {
            repo_id: "repo_sourcebot_rewrite".into(),
            path: "crates/api/src/main.rs".into(),
            revision: "main".into(),
            line_start: 10,
            line_end: 18,
        };

        assert_eq!(citation.line_fragment(), "L10-L18");
        assert_eq!(
            citation.pinned_location(),
            "main:crates/api/src/main.rs#L10-L18"
        );
    }

    #[test]
    fn ask_citation_rendered_payload_includes_machine_and_human_facing_fields() {
        let citation = AskCitation {
            repo_id: "repo_sourcebot_rewrite".into(),
            path: "crates/api/src/main.rs".into(),
            revision: "main".into(),
            line_start: 10,
            line_end: 18,
        };

        assert_eq!(
            serde_json::to_value(citation.rendered()).unwrap(),
            json!({
                "repo_id": "repo_sourcebot_rewrite",
                "path": "crates/api/src/main.rs",
                "revision": "main",
                "line_start": 10,
                "line_end": 18,
                "display_label": "crates/api/src/main.rs:10-18",
                "pinned_location": "main:crates/api/src/main.rs#L10-L18",
                "line_fragment": "L10-L18"
            })
        );
    }

    #[test]
    fn organization_membership_management_tracks_role_capabilities() {
        let admin_membership = OrganizationMembership {
            organization_id: "org_acme".into(),
            user_id: "user_admin".into(),
            role: OrganizationRole::Admin,
            joined_at: "2026-04-16T20:00:00Z".into(),
        };
        let viewer_membership = OrganizationMembership {
            organization_id: "org_acme".into(),
            user_id: "user_viewer".into(),
            role: OrganizationRole::Viewer,
            joined_at: "2026-04-16T20:01:00Z".into(),
        };

        assert!(admin_membership.can_manage_members());
        assert!(!viewer_membership.can_manage_members());
    }

    #[test]
    fn organization_state_defaults_to_empty_collections_and_serializes_cleanly() {
        let state = OrganizationState::default();

        assert!(state.organizations.is_empty());
        assert!(state.memberships.is_empty());
        assert!(state.accounts.is_empty());
        assert!(state.invites.is_empty());
        assert!(state.api_keys.is_empty());
        assert_eq!(serde_json::to_value(&state).unwrap(), json!({}));
    }

    #[test]
    fn organization_state_serializes_api_keys_for_persistence() {
        let state = OrganizationState {
            organizations: vec![Organization {
                id: "org_acme".into(),
                slug: "acme".into(),
                name: "Acme".into(),
            }],
            api_keys: vec![ApiKey {
                id: "key_123".into(),
                user_id: "user_admin".into(),
                name: "CI token".into(),
                secret_hash: "hashed-secret".into(),
                created_at: "2026-04-18T10:00:00Z".into(),
                revoked_at: None,
                repo_scope: vec!["repo_sourcebot_rewrite".into(), "repo_demo_docs".into()],
            }],
            ..OrganizationState::default()
        };

        assert_eq!(
            serde_json::to_value(&state).unwrap(),
            json!({
                "organizations": [{
                    "id": "org_acme",
                    "slug": "acme",
                    "name": "Acme"
                }],
                "api_keys": [{
                    "id": "key_123",
                    "user_id": "user_admin",
                    "name": "CI token",
                    "secret_hash": "hashed-secret",
                    "created_at": "2026-04-18T10:00:00Z",
                    "repo_scope": ["repo_sourcebot_rewrite", "repo_demo_docs"]
                }]
            })
        );
    }

    #[test]
    fn organization_state_defaults_api_keys_to_empty_on_deserialize() {
        let state: OrganizationState = serde_json::from_value(json!({
            "organizations": [{
                "id": "org_acme",
                "slug": "acme",
                "name": "Acme"
            }]
        }))
        .unwrap();

        assert!(state.api_keys.is_empty());
    }

    #[test]
    fn organization_models_round_trip_as_reusable_domain_data() {
        let state = OrganizationState {
            organizations: vec![Organization {
                id: "org_acme".into(),
                slug: "acme".into(),
                name: "Acme".into(),
            }],
            memberships: vec![OrganizationMembership {
                organization_id: "org_acme".into(),
                user_id: "user_admin".into(),
                role: OrganizationRole::Admin,
                joined_at: "2026-04-16T20:00:00Z".into(),
            }],
            accounts: vec![LocalAccount {
                id: "user_admin".into(),
                email: "admin@example.com".into(),
                name: "Admin User".into(),
                created_at: "2026-04-16T19:59:00Z".into(),
            }],
            invites: vec![OrganizationInvite {
                id: "invite_123".into(),
                organization_id: "org_acme".into(),
                email: "invitee@example.com".into(),
                role: OrganizationRole::Viewer,
                invited_by_user_id: "user_admin".into(),
                created_at: "2026-04-16T20:05:00Z".into(),
                expires_at: "2026-04-23T20:05:00Z".into(),
                accepted_by_user_id: None,
                accepted_at: None,
            }],
            api_keys: Vec::new(),
            repo_permissions: Vec::new(),
        };

        assert_eq!(
            serde_json::to_value(&state).unwrap(),
            json!({
                "organizations": [{
                    "id": "org_acme",
                    "slug": "acme",
                    "name": "Acme"
                }],
                "memberships": [{
                    "organization_id": "org_acme",
                    "user_id": "user_admin",
                    "role": "admin",
                    "joined_at": "2026-04-16T20:00:00Z"
                }],
                "accounts": [{
                    "id": "user_admin",
                    "email": "admin@example.com",
                    "name": "Admin User",
                    "created_at": "2026-04-16T19:59:00Z"
                }],
                "invites": [{
                    "id": "invite_123",
                    "organization_id": "org_acme",
                    "email": "invitee@example.com",
                    "role": "viewer",
                    "invited_by_user_id": "user_admin",
                    "created_at": "2026-04-16T20:05:00Z",
                    "expires_at": "2026-04-23T20:05:00Z"
                }]
            })
        );
    }

    #[test]
    fn organization_invite_tracks_acceptance_without_extra_status_fields() {
        let invite = OrganizationInvite {
            id: "invite_accepted".into(),
            organization_id: "org_acme".into(),
            email: "member@example.com".into(),
            role: OrganizationRole::Admin,
            invited_by_user_id: "user_admin".into(),
            created_at: "2026-04-16T20:05:00Z".into(),
            expires_at: "2026-04-23T20:05:00Z".into(),
            accepted_by_user_id: Some("user_member".into()),
            accepted_at: Some("2026-04-17T09:00:00Z".into()),
        };

        assert_eq!(
            serde_json::to_value(&invite).unwrap(),
            json!({
                "id": "invite_accepted",
                "organization_id": "org_acme",
                "email": "member@example.com",
                "role": "admin",
                "invited_by_user_id": "user_admin",
                "created_at": "2026-04-16T20:05:00Z",
                "expires_at": "2026-04-23T20:05:00Z",
                "accepted_by_user_id": "user_member",
                "accepted_at": "2026-04-17T09:00:00Z"
            })
        );
    }

    #[test]
    fn organization_state_serializes_repo_permission_bindings_for_persistence() {
        let state = OrganizationState {
            organizations: vec![Organization {
                id: "org_acme".into(),
                slug: "acme".into(),
                name: "Acme".into(),
            }],
            repo_permissions: vec![RepositoryPermissionBinding {
                organization_id: "org_acme".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                synced_at: "2026-04-18T09:30:00Z".into(),
            }],
            ..OrganizationState::default()
        };

        assert_eq!(
            serde_json::to_value(&state).unwrap(),
            json!({
                "organizations": [{
                    "id": "org_acme",
                    "slug": "acme",
                    "name": "Acme"
                }],
                "repo_permissions": [{
                    "organization_id": "org_acme",
                    "repository_id": "repo_sourcebot_rewrite",
                    "synced_at": "2026-04-18T09:30:00Z"
                }]
            })
        );
    }

    #[test]
    fn organization_state_defaults_repo_permissions_to_empty_on_deserialize() {
        let state: OrganizationState = serde_json::from_value(json!({
            "organizations": [{
                "id": "org_acme",
                "slug": "acme",
                "name": "Acme"
            }]
        }))
        .unwrap();

        assert!(state.repo_permissions.is_empty());
    }
}
