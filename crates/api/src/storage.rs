use anyhow::Result;
use sourcebot_core::{build_repository_detail, CatalogStore};
use sourcebot_models::{
    seed_connections, seed_repositories, Connection, Repository, RepositoryDetail,
    RepositorySummary,
};
use std::sync::Arc;
use tracing::warn;

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

impl CatalogStore for InMemoryCatalogStore {
    fn list_repositories(&self) -> Result<Vec<RepositorySummary>> {
        Ok(self.repositories.iter().map(Repository::summary).collect())
    }

    fn get_repository_detail(&self, repo_id: &str) -> Result<Option<RepositoryDetail>> {
        build_repository_detail(&self.repositories, &self.connections, repo_id)
    }
}

#[allow(dead_code)]
pub struct PgCatalogStore {
    pool: sqlx::PgPool,
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

impl CatalogStore for PgCatalogStore {
    fn list_repositories(&self) -> Result<Vec<RepositorySummary>> {
        anyhow::bail!(
            "postgres catalog store list_repositories is not implemented yet for pool {}",
            self.pool
                .connect_options()
                .get_database()
                .unwrap_or("<unknown>")
        )
    }

    fn get_repository_detail(&self, repo_id: &str) -> Result<Option<RepositoryDetail>> {
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
    fn in_memory_store_uses_provided_catalog_data() {
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
            store.list_repositories().unwrap(),
            vec![RepositorySummary {
                id: "repo_custom".into(),
                name: "custom".into(),
                default_branch: "develop".into(),
                sync_state: SyncState::Ready,
            }]
        );

        assert_eq!(
            store.get_repository_detail("repo_custom").unwrap(),
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
        let repositories = store.list_repositories().unwrap();

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
        let repositories = store.list_repositories().unwrap();

        assert!(repositories
            .iter()
            .any(|repository| repository.id == "repo_sourcebot_rewrite"));
    }
}
