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
}
