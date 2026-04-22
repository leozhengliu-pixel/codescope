use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use sourcebot_core::{build_repository_detail, CatalogStore, ImportRepositoryResult};
use sourcebot_models::{
    seed_connections, seed_repositories, Connection, Repository, RepositoryDetail,
    RepositorySummary, SyncState,
};
use sqlx::migrate::Migrator;
use std::{
    path::Path,
    process::Command,
    sync::{Arc, RwLock},
};
use tracing::warn;

static CATALOG_MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub type DynCatalogStore = Arc<dyn CatalogStore>;

#[derive(Debug, Clone)]
struct CatalogImportEntry {
    repository: Repository,
    connection: Connection,
    canonical_path: Option<String>,
}

#[derive(Debug, Clone)]
struct CatalogState {
    entries: Vec<CatalogImportEntry>,
}

#[derive(Clone)]
pub struct InMemoryCatalogStore {
    state: Arc<RwLock<CatalogState>>,
}

impl InMemoryCatalogStore {
    pub fn new(repositories: Vec<Repository>, connections: Vec<Connection>) -> Self {
        let entries = repositories
            .into_iter()
            .map(|repository| {
                let connection = connections
                    .iter()
                    .find(|connection| connection.id == repository.connection_id)
                    .cloned()
                    .unwrap_or_else(|| panic!("missing connection {}", repository.connection_id));
                CatalogImportEntry {
                    repository,
                    connection,
                    canonical_path: None,
                }
            })
            .collect();

        Self {
            state: Arc::new(RwLock::new(CatalogState { entries })),
        }
    }

    pub fn seeded() -> Self {
        Self::new(seed_repositories(), seed_connections())
    }
}

fn git_output(path: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .with_context(|| format!("failed to execute git {}", args.join(" ")))?;

    if !output.status.success() {
        return Err(anyhow!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn canonical_local_git_repository_path(repo_path: &str) -> Result<std::path::PathBuf> {
    let canonical_path = std::fs::canonicalize(repo_path)
        .with_context(|| format!("local repository path does not exist: {repo_path}"))?;

    if !canonical_path.is_dir() {
        anyhow::bail!("local repository path must be a directory: {repo_path}");
    }

    let is_git_repo = git_output(&canonical_path, &["rev-parse", "--is-inside-work-tree"])?;
    if is_git_repo != "true" {
        anyhow::bail!("local repository path must be a git repository: {repo_path}");
    }

    let repository_root = std::path::PathBuf::from(git_output(
        &canonical_path,
        &["rev-parse", "--show-toplevel"],
    )?);
    let canonical_repository_root = std::fs::canonicalize(&repository_root)
        .with_context(|| format!("failed to resolve repository root for {repo_path}"))?;
    if canonical_repository_root != canonical_path {
        anyhow::bail!("local repository path must point at the repository root: {repo_path}");
    }

    Ok(canonical_path)
}

fn repository_name_from_path(path: &Path) -> Result<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("local repository path must end with a repository directory name"))
}

fn sanitize_repo_id_component(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();

    let trimmed = sanitized.trim_matches('_');
    if trimmed.is_empty() {
        "local".into()
    } else {
        trimmed.into()
    }
}

#[async_trait]
impl CatalogStore for InMemoryCatalogStore {
    fn supports_local_repository_import(&self) -> bool {
        true
    }

    async fn list_repositories(&self) -> Result<Vec<RepositorySummary>> {
        Ok(self
            .state
            .read()
            .expect("catalog state lock poisoned")
            .entries
            .iter()
            .map(|entry| entry.repository.summary())
            .collect())
    }

    async fn get_repository_detail(&self, repo_id: &str) -> Result<Option<RepositoryDetail>> {
        let state = self.state.read().expect("catalog state lock poisoned");
        let repositories = state
            .entries
            .iter()
            .map(|entry| entry.repository.clone())
            .collect::<Vec<_>>();
        let connections = state
            .entries
            .iter()
            .map(|entry| entry.connection.clone())
            .collect::<Vec<_>>();
        build_repository_detail(&repositories, &connections, repo_id)
    }

    async fn import_local_repository(
        &self,
        connection: Connection,
        repo_path: &str,
    ) -> Result<ImportRepositoryResult> {
        let canonical_path = canonical_local_git_repository_path(repo_path)?;
        let canonical_path_string = canonical_path.display().to_string();
        let default_branch = git_output(&canonical_path, &["symbolic-ref", "--short", "HEAD"])?;
        let repository_name = repository_name_from_path(&canonical_path)?;
        let repo_id_base = format!(
            "repo_local_{}",
            sanitize_repo_id_component(&repository_name)
        );

        let mut state = self.state.write().expect("catalog state lock poisoned");
        if let Some(existing_entry) = state.entries.iter().find(|entry| {
            entry.connection.id == connection.id
                && entry.canonical_path.as_deref() == Some(canonical_path_string.as_str())
        }) {
            return Ok(ImportRepositoryResult {
                detail: RepositoryDetail {
                    repository: existing_entry.repository.clone(),
                    connection: existing_entry.connection.clone(),
                },
                created: false,
            });
        }

        let mut repo_id = repo_id_base.clone();
        let mut suffix = 2usize;
        while state
            .entries
            .iter()
            .any(|entry| entry.repository.id == repo_id)
        {
            repo_id = format!("{repo_id_base}_{suffix}");
            suffix += 1;
        }

        let repository = Repository {
            id: repo_id,
            name: repository_name,
            default_branch,
            connection_id: connection.id.clone(),
            sync_state: SyncState::Ready,
        };
        let detail = RepositoryDetail {
            repository: repository.clone(),
            connection: connection.clone(),
        };
        state.entries.push(CatalogImportEntry {
            repository,
            connection,
            canonical_path: Some(canonical_path_string),
        });

        Ok(ImportRepositoryResult {
            detail,
            created: true,
        })
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

    pub fn connect_lazy(database_url: &str) -> Result<Self> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect_lazy(database_url)?;
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

    async fn import_local_repository(
        &self,
        connection: Connection,
        repo_path: &str,
    ) -> Result<ImportRepositoryResult> {
        anyhow::bail!(
            "postgres catalog store import_local_repository is not implemented yet for connection {} and path {}",
            connection.id,
            repo_path
        )
    }
}

pub async fn build_catalog_store(database_url: Option<&str>) -> Result<DynCatalogStore> {
    if let Some(database_url) = database_url {
        warn!(
            "DATABASE_URL is configured; using lazy PgCatalogStore skeleton until catalog queries are implemented"
        );

        return Ok(Arc::new(PgCatalogStore::connect_lazy(database_url)?));
    }

    Ok(Arc::new(InMemoryCatalogStore::seeded()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sourcebot_models::{ConnectionKind, SyncState};

    #[test]
    fn catalog_migration_inventory_bootstraps_catalog_org_repository_permissions_sessions_ask_threads_review_agent_runs_delivery_attempts_and_task87b1_local_account_password_hash(
    ) {
        let migrations = catalog_migrator().iter().collect::<Vec<_>>();
        let migration_versions = migrations
            .iter()
            .map(|migration| migration.version)
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(
            migration_versions,
            [1, 2, 3, 4, 5, 6, 7, 8].into_iter().collect(),
            "expected only the task05a + task05b1 + task05b2 + task05b3 + task05b4 + task05b5 + task05b6 + task87b1 migration versions"
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
                "0005_ask_threads.down.sql".to_string(),
                "0005_ask_threads.up.sql".to_string(),
                "0006_review_agent_runs.down.sql".to_string(),
                "0006_review_agent_runs.up.sql".to_string(),
                "0007_delivery_attempts.down.sql".to_string(),
                "0007_delivery_attempts.up.sql".to_string(),
                "0008_local_account_password_hash.down.sql".to_string(),
                "0008_local_account_password_hash.up.sql".to_string(),
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

        let task05b4_up_migration =
            std::fs::read_to_string(migration_dir.join("0005_ask_threads.up.sql")).unwrap();

        for expected_snippet in [
            "CREATE TABLE ask_threads",
            "id TEXT PRIMARY KEY",
            "session_id TEXT NOT NULL REFERENCES sessions(id)",
            "user_id TEXT NOT NULL REFERENCES local_accounts(id)",
            "title TEXT NOT NULL",
            "repo_scope TEXT[] NOT NULL",
            "visibility TEXT NOT NULL",
            "CONSTRAINT ask_threads_visibility_check",
            "CHECK (visibility IN ('private', 'shared'))",
            "created_at TIMESTAMPTZ NOT NULL",
            "updated_at TIMESTAMPTZ NOT NULL",
            "UNIQUE (user_id, session_id)",
        ] {
            assert!(
                task05b4_up_migration.contains(expected_snippet),
                "missing task05b4 migration snippet: {expected_snippet}"
            );
        }

        let task05b5_up_migration =
            std::fs::read_to_string(migration_dir.join("0006_review_agent_runs.up.sql")).unwrap();

        for expected_snippet in [
            "ALTER TABLE repositories",
            "ADD CONSTRAINT repositories_id_connection_id_unique UNIQUE (id, connection_id)",
            "CREATE TABLE review_agent_runs",
            "id TEXT PRIMARY KEY",
            "organization_id TEXT NOT NULL REFERENCES organizations(id)",
            "webhook_id TEXT NOT NULL",
            "delivery_attempt_id TEXT NOT NULL",
            "connection_id TEXT NOT NULL REFERENCES connections(id)",
            "repository_id TEXT NOT NULL REFERENCES repositories(id)",
            "review_id TEXT NOT NULL",
            "status TEXT NOT NULL DEFAULT 'queued'",
            "CONSTRAINT review_agent_runs_status_check",
            "CHECK (status IN ('queued', 'claimed', 'completed', 'failed'))",
            "CONSTRAINT review_agent_runs_repository_visibility_fk",
            "FOREIGN KEY (organization_id, repository_id)",
            "REFERENCES repository_permission_bindings(organization_id, repository_id)",
            "CONSTRAINT review_agent_runs_repository_connection_fk",
            "FOREIGN KEY (repository_id, connection_id)",
            "REFERENCES repositories(id, connection_id)",
            "created_at TIMESTAMPTZ NOT NULL",
        ] {
            assert!(
                task05b5_up_migration.contains(expected_snippet),
                "missing task05b5 migration snippet: {expected_snippet}"
            );
        }

        for unexpected_snippet in [
            "CREATE TABLE delivery_attempts",
            "CREATE TABLE api_keys",
            "CREATE TABLE oauth_clients",
            "CREATE TABLE analytics_events",
        ] {
            assert!(
                !task05b5_up_migration.contains(unexpected_snippet),
                "unexpected out-of-scope table present in 0006: {unexpected_snippet}"
            );
        }

        let task05b6_up_migration =
            std::fs::read_to_string(migration_dir.join("0007_delivery_attempts.up.sql")).unwrap();

        for expected_snippet in [
            "CREATE TABLE delivery_attempts",
            "id TEXT PRIMARY KEY",
            "webhook_id TEXT NOT NULL",
            "connection_id TEXT NOT NULL REFERENCES connections(id)",
            "repository_id TEXT NOT NULL REFERENCES repositories(id)",
            "event_type TEXT NOT NULL",
            "review_id TEXT NOT NULL",
            "external_event_id TEXT NOT NULL",
            "accepted_at TIMESTAMPTZ NOT NULL",
            "CONSTRAINT delivery_attempts_repository_connection_fk",
            "FOREIGN KEY (repository_id, connection_id)",
            "REFERENCES repositories(id, connection_id)",
        ] {
            assert!(
                task05b6_up_migration.contains(expected_snippet),
                "missing task05b6 migration snippet: {expected_snippet}"
            );
        }

        for unexpected_snippet in [
            "CREATE TABLE review_webhooks",
            "organization_id TEXT NOT NULL",
            "secret_hash TEXT NOT NULL",
            "created_by_user_id TEXT NOT NULL",
            "created_at TIMESTAMPTZ NOT NULL",
            "CREATE TABLE api_keys",
            "CREATE TABLE oauth_clients",
            "CREATE TABLE analytics_events",
        ] {
            assert!(
                !task05b6_up_migration.contains(unexpected_snippet),
                "unexpected out-of-scope table present in 0007: {unexpected_snippet}"
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
                config: None,
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
                    config: None,
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
    async fn build_catalog_store_with_database_uses_postgres_catalog_store_path() {
        let store = build_catalog_store(Some("postgres://sourcebot:***@127.0.0.1:5432/sourcebot"))
            .await
            .unwrap();
        let error = store.list_repositories().await.unwrap_err();

        assert!(
            error
                .to_string()
                .contains("postgres catalog store list_repositories is not implemented yet"),
            "expected postgres-backed catalog store error, got: {error:?}"
        );
    }
}
