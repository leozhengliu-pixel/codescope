use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use sourcebot_core::{build_repository_detail, CatalogStore, ImportRepositoryResult};
use sourcebot_models::{
    seed_connections, seed_repositories, Connection, ConnectionKind, Repository, RepositoryDetail,
    RepositorySummary, SyncState,
};
use sqlx::{migrate::Migrator, Row};
use std::{
    path::Path,
    process::Command,
    sync::{Arc, RwLock},
};

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

fn connection_kind_to_str(kind: &ConnectionKind) -> &'static str {
    match kind {
        ConnectionKind::GitHub => "github",
        ConnectionKind::GitLab => "gitlab",
        ConnectionKind::Gitea => "gitea",
        ConnectionKind::Gerrit => "gerrit",
        ConnectionKind::Bitbucket => "bitbucket",
        ConnectionKind::AzureDevOps => "azure_devops",
        ConnectionKind::GenericGit => "generic_git",
        ConnectionKind::Local => "local",
    }
}

fn parse_connection_kind(value: &str) -> Result<ConnectionKind> {
    match value {
        "github" => Ok(ConnectionKind::GitHub),
        "gitlab" => Ok(ConnectionKind::GitLab),
        "local" => Ok(ConnectionKind::Local),
        other => anyhow::bail!("unknown catalog connection kind `{other}`"),
    }
}

fn parse_sync_state(value: &str) -> Result<SyncState> {
    match value {
        "pending" => Ok(SyncState::Pending),
        "ready" => Ok(SyncState::Ready),
        "error" => Ok(SyncState::Error),
        other => anyhow::bail!("unknown catalog sync state `{other}`"),
    }
}

fn row_to_repository(row: &sqlx::postgres::PgRow) -> Result<Repository> {
    Ok(Repository {
        id: row.get("repo_id"),
        name: row.get("repo_name"),
        default_branch: row.get("default_branch"),
        connection_id: row.get("connection_id"),
        sync_state: parse_sync_state(row.get::<String, _>("sync_state").as_str())?,
    })
}

fn row_to_connection(row: &sqlx::postgres::PgRow) -> Result<Connection> {
    Ok(Connection {
        id: row.get("connection_id"),
        name: row.get("connection_name"),
        kind: parse_connection_kind(row.get::<String, _>("connection_kind").as_str())?,
        config: None,
    })
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
    fn supports_local_repository_import(&self) -> bool {
        true
    }

    async fn list_repositories(&self) -> Result<Vec<RepositorySummary>> {
        let rows = sqlx::query(
            "SELECT id AS repo_id, name AS repo_name, default_branch, connection_id, sync_state FROM repositories ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;

        rows.iter()
            .map(row_to_repository)
            .map(|repository| repository.map(|repository| repository.summary()))
            .collect()
    }

    async fn get_repository_detail(&self, repo_id: &str) -> Result<Option<RepositoryDetail>> {
        let Some(row) = sqlx::query(
            "SELECT r.id AS repo_id, r.name AS repo_name, r.default_branch, r.connection_id, r.sync_state, c.name AS connection_name, c.kind AS connection_kind
             FROM repositories r
             INNER JOIN connections c ON c.id = r.connection_id
             WHERE r.id = $1",
        )
        .bind(repo_id)
        .fetch_optional(&self.pool)
        .await? else {
            return Ok(None);
        };

        Ok(Some(RepositoryDetail {
            repository: row_to_repository(&row)?,
            connection: row_to_connection(&row)?,
        }))
    }

    async fn import_local_repository(
        &self,
        connection: Connection,
        repo_path: &str,
    ) -> Result<ImportRepositoryResult> {
        let canonical_path = canonical_local_git_repository_path(repo_path)?;
        let default_branch = git_output(&canonical_path, &["symbolic-ref", "--short", "HEAD"])?;
        let repository_name = repository_name_from_path(&canonical_path)?;
        let repo_id_base = format!(
            "repo_local_{}",
            sanitize_repo_id_component(&repository_name)
        );

        let mut transaction = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind("local-import")
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            "INSERT INTO connections (id, name, kind) VALUES ($1, $2, $3)
             ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, kind = EXCLUDED.kind",
        )
        .bind(&connection.id)
        .bind(&connection.name)
        .bind(connection_kind_to_str(&connection.kind))
        .execute(&mut *transaction)
        .await?;

        if let Some(row) = sqlx::query(
            "SELECT r.id AS repo_id, r.name AS repo_name, r.default_branch, r.connection_id, r.sync_state
             FROM repositories r
             INNER JOIN local_repository_paths p ON p.repository_id = r.id
             WHERE p.connection_id = $1 AND p.canonical_path = $2
             ORDER BY r.id
             LIMIT 1",
        )
        .bind(&connection.id)
        .bind(canonical_path.display().to_string())
        .fetch_optional(&mut *transaction)
        .await?
        {
            let repository = row_to_repository(&row)?;
            transaction.commit().await?;
            return Ok(ImportRepositoryResult {
                detail: RepositoryDetail {
                    repository,
                    connection,
                },
                created: false,
            });
        }

        if let Some(row) = sqlx::query(
            "SELECT r.id AS repo_id, r.name AS repo_name, r.default_branch, r.connection_id, r.sync_state
             FROM repositories r
             LEFT JOIN local_repository_paths p ON p.repository_id = r.id
             WHERE r.connection_id = $1 AND r.name = $2 AND p.repository_id IS NULL
             ORDER BY r.id
             LIMIT 1",
        )
        .bind(&connection.id)
        .bind(&repository_name)
        .fetch_optional(&mut *transaction)
        .await?
        {
            let repository = row_to_repository(&row)?;
            sqlx::query(
                "INSERT INTO local_repository_paths (repository_id, connection_id, canonical_path)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (connection_id, canonical_path) DO NOTHING",
            )
            .bind(&repository.id)
            .bind(&connection.id)
            .bind(canonical_path.display().to_string())
            .execute(&mut *transaction)
            .await?;
            transaction.commit().await?;
            return Ok(ImportRepositoryResult {
                detail: RepositoryDetail {
                    repository,
                    connection,
                },
                created: false,
            });
        }

        let existing_ids = sqlx::query_scalar::<_, String>("SELECT id FROM repositories")
            .fetch_all(&mut *transaction)
            .await?
            .into_iter()
            .collect::<std::collections::HashSet<_>>();
        let mut repo_id = repo_id_base.clone();
        let mut suffix = 2usize;
        while existing_ids.contains(&repo_id) {
            repo_id = format!("{repo_id_base}_{suffix}");
            suffix += 1;
        }

        sqlx::query(
            "INSERT INTO repositories (id, name, default_branch, connection_id, sync_state)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(&repo_id)
        .bind(&repository_name)
        .bind(&default_branch)
        .bind(&connection.id)
        .bind("ready")
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO local_repository_paths (repository_id, connection_id, canonical_path)
             VALUES ($1, $2, $3)",
        )
        .bind(&repo_id)
        .bind(&connection.id)
        .bind(canonical_path.display().to_string())
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;

        let repository = Repository {
            id: repo_id,
            name: repository_name,
            default_branch,
            connection_id: connection.id.clone(),
            sync_state: SyncState::Ready,
        };
        Ok(ImportRepositoryResult {
            detail: RepositoryDetail {
                repository,
                connection,
            },
            created: true,
        })
    }
}

pub async fn build_catalog_store(database_url: Option<&str>) -> Result<DynCatalogStore> {
    if let Some(database_url) = database_url {
        return Ok(Arc::new(PgCatalogStore::connect_lazy(database_url)?));
    }

    Ok(Arc::new(InMemoryCatalogStore::seeded()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sourcebot_models::{ConnectionConfig, ConnectionKind, SyncState};
    use sqlx::postgres::PgPoolOptions;
    use std::env;

    #[test]
    fn catalog_migration_inventory_bootstraps_catalog_org_repository_permissions_sessions_ask_threads_review_agent_runs_delivery_attempts_repository_sync_jobs_auth_audit_events_and_external_accounts(
    ) {
        let migrations = catalog_migrator().iter().collect::<Vec<_>>();
        let migration_versions = migrations
            .iter()
            .map(|migration| migration.version)
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(
            migration_versions,
            [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
                .into_iter()
                .collect(),
            "expected only the task05a + task05b1 + task05b2 + task05b3 + task05b4 + task05b5 + task05b6 + task87b1 + task87c + task87c4 + task88 ask-thread message + task94h auth audit-event + task95 external-account + task96 local-repository-path + task96 repository-sync terminal metadata migration versions"
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
                "0009_api_key_oauth_client_metadata.down.sql".to_string(),
                "0009_api_key_oauth_client_metadata.up.sql".to_string(),
                "0010_repository_sync_jobs.down.sql".to_string(),
                "0010_repository_sync_jobs.up.sql".to_string(),
                "0011_ask_thread_messages.down.sql".to_string(),
                "0011_ask_thread_messages.up.sql".to_string(),
                "0012_auth_audit_events.down.sql".to_string(),
                "0012_auth_audit_events.up.sql".to_string(),
                "0013_external_accounts.down.sql".to_string(),
                "0013_external_accounts.up.sql".to_string(),
                "0014_local_repository_paths.down.sql".to_string(),
                "0014_local_repository_paths.up.sql".to_string(),
                "0015_repository_sync_job_terminal_metadata.down.sql".to_string(),
                "0015_repository_sync_job_terminal_metadata.up.sql".to_string(),
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
            "CREATE TABLE local_repository_paths",
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

        let task87b2_up_migration =
            std::fs::read_to_string(migration_dir.join("0008_local_account_password_hash.up.sql"))
                .unwrap();

        for expected_snippet in [
            "ALTER TABLE local_accounts",
            "ADD COLUMN password_hash TEXT",
        ] {
            assert!(
                task87b2_up_migration.contains(expected_snippet),
                "missing task87b2 migration snippet: {expected_snippet}"
            );
        }

        for unexpected_snippet in [
            "CREATE TABLE api_keys",
            "CREATE TABLE oauth_clients",
            "CREATE TABLE analytics_events",
        ] {
            assert!(
                !task87b2_up_migration.contains(unexpected_snippet),
                "unexpected out-of-scope table present in 0008: {unexpected_snippet}"
            );
        }

        let task87c_up_migration = std::fs::read_to_string(
            migration_dir.join("0009_api_key_oauth_client_metadata.up.sql"),
        )
        .unwrap();

        for expected_snippet in [
            "CREATE TABLE api_keys",
            "id TEXT PRIMARY KEY",
            "user_id TEXT NOT NULL REFERENCES local_accounts(id)",
            "name TEXT NOT NULL",
            "secret_hash TEXT NOT NULL",
            "created_at TIMESTAMPTZ NOT NULL",
            "revoked_at TIMESTAMPTZ",
            "repo_scope TEXT[] NOT NULL",
            "CREATE TABLE oauth_clients",
            "organization_id TEXT NOT NULL REFERENCES organizations(id)",
            "client_id TEXT NOT NULL UNIQUE",
            "client_secret_hash TEXT NOT NULL",
            "redirect_uris TEXT[] NOT NULL",
            "created_by_user_id TEXT NOT NULL REFERENCES local_accounts(id)",
        ] {
            assert!(
                task87c_up_migration.contains(expected_snippet),
                "missing task87c migration snippet: {expected_snippet}"
            );
        }

        let task87c4_up_migration =
            std::fs::read_to_string(migration_dir.join("0010_repository_sync_jobs.up.sql"))
                .unwrap();

        for expected_snippet in [
            "CREATE TABLE repository_sync_jobs",
            "organization_id TEXT NOT NULL REFERENCES organizations(id)",
            "repository_id TEXT NOT NULL REFERENCES repositories(id)",
            "connection_id TEXT NOT NULL REFERENCES connections(id)",
            "status TEXT NOT NULL",
            "queued_at TIMESTAMPTZ NOT NULL",
            "started_at TIMESTAMPTZ",
            "finished_at TIMESTAMPTZ",
            "error TEXT",
            "CHECK (status IN ('queued', 'running', 'succeeded', 'failed'))",
            "CREATE INDEX repository_sync_jobs_claim_idx",
        ] {
            assert!(
                task87c4_up_migration.contains(expected_snippet),
                "missing task87c4 migration snippet: {expected_snippet}"
            );
        }

        for unexpected_snippet in ["CREATE TABLE organization_aggregates"] {
            assert!(
                !task87c4_up_migration.contains(unexpected_snippet),
                "unexpected out-of-scope table present in 0010: {unexpected_snippet}"
            );
        }

        let task88_up_migration =
            std::fs::read_to_string(migration_dir.join("0011_ask_thread_messages.up.sql")).unwrap();

        for expected_snippet in [
            "ALTER TABLE ask_threads",
            "ADD COLUMN messages JSONB NOT NULL DEFAULT '[]'::jsonb",
        ] {
            assert!(
                task88_up_migration.contains(expected_snippet),
                "missing task88 migration snippet: {expected_snippet}"
            );
        }

        for unexpected_snippet in [
            "CREATE TABLE review_agent_runs",
            "CREATE TABLE repository_sync_jobs",
            "CREATE TABLE organization_aggregates",
            "CREATE TABLE audit_events",
        ] {
            assert!(
                !task88_up_migration.contains(unexpected_snippet),
                "unexpected out-of-scope table present in 0011: {unexpected_snippet}"
            );
        }

        let task94h_up_migration =
            std::fs::read_to_string(migration_dir.join("0012_auth_audit_events.up.sql")).unwrap();

        for expected_snippet in [
            "CREATE TABLE audit_events",
            "id TEXT PRIMARY KEY",
            "organization_id TEXT NOT NULL REFERENCES organizations(id)",
            "actor_user_id TEXT REFERENCES local_accounts(id)",
            "actor_api_key_id TEXT",
            "action TEXT NOT NULL",
            "target_type TEXT NOT NULL",
            "target_id TEXT NOT NULL",
            "occurred_at TIMESTAMPTZ NOT NULL",
            "metadata JSONB NOT NULL DEFAULT '{}'::jsonb",
            "CREATE INDEX audit_events_organization_occurred_at_idx",
        ] {
            assert!(
                task94h_up_migration.contains(expected_snippet),
                "missing task94h migration snippet: {expected_snippet}"
            );
        }

        let task95_up_migration =
            std::fs::read_to_string(migration_dir.join("0013_external_accounts.up.sql")).unwrap();

        for expected_snippet in [
            "CREATE TABLE external_accounts",
            "id TEXT PRIMARY KEY",
            "user_id TEXT NOT NULL REFERENCES local_accounts(id) ON DELETE CASCADE",
            "provider TEXT NOT NULL CHECK (length(trim(provider)) > 0)",
            "provider_user_id TEXT NOT NULL CHECK (length(trim(provider_user_id)) > 0)",
            "email TEXT NOT NULL",
            "name TEXT NOT NULL",
            "linked_at TIMESTAMPTZ NOT NULL",
            "last_login_at TIMESTAMPTZ",
            "UNIQUE (provider, provider_user_id)",
            "CREATE INDEX external_accounts_user_id_idx ON external_accounts(user_id)",
        ] {
            assert!(
                task95_up_migration.contains(expected_snippet),
                "missing task95 migration snippet: {expected_snippet}"
            );
        }

        let task96k_up_migration = std::fs::read_to_string(
            migration_dir.join("0015_repository_sync_job_terminal_metadata.up.sql"),
        )
        .unwrap();

        for expected_snippet in [
            "ALTER TABLE repository_sync_jobs",
            "ADD COLUMN synced_revision TEXT",
            "ADD COLUMN synced_branch TEXT",
            "ADD COLUMN synced_content_file_count BIGINT",
        ] {
            assert!(
                task96k_up_migration.contains(expected_snippet),
                "missing task96k migration snippet: {expected_snippet}"
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

    async fn pg_catalog_test_pool() -> sqlx::PgPool {
        let database_url = env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL must be set for destructive Postgres catalog-store tests");
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .expect("connect to Postgres test database");
        catalog_migrator()
            .run(&pool)
            .await
            .expect("apply catalog migrations");
        sqlx::query(
            "TRUNCATE TABLE delivery_attempts, review_agent_runs, repository_permission_bindings, repositories, connections RESTART IDENTITY CASCADE",
        )
        .execute(&pool)
        .await
        .expect("reset catalog test tables");
        pool
    }

    #[tokio::test]
    async fn pg_catalog_store_lists_repository_summaries_from_postgres() {
        let pool = pg_catalog_test_pool().await;
        sqlx::query("INSERT INTO connections (id, name, kind) VALUES ($1, $2, $3)")
            .bind("conn_pg")
            .bind("Postgres GitHub")
            .bind("github")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO repositories (id, name, default_branch, connection_id, sync_state) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind("repo_pg")
        .bind("pg repo")
        .bind("main")
        .bind("conn_pg")
        .bind("ready")
        .execute(&pool)
        .await
        .unwrap();

        let store = PgCatalogStore { pool };

        assert_eq!(
            store.list_repositories().await.unwrap(),
            vec![RepositorySummary {
                id: "repo_pg".into(),
                name: "pg repo".into(),
                default_branch: "main".into(),
                sync_state: SyncState::Ready,
            }]
        );
    }

    #[tokio::test]
    async fn pg_catalog_store_returns_repository_detail_with_connection_from_postgres() {
        let pool = pg_catalog_test_pool().await;
        sqlx::query("INSERT INTO connections (id, name, kind) VALUES ($1, $2, $3)")
            .bind("conn_pg_detail")
            .bind("Postgres Local")
            .bind("local")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO repositories (id, name, default_branch, connection_id, sync_state) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind("repo_pg_detail")
        .bind("pg detail")
        .bind("trunk")
        .bind("conn_pg_detail")
        .bind("pending")
        .execute(&pool)
        .await
        .unwrap();

        let store = PgCatalogStore { pool };

        assert_eq!(
            store.get_repository_detail("repo_pg_detail").await.unwrap(),
            Some(RepositoryDetail {
                repository: Repository {
                    id: "repo_pg_detail".into(),
                    name: "pg detail".into(),
                    default_branch: "trunk".into(),
                    connection_id: "conn_pg_detail".into(),
                    sync_state: SyncState::Pending,
                },
                connection: Connection {
                    id: "conn_pg_detail".into(),
                    name: "Postgres Local".into(),
                    kind: ConnectionKind::Local,
                    config: None,
                },
            })
        );
        assert_eq!(store.get_repository_detail("missing").await.unwrap(), None);
    }

    #[tokio::test]
    async fn pg_catalog_store_imports_local_repository_into_postgres_catalog() {
        let pool = pg_catalog_test_pool().await;
        let repo_path =
            std::env::temp_dir().join(format!("sourcebot-pg-local-import-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&repo_path);
        std::fs::create_dir_all(&repo_path).unwrap();
        std::process::Command::new("git")
            .arg("init")
            .arg("-b")
            .arg("main")
            .arg(&repo_path)
            .output()
            .expect("initialize local git repository");
        std::fs::write(repo_path.join("README.md"), "# imported\n").unwrap();
        std::process::Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .args(["add", "README.md"])
            .output()
            .expect("stage local git repository content");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .args([
                "-c",
                "user.email=test@example.com",
                "-c",
                "user.name=Test User",
            ])
            .args(["commit", "-m", "initial"])
            .output()
            .expect("commit local git repository content");

        let store = PgCatalogStore { pool };
        let connection = Connection {
            id: "conn_pg_local_import".into(),
            name: "Postgres Local Import".into(),
            kind: ConnectionKind::Local,
            config: Some(ConnectionConfig::Local {
                repo_path: repo_path.display().to_string(),
            }),
        };

        let imported = store
            .import_local_repository(connection.clone(), &repo_path.display().to_string())
            .await
            .unwrap();

        assert!(imported.created);
        assert_eq!(
            imported.detail.repository.name,
            repo_path.file_name().unwrap().to_string_lossy()
        );
        assert_eq!(imported.detail.repository.default_branch, "main");
        assert_eq!(
            imported.detail.repository.connection_id,
            "conn_pg_local_import"
        );
        assert_eq!(imported.detail.connection, connection);
        assert!(store
            .list_repositories()
            .await
            .unwrap()
            .iter()
            .any(|repository| repository.id == imported.detail.repository.id
                && repository.name == imported.detail.repository.name
                && repository.sync_state == SyncState::Ready));
        assert_eq!(
            store
                .get_repository_detail(&imported.detail.repository.id)
                .await
                .unwrap()
                .unwrap()
                .repository,
            imported.detail.repository
        );

        std::fs::remove_dir_all(repo_path).unwrap();
    }

    #[tokio::test]
    async fn pg_catalog_store_imports_same_named_local_repositories_as_distinct_paths() {
        let pool = pg_catalog_test_pool().await;
        let root = std::env::temp_dir().join(format!(
            "sourcebot-pg-local-import-same-name-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let first_repo = root.join("first").join("duplicate-name");
        let second_repo = root.join("second").join("duplicate-name");
        for repo_path in [&first_repo, &second_repo] {
            std::fs::create_dir_all(repo_path).unwrap();
            std::process::Command::new("git")
                .arg("init")
                .arg("-b")
                .arg("main")
                .arg(repo_path)
                .output()
                .expect("initialize local git repository");
            std::fs::write(repo_path.join("README.md"), "# imported\n").unwrap();
            std::process::Command::new("git")
                .arg("-C")
                .arg(repo_path)
                .args(["add", "README.md"])
                .output()
                .expect("stage local git repository content");
            std::process::Command::new("git")
                .arg("-C")
                .arg(repo_path)
                .args([
                    "-c",
                    "user.email=test@example.com",
                    "-c",
                    "user.name=Test User",
                ])
                .args(["commit", "-m", "initial"])
                .output()
                .expect("commit local git repository content");
        }

        let store = PgCatalogStore { pool };
        let connection = Connection {
            id: "conn_pg_local_import_same_name".into(),
            name: "Postgres Local Import".into(),
            kind: ConnectionKind::Local,
            config: Some(ConnectionConfig::Local {
                repo_path: root.display().to_string(),
            }),
        };

        let first = store
            .import_local_repository(connection.clone(), &first_repo.display().to_string())
            .await
            .unwrap();
        let second = store
            .import_local_repository(connection.clone(), &second_repo.display().to_string())
            .await
            .unwrap();
        let first_again = store
            .import_local_repository(connection, &first_repo.display().to_string())
            .await
            .unwrap();

        assert!(first.created);
        assert!(second.created);
        assert!(!first_again.created);
        assert_ne!(first.detail.repository.id, second.detail.repository.id);
        assert_eq!(first.detail.repository.name, "duplicate-name");
        assert_eq!(second.detail.repository.name, "duplicate-name");
        assert_eq!(first_again.detail.repository.id, first.detail.repository.id);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn pg_catalog_store_build_catalog_store_with_database_uses_postgres_catalog_store_path() {
        let pool = pg_catalog_test_pool().await;
        sqlx::query("INSERT INTO connections (id, name, kind) VALUES ($1, $2, $3)")
            .bind("conn_build")
            .bind("Build GitLab")
            .bind("gitlab")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO repositories (id, name, default_branch, connection_id, sync_state) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind("repo_build")
        .bind("build repo")
        .bind("main")
        .bind("conn_build")
        .bind("error")
        .execute(&pool)
        .await
        .unwrap();
        drop(pool);

        let database_url = env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL must be set for destructive Postgres catalog-store tests");
        let store = build_catalog_store(Some(database_url.as_str()))
            .await
            .unwrap();

        assert_eq!(
            store.list_repositories().await.unwrap(),
            vec![RepositorySummary {
                id: "repo_build".into(),
                name: "build repo".into(),
                default_branch: "main".into(),
                sync_state: SyncState::Error,
            }]
        );
    }
}
