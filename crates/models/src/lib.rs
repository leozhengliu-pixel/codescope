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
