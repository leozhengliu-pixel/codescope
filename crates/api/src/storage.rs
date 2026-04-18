use anyhow::Result;
use async_trait::async_trait;
use sourcebot_core::{build_repository_detail, CatalogStore};
use sourcebot_models::{
    seed_connections, seed_repositories, Connection, Repository, RepositoryDetail,
    RepositorySummary,
};
use sqlx::migrate::Migrator;
use std::sync::Arc;
use tracing::warn;

static CATALOG_MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub type DynCatalogStore = Arc<dyn CatalogStore>;

#[derive(Clone)]
pub struct InMemoryCatalogStore {
    repositories: Vec<Repository>,
    connections: Vec<Connection>,
}

impl InMemoryCatalogStore {
    pub fn new(repositories: Vec<Repository>, connections: Vec<Connection>) -> Self {
        Self {
            repositories,
            connections,
        }
    }

    pub fn seeded() -> Self {
        Self::new(seed_repositories(), seed_connections())
    }
}

#[async_trait]
impl CatalogStore for InMemoryCatalogStore {
    async fn list_repositories(&self) -> Result<Vec<RepositorySummary>> {
        Ok(self.repositories.iter().map(Repository::summary).collect())
    }

    async fn get_repository_detail(&self, repo_id: &str) -> Result<Option<RepositoryDetail>> {
        build_repository_detail(&self.repositories, &self.connections, repo_id)
    }
}

#[allow(dead_code)]
pub struct PgCatalogStore {
    pool: sqlx::PgPool,
}

pub fn catalog_migrator() -> &'static Migrator {
    &CATALOG_MIGRATOR
}

#[allow(dead_code)]
impl PgCatalogStore {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &sqlx::PgPool {
        &self.pool
    }
}

#[async_trait]
impl CatalogStore for PgCatalogStore {
    async fn list_repositories(&self) -> Result<Vec<RepositorySummary>> {
        anyhow::bail!(
            "postgres catalog store list_repositories is not implemented yet for pool {}",
            self.pool
                .connect_options()
                .get_database()
                .unwrap_or("<unknown>")
        )
    }

    async fn get_repository_detail(&self, repo_id: &str) -> Result<Option<RepositoryDetail>> {
        anyhow::bail!(
            "postgres catalog store get_repository_detail is not implemented yet for repo {repo_id}"
        )
    }
}

pub async fn build_catalog_store(database_url: Option<&str>) -> Result<DynCatalogStore> {
    if database_url.is_some() {
        warn!(
            "DATABASE_URL is configured, but PgCatalogStore is still a skeleton; falling back to seeded in-memory catalog store"
        );
    }

    Ok(Arc::new(InMemoryCatalogStore::seeded()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sourcebot_models::{ConnectionKind, SyncState};

    #[test]
    fn catalog_migration_inventory_bootstraps_catalog_org_repository_permissions_and_task05b3_sessions_only(
    ) {
        let migrations = catalog_migrator().iter().collect::<Vec<_>>();
        let migration_versions = migrations
            .iter()
            .map(|migration| migration.version)
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(
            migration_versions,
            [1, 2, 3, 4].into_iter().collect(),
            "expected only the task05a + task05b1 + task05b2 + task05b3 migration versions"
        );

        let migration_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
        let migration_files = std::fs::read_dir(&migration_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(
            migration_files,
            [
                "0001_catalog_metadata.down.sql".to_string(),
                "0001_catalog_metadata.up.sql".to_string(),
                "0002_org_metadata.down.sql".to_string(),
                "0002_org_metadata.up.sql".to_string(),
                "0003_repository_permission_bindings.down.sql".to_string(),
                "0003_repository_permission_bindings.up.sql".to_string(),
                "0004_sessions.down.sql".to_string(),
                "0004_sessions.up.sql".to_string(),
            ]
            .into_iter()
            .collect()
        );

        let up_migration =
            std::fs::read_to_string(migration_dir.join("0001_catalog_metadata.up.sql")).unwrap();

        for expected_snippet in [
            "CREATE TABLE connections",
            "id TEXT PRIMARY KEY",
            "name TEXT NOT NULL",
            "kind TEXT NOT NULL",
            "CONSTRAINT connections_kind_check",
            "CHECK (kind IN ('github', 'gitlab', 'local'))",
            "CREATE TABLE repositories",
            "default_branch TEXT NOT NULL",
            "connection_id TEXT NOT NULL REFERENCES connections(id)",
            "sync_state TEXT NOT NULL",
            "CONSTRAINT repositories_sync_state_check",
            "CHECK (sync_state IN ('pending', 'ready', 'error'))",
        ] {
            assert!(
                up_migration.contains(expected_snippet),
                "missing migration snippet: {expected_snippet}"
            );
        }

        for unexpected_snippet in [
            "CREATE TABLE repository_permission_bindings",
            "CREATE TABLE sessions",
            "CREATE TABLE ask_threads",
            "CREATE TABLE review_agent_runs",
        ] {
            assert!(
                !up_migration.contains(unexpected_snippet),
                "unexpected later-slice table present in 0001: {unexpected_snippet}"
            );
        }

        // Keep task05b1 limited to org/account/invite/membership schema only.
        let task05b1_up_migration =
            std::fs::read_to_string(migration_dir.join("0002_org_metadata.up.sql")).unwrap();

        for expected_snippet in [
            "CREATE TABLE organizations",
            "slug TEXT NOT NULL UNIQUE",
            "CREATE TABLE local_accounts",
            "email TEXT NOT NULL UNIQUE",
            "created_at TIMESTAMPTZ NOT NULL",
            "CREATE TABLE organization_memberships",
            "organization_id TEXT NOT NULL REFERENCES organizations(id)",
            "user_id TEXT NOT NULL REFERENCES local_accounts(id)",
            "role TEXT NOT NULL",
            "CONSTRAINT organization_memberships_role_check",
            "CHECK (role IN ('admin', 'viewer'))",
            "joined_at TIMESTAMPTZ NOT NULL",
            "PRIMARY KEY (organization_id, user_id)",
            "CREATE TABLE organization_invites",
            "organization_id TEXT NOT NULL REFERENCES organizations(id)",
            "invited_by_user_id TEXT NOT NULL REFERENCES local_accounts(id)",
            "accepted_by_user_id TEXT REFERENCES local_accounts(id)",
            "expires_at TIMESTAMPTZ NOT NULL",
            "accepted_at TIMESTAMPTZ",
            "CONSTRAINT organization_invites_role_check",
            "CHECK (role IN ('admin', 'viewer'))",
        ] {
            assert!(
                task05b1_up_migration.contains(expected_snippet),
                "missing task05b1 migration snippet: {expected_snippet}"
            );
        }

        for unexpected_snippet in [
            "CREATE TABLE repository_permission_bindings",
            "CREATE TABLE sessions",
            "CREATE TABLE ask_threads",
            "CREATE TABLE review_agent_runs",
            "CREATE TABLE api_keys",
            "CREATE TABLE oauth_clients",
            "CREATE TABLE analytics_events",
        ] {
            assert!(
                !task05b1_up_migration.contains(unexpected_snippet),
                "unexpected out-of-scope table present in 0002: {unexpected_snippet}"
            );
        }

        let task05b2_up_migration = std::fs::read_to_string(
            migration_dir.join("0003_repository_permission_bindings.up.sql"),
        )
        .unwrap();

        for expected_snippet in [
            "CREATE TABLE repository_permission_bindings",
            "organization_id TEXT NOT NULL REFERENCES organizations(id)",
            "repository_id TEXT NOT NULL REFERENCES repositories(id)",
            "synced_at TIMESTAMPTZ NOT NULL",
            "PRIMARY KEY (organization_id, repository_id)",
        ] {
            assert!(
                task05b2_up_migration.contains(expected_snippet),
                "missing task05b2 migration snippet: {expected_snippet}"
            );
        }

        for unexpected_snippet in [
            "CREATE TABLE sessions",
            "CREATE TABLE ask_threads",
            "CREATE TABLE review_agent_runs",
            "CREATE TABLE api_keys",
            "CREATE TABLE oauth_clients",
            "CREATE TABLE analytics_events",
        ] {
            assert!(
                !task05b2_up_migration.contains(unexpected_snippet),
                "unexpected out-of-scope table present in 0003: {unexpected_snippet}"
            );
        }

        let task05b3_up_migration =
            std::fs::read_to_string(migration_dir.join("0004_sessions.up.sql")).unwrap();

        for expected_snippet in [
            "CREATE TABLE sessions",
            "id TEXT PRIMARY KEY",
            "user_id TEXT NOT NULL REFERENCES local_accounts(id)",
            "secret_hash TEXT NOT NULL",
            "created_at TIMESTAMPTZ NOT NULL",
        ] {
            assert!(
                task05b3_up_migration.contains(expected_snippet),
                "missing task05b3 migration snippet: {expected_snippet}"
            );
        }

        for unexpected_snippet in [
            "CREATE TABLE ask_threads",
            "CREATE TABLE review_agent_runs",
            "CREATE TABLE api_keys",
            "CREATE TABLE oauth_clients",
            "CREATE TABLE analytics_events",
        ] {
            assert!(
                !task05b3_up_migration.contains(unexpected_snippet),
                "unexpected out-of-scope table present in 0004: {unexpected_snippet}"
            );
        }
    }

    #[tokio::test]
    async fn in_memory_store_uses_provided_catalog_data() {
        let store = InMemoryCatalogStore::new(
            vec![Repository {
                id: "repo_custom".into(),
                name: "custom".into(),
                default_branch: "develop".into(),
                connection_id: "conn_custom".into(),
                sync_state: SyncState::Ready,
            }],
            vec![Connection {
                id: "conn_custom".into(),
                name: "Custom GitHub".into(),
                kind: ConnectionKind::GitHub,
            }],
        );

        assert_eq!(
            store.list_repositories().await.unwrap(),
            vec![RepositorySummary {
                id: "repo_custom".into(),
                name: "custom".into(),
                default_branch: "develop".into(),
                sync_state: SyncState::Ready,
            }]
        );

        assert_eq!(
            store.get_repository_detail("repo_custom").await.unwrap(),
            Some(RepositoryDetail {
                repository: Repository {
                    id: "repo_custom".into(),
                    name: "custom".into(),
                    default_branch: "develop".into(),
                    connection_id: "conn_custom".into(),
                    sync_state: SyncState::Ready,
                },
                connection: Connection {
                    id: "conn_custom".into(),
                    name: "Custom GitHub".into(),
                    kind: ConnectionKind::GitHub,
                },
            })
        );
    }

    #[tokio::test]
    async fn build_catalog_store_without_database_uses_seeded_in_memory_store() {
        let store = build_catalog_store(None).await.unwrap();
        let repositories = store.list_repositories().await.unwrap();

        assert!(repositories
            .iter()
            .any(|repository| repository.id == "repo_sourcebot_rewrite"));
    }

    #[tokio::test]
    async fn build_catalog_store_with_database_still_uses_seeded_in_memory_store() {
        let store = build_catalog_store(Some(
            "postgres://sourcebot:sourcebot@127.0.0.1:5432/sourcebot",
        ))
        .await
        .unwrap();
        let repositories = store.list_repositories().await.unwrap();

        assert!(repositories
            .iter()
            .any(|repository| repository.id == "repo_sourcebot_rewrite"));
    }
}
