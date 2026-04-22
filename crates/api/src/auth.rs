use anyhow::{anyhow, Result};
use async_trait::async_trait;
use sourcebot_core::{
    claim_next_repository_sync_job, claim_next_review_agent_run, complete_review_agent_run,
    fail_review_agent_run, store_repository_sync_job as upsert_repository_sync_job, BootstrapStore,
    LocalSessionStore, OrganizationStore,
};
use sourcebot_models::RepositorySyncJobStatus;
use sourcebot_models::{
    BootstrapState, BootstrapStatus, LocalAccount, LocalSession, LocalSessionState,
    OrganizationState, RepositorySyncJob, ReviewAgentRunStatus,
};
use sqlx::{postgres::PgPoolOptions, Row};
use std::{
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    sync::Arc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

pub type DynBootstrapStore = Arc<dyn BootstrapStore>;
pub type DynLocalSessionStore = Arc<dyn LocalSessionStore>;
pub type DynOrganizationStore = Arc<dyn OrganizationStore>;

const LOCAL_BOOTSTRAP_ADMIN_USER_ID: &str = "local_user_bootstrap_admin";

#[derive(Clone, Debug)]
pub struct FileBootstrapStore {
    state_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct FileLocalSessionStore {
    state_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct FileOrganizationStore {
    state_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct PgLocalSessionStore {
    pool: sqlx::PgPool,
}

#[derive(Clone, Debug)]
pub struct PgBootstrapStore {
    pool: sqlx::PgPool,
}

#[derive(Debug)]
struct StateFileWriteLock {
    file: File,
    lock_path: PathBuf,
}

impl Drop for StateFileWriteLock {
    fn drop(&mut self) {
        let _ = self.file.sync_all();
        let _ = fs::remove_file(&self.lock_path);
    }
}

impl FileBootstrapStore {
    pub fn new(state_path: impl Into<PathBuf>) -> Self {
        Self {
            state_path: state_path.into(),
        }
    }

    fn read_persisted_state(&self) -> Result<Option<BootstrapState>> {
        if !self.state_path.is_file() {
            return Ok(None);
        }

        match fs::read(&self.state_path) {
            Ok(bytes) => Ok(serde_json::from_slice::<BootstrapState>(&bytes).ok()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

impl FileLocalSessionStore {
    pub fn new(state_path: impl Into<PathBuf>) -> Self {
        Self {
            state_path: state_path.into(),
        }
    }

    fn lock_path(&self) -> PathBuf {
        let file_name = self
            .state_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("local-sessions.json");
        self.state_path.with_file_name(format!(".{file_name}.lock"))
    }

    fn acquire_write_lock(&self) -> Result<StateFileWriteLock> {
        const MAX_LOCK_WAIT: Duration = Duration::from_millis(100);
        const LOCK_RETRY_DELAY: Duration = Duration::from_millis(10);

        ensure_parent_directory(&self.state_path)?;
        let lock_path = self.lock_path();
        let start = SystemTime::now();

        loop {
            match open_new_private_file(&lock_path) {
                Ok(file) => return Ok(StateFileWriteLock { file, lock_path }),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    if start.elapsed().unwrap_or_default() >= MAX_LOCK_WAIT {
                        return Err(anyhow!(
                            "timed out waiting for local session lock at {}",
                            lock_path.display()
                        ));
                    }
                    thread::sleep(LOCK_RETRY_DELAY);
                }
                Err(error) => return Err(error.into()),
            }
        }
    }

    fn read_persisted_state(&self) -> Result<LocalSessionState> {
        if !self.state_path.is_file() {
            return Ok(LocalSessionState::default());
        }

        match fs::read(&self.state_path) {
            Ok(bytes) => Ok(serde_json::from_slice::<LocalSessionState>(&bytes)?),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(LocalSessionState::default()),
            Err(error) => Err(error.into()),
        }
    }
}

impl PgLocalSessionStore {
    pub fn connect_lazy(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_lazy(database_url)?;
        Ok(Self { pool })
    }
}

impl PgBootstrapStore {
    pub fn connect_lazy(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_lazy(database_url)?;
        Ok(Self { pool })
    }

    async fn bootstrap_row(&self) -> Result<Option<BootstrapState>> {
        let row = sqlx::query(
            "SELECT email, name, password_hash, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at FROM local_accounts WHERE id = $1 AND password_hash IS NOT NULL",
        )
        .bind(LOCAL_BOOTSTRAP_ADMIN_USER_ID)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| BootstrapState {
            initialized_at: row.get("created_at"),
            admin_email: row.get("email"),
            admin_name: row.get("name"),
            password_hash: row.get("password_hash"),
        }))
    }
}

impl FileOrganizationStore {
    pub fn new(state_path: impl Into<PathBuf>) -> Self {
        Self {
            state_path: state_path.into(),
        }
    }

    fn lock_path(&self) -> PathBuf {
        let file_name = self
            .state_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("organization-state.json");
        self.state_path.with_file_name(format!(".{file_name}.lock"))
    }

    fn acquire_write_lock(&self) -> Result<StateFileWriteLock> {
        const MAX_LOCK_WAIT: Duration = Duration::from_millis(100);
        const LOCK_RETRY_DELAY: Duration = Duration::from_millis(10);

        ensure_parent_directory(&self.state_path)?;
        let lock_path = self.lock_path();
        let start = SystemTime::now();

        loop {
            match open_new_private_file(&lock_path) {
                Ok(file) => return Ok(StateFileWriteLock { file, lock_path }),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    if start.elapsed().unwrap_or_default() >= MAX_LOCK_WAIT {
                        return Err(anyhow!(
                            "timed out waiting for organization-state lock at {}",
                            lock_path.display()
                        ));
                    }
                    thread::sleep(LOCK_RETRY_DELAY);
                }
                Err(error) => return Err(error.into()),
            }
        }
    }

    fn read_persisted_state(&self) -> Result<OrganizationState> {
        if !self.state_path.is_file() {
            return Ok(OrganizationState::default());
        }

        match fs::read(&self.state_path) {
            Ok(bytes) => Ok(serde_json::from_slice::<OrganizationState>(&bytes)?),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(OrganizationState::default()),
            Err(error) => Err(error.into()),
        }
    }
}

fn review_agent_run_status_rank(status: &ReviewAgentRunStatus) -> u8 {
    match status {
        ReviewAgentRunStatus::Queued => 0,
        ReviewAgentRunStatus::Claimed => 1,
        ReviewAgentRunStatus::Completed | ReviewAgentRunStatus::Failed => 2,
    }
}

fn preserve_terminal_review_agent_runs(
    persisted_state: &OrganizationState,
    next_state: &mut OrganizationState,
) {
    for persisted_run in &persisted_state.review_agent_runs {
        if !matches!(
            persisted_run.status,
            ReviewAgentRunStatus::Claimed
                | ReviewAgentRunStatus::Completed
                | ReviewAgentRunStatus::Failed
        ) {
            continue;
        }

        if let Some(next_run) = next_state
            .review_agent_runs
            .iter_mut()
            .find(|run| run.id == persisted_run.id)
        {
            let persisted_rank = review_agent_run_status_rank(&persisted_run.status);
            let next_rank = review_agent_run_status_rank(&next_run.status);
            let persisted_terminal_mismatch =
                persisted_rank == 2 && next_rank == 2 && next_run.status != persisted_run.status;

            if persisted_rank > next_rank || persisted_terminal_mismatch {
                next_run.status = persisted_run.status.clone();
            }
        }
    }
}

fn repository_sync_job_status_rank(status: &RepositorySyncJobStatus) -> u8 {
    match status {
        RepositorySyncJobStatus::Queued => 0,
        RepositorySyncJobStatus::Running => 1,
        RepositorySyncJobStatus::Succeeded | RepositorySyncJobStatus::Failed => 2,
    }
}

fn preserve_repository_sync_job_progress(
    persisted_state: &OrganizationState,
    next_state: &mut OrganizationState,
) {
    for persisted_job in &persisted_state.repository_sync_jobs {
        if let Some(next_job) = next_state
            .repository_sync_jobs
            .iter_mut()
            .find(|job| job.id == persisted_job.id)
        {
            let persisted_rank = repository_sync_job_status_rank(&persisted_job.status);
            let next_rank = repository_sync_job_status_rank(&next_job.status);
            let persisted_terminal_mismatch =
                persisted_rank == 2 && next_rank == 2 && next_job.status != persisted_job.status;

            if persisted_rank > next_rank || persisted_terminal_mismatch {
                *next_job = persisted_job.clone();
            }
        } else {
            next_state.repository_sync_jobs.push(persisted_job.clone());
        }
    }
}

fn temporary_state_path(state_path: &Path, fallback_name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let file_name = state_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(fallback_name);
    state_path.with_file_name(format!(".{file_name}.{nanos}.tmp"))
}

fn open_new_private_file(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        options.mode(0o600);
    }
    options.open(path)
}

fn sync_parent_directory(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            File::open(parent)?.sync_all()?;
        }
    }

    Ok(())
}

fn ensure_parent_directory(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    Ok(())
}

fn write_json_file(path: &Path, payload: &[u8], replace_existing: bool) -> std::io::Result<()> {
    ensure_parent_directory(path)?;

    let temp_path = temporary_state_path(path, "state.json");
    let write_result = (|| -> std::io::Result<()> {
        let mut file = open_new_private_file(&temp_path)?;
        file.write_all(payload)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);

        if replace_existing {
            fs::rename(&temp_path, path)?;
        } else {
            fs::hard_link(&temp_path, path)?;
            fs::remove_file(&temp_path)?;
        }

        sync_parent_directory(path)?;
        Ok(())
    })();

    if let Err(error) = write_result {
        match fs::remove_file(&temp_path) {
            Ok(()) => {}
            Err(remove_error) if remove_error.kind() == ErrorKind::NotFound => {}
            Err(remove_error) => return Err(remove_error),
        }
        return Err(error);
    }

    Ok(())
}

#[async_trait]
impl BootstrapStore for FileBootstrapStore {
    async fn bootstrap_status(&self) -> Result<BootstrapStatus> {
        let bootstrap_required = self.read_persisted_state()?.is_none();

        Ok(BootstrapStatus { bootstrap_required })
    }

    async fn bootstrap_state(&self) -> Result<Option<BootstrapState>> {
        self.read_persisted_state()
    }

    async fn initialize_bootstrap(&self, state: BootstrapState) -> Result<()> {
        let payload = serde_json::to_vec_pretty(&state)?;
        write_json_file(&self.state_path, &payload, false)?;
        Ok(())
    }
}

#[async_trait]
impl BootstrapStore for PgBootstrapStore {
    async fn bootstrap_status(&self) -> Result<BootstrapStatus> {
        let bootstrap_required = sqlx::query(
            "SELECT 1 FROM local_accounts WHERE id = $1 AND password_hash IS NOT NULL",
        )
        .bind(LOCAL_BOOTSTRAP_ADMIN_USER_ID)
        .fetch_optional(&self.pool)
        .await?
        .is_none();

        Ok(BootstrapStatus { bootstrap_required })
    }

    async fn bootstrap_state(&self) -> Result<Option<BootstrapState>> {
        self.bootstrap_row().await
    }

    async fn initialize_bootstrap(&self, state: BootstrapState) -> Result<()> {
        let result = sqlx::query(
            "INSERT INTO local_accounts (id, email, name, password_hash, created_at) VALUES ($1, $2, $3, $4, $5::timestamptz) ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.email, name = EXCLUDED.name, password_hash = EXCLUDED.password_hash, created_at = EXCLUDED.created_at WHERE local_accounts.password_hash IS NULL",
        )
        .bind(LOCAL_BOOTSTRAP_ADMIN_USER_ID)
        .bind(state.admin_email)
        .bind(state.admin_name)
        .bind(state.password_hash)
        .bind(state.initialized_at)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(std::io::Error::new(
                ErrorKind::AlreadyExists,
                "bootstrap already initialized",
            )
            .into());
        }

        Ok(())
    }
}

#[async_trait]
impl LocalSessionStore for FileLocalSessionStore {
    async fn local_session(&self, session_id: &str) -> Result<Option<LocalSession>> {
        let state = self.read_persisted_state()?;
        Ok(state
            .sessions
            .into_iter()
            .find(|session| session.id == session_id))
    }

    async fn store_local_session(&self, session: LocalSession) -> Result<()> {
        let _lock = self.acquire_write_lock()?;
        let mut state = self.read_persisted_state()?;
        state
            .sessions
            .retain(|persisted| persisted.id != session.id);
        state.sessions.push(session);

        let payload = serde_json::to_vec_pretty(&state)?;
        write_json_file(&self.state_path, &payload, true)?;
        Ok(())
    }

    async fn delete_local_session(&self, session_id: &str) -> Result<bool> {
        let _lock = self.acquire_write_lock()?;
        let mut state = self.read_persisted_state()?;
        let original_len = state.sessions.len();
        state
            .sessions
            .retain(|persisted| persisted.id != session_id);

        if state.sessions.len() == original_len {
            return Ok(false);
        }

        let payload = serde_json::to_vec_pretty(&state)?;
        write_json_file(&self.state_path, &payload, true)?;
        Ok(true)
    }
}

#[async_trait]
impl LocalSessionStore for PgLocalSessionStore {
    async fn local_session(&self, session_id: &str) -> Result<Option<LocalSession>> {
        let row = sqlx::query(
            "SELECT id, user_id, secret_hash, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at FROM sessions WHERE id = $1",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| LocalSession {
            id: row.get("id"),
            user_id: row.get("user_id"),
            secret_hash: row.get("secret_hash"),
            created_at: row.get("created_at"),
        }))
    }

    async fn persist_local_session_account(&self, account: LocalAccount) -> Result<()> {
        sqlx::query(
            "INSERT INTO local_accounts (id, email, name, created_at) VALUES ($1, $2, $3, $4::timestamptz) ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.email, name = EXCLUDED.name, created_at = EXCLUDED.created_at",
        )
        .bind(account.id)
        .bind(account.email)
        .bind(account.name)
        .bind(account.created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn store_local_session(&self, session: LocalSession) -> Result<()> {
        sqlx::query(
            "INSERT INTO sessions (id, user_id, secret_hash, created_at) VALUES ($1, $2, $3, $4::timestamptz) ON CONFLICT (id) DO UPDATE SET user_id = EXCLUDED.user_id, secret_hash = EXCLUDED.secret_hash, created_at = EXCLUDED.created_at",
        )
        .bind(session.id)
        .bind(session.user_id)
        .bind(session.secret_hash)
        .bind(session.created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn delete_local_session(&self, session_id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM sessions WHERE id = $1")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

#[async_trait]
impl OrganizationStore for FileOrganizationStore {
    async fn organization_state(&self) -> Result<OrganizationState> {
        self.read_persisted_state()
    }

    async fn store_organization_state(&self, state: OrganizationState) -> Result<()> {
        let _lock = self.acquire_write_lock()?;
        let persisted_state = self.read_persisted_state()?;
        let mut state = state;
        preserve_terminal_review_agent_runs(&persisted_state, &mut state);
        preserve_repository_sync_job_progress(&persisted_state, &mut state);
        let payload = serde_json::to_vec_pretty(&state)?;
        write_json_file(&self.state_path, &payload, true)?;
        Ok(())
    }

    async fn store_repository_sync_job(&self, job: RepositorySyncJob) -> Result<()> {
        let _lock = self.acquire_write_lock()?;
        let mut state = self.read_persisted_state()?;
        upsert_repository_sync_job(&mut state, job);
        let payload = serde_json::to_vec_pretty(&state)?;
        write_json_file(&self.state_path, &payload, true)?;
        Ok(())
    }

    async fn claim_next_repository_sync_job(
        &self,
        started_at: &str,
    ) -> Result<Option<RepositorySyncJob>> {
        let _lock = self.acquire_write_lock()?;
        let mut state = self.read_persisted_state()?;
        let claimed_job = claim_next_repository_sync_job(&mut state, started_at);

        if claimed_job.is_some() {
            let payload = serde_json::to_vec_pretty(&state)?;
            write_json_file(&self.state_path, &payload, true)?;
        }

        Ok(claimed_job)
    }

    async fn claim_and_complete_next_repository_sync_job(
        &self,
        started_at: &str,
        execute: fn(RepositorySyncJob) -> RepositorySyncJob,
    ) -> Result<Option<RepositorySyncJob>> {
        let _lock = self.acquire_write_lock()?;
        let mut state = self.read_persisted_state()?;
        let Some(claimed_job) = claim_next_repository_sync_job(&mut state, started_at) else {
            return Ok(None);
        };

        let completed_job = execute(claimed_job);
        upsert_repository_sync_job(&mut state, completed_job.clone());
        let payload = serde_json::to_vec_pretty(&state)?;
        write_json_file(&self.state_path, &payload, true)?;
        Ok(Some(completed_job))
    }

    async fn claim_next_review_agent_run(
        &self,
    ) -> Result<Option<sourcebot_models::ReviewAgentRun>> {
        let _lock = self.acquire_write_lock()?;
        let mut state = self.read_persisted_state()?;
        let claimed_run = claim_next_review_agent_run(&mut state);

        if claimed_run.is_some() {
            let payload = serde_json::to_vec_pretty(&state)?;
            write_json_file(&self.state_path, &payload, true)?;
        }

        Ok(claimed_run)
    }

    async fn complete_review_agent_run(
        &self,
        run_id: &str,
    ) -> Result<Option<sourcebot_models::ReviewAgentRun>> {
        let _lock = self.acquire_write_lock()?;
        let mut state = self.read_persisted_state()?;
        let completed_run = complete_review_agent_run(&mut state, run_id);

        if completed_run.is_some() {
            let payload = serde_json::to_vec_pretty(&state)?;
            write_json_file(&self.state_path, &payload, true)?;
        }

        Ok(completed_run)
    }

    async fn fail_review_agent_run(
        &self,
        run_id: &str,
    ) -> Result<Option<sourcebot_models::ReviewAgentRun>> {
        let _lock = self.acquire_write_lock()?;
        let mut state = self.read_persisted_state()?;
        let failed_run = fail_review_agent_run(&mut state, run_id);

        if failed_run.is_some() {
            let payload = serde_json::to_vec_pretty(&state)?;
            write_json_file(&self.state_path, &payload, true)?;
        }

        Ok(failed_run)
    }
}

pub fn try_build_bootstrap_store(
    state_path: impl Into<PathBuf>,
    database_url: Option<&str>,
) -> Result<DynBootstrapStore> {
    if let Some(database_url) = database_url {
        return Ok(Arc::new(PgBootstrapStore::connect_lazy(database_url)?));
    }

    Ok(Arc::new(FileBootstrapStore::new(state_path)))
}

pub fn build_bootstrap_store(
    state_path: impl Into<PathBuf>,
    database_url: Option<&str>,
) -> DynBootstrapStore {
    try_build_bootstrap_store(state_path, database_url)
        .expect("bootstrap store DATABASE_URL must be valid when configured")
}

pub fn try_build_local_session_store(
    state_path: impl Into<PathBuf>,
    database_url: Option<&str>,
) -> Result<DynLocalSessionStore> {
    if let Some(database_url) = database_url {
        return Ok(Arc::new(PgLocalSessionStore::connect_lazy(database_url)?));
    }

    Ok(Arc::new(FileLocalSessionStore::new(state_path)))
}

pub fn build_local_session_store(
    state_path: impl Into<PathBuf>,
    database_url: Option<&str>,
) -> DynLocalSessionStore {
    try_build_local_session_store(state_path, database_url)
        .expect("local session store DATABASE_URL must be valid")
}

pub fn build_organization_store(state_path: impl Into<PathBuf>) -> DynOrganizationStore {
    Arc::new(FileOrganizationStore::new(state_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::catalog_migrator;
    use sourcebot_models::{
        AnalyticsRecord, ApiKey, AuditActor, AuditEvent, Connection, ConnectionConfig,
        ConnectionKind, LocalAccount, OAuthClient, Organization, OrganizationInvite,
        OrganizationMembership, OrganizationRole, RepositoryPermissionBinding, RepositorySyncJob,
        RepositorySyncJobStatus, ReviewAgentRun, ReviewAgentRunStatus, ReviewWebhook,
        ReviewWebhookDeliveryAttempt, SearchContext,
    };
    use sqlx::{postgres::PgPoolOptions, Row};
    use std::{
        env, fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn unique_test_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("sourcebot-bootstrap-{name}-{nanos}.json"))
    }

    #[tokio::test]
    async fn file_bootstrap_store_requires_bootstrap_when_state_file_is_missing() {
        let path = unique_test_path("missing");
        let store = FileBootstrapStore::new(&path);

        assert!(store.bootstrap_status().await.unwrap().bootstrap_required);
    }

    #[tokio::test]
    async fn file_bootstrap_store_requires_bootstrap_when_state_file_is_invalid_json() {
        let path = unique_test_path("invalid-json");
        fs::write(&path, b"{\"initialized_at\":").unwrap();
        let store = FileBootstrapStore::new(&path);

        assert!(store.bootstrap_status().await.unwrap().bootstrap_required);

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_bootstrap_store_disables_bootstrap_when_state_file_exists() {
        let path = unique_test_path("present");
        fs::write(
            &path,
            serde_json::to_vec(&BootstrapState {
                initialized_at: "2026-04-16T17:00:00Z".into(),
                admin_email: "admin@example.com".into(),
                admin_name: "Admin User".into(),
                password_hash: "$argon2id$example".into(),
            })
            .unwrap(),
        )
        .unwrap();

        let store = FileBootstrapStore::new(&path);

        assert!(!store.bootstrap_status().await.unwrap().bootstrap_required);

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_bootstrap_store_requires_bootstrap_when_path_is_a_directory() {
        let path = unique_test_path("directory");
        fs::create_dir(&path).unwrap();

        let store = FileBootstrapStore::new(&path);

        assert!(store.bootstrap_status().await.unwrap().bootstrap_required);

        fs::remove_dir(path).unwrap();
    }

    #[tokio::test]
    async fn file_bootstrap_store_persists_initial_admin_state() {
        let path = unique_test_path("persisted-state");
        let store = FileBootstrapStore::new(&path);
        let state = BootstrapState {
            initialized_at: "2026-04-16T17:00:00Z".into(),
            admin_email: "admin@example.com".into(),
            admin_name: "Admin User".into(),
            password_hash: "$argon2id$example".into(),
        };

        store.initialize_bootstrap(state.clone()).await.unwrap();

        let persisted: BootstrapState = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(persisted, state);
        assert!(!store.bootstrap_status().await.unwrap().bootstrap_required);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_bootstrap_store_reads_bootstrap_state_only_when_persisted_state_is_valid() {
        let missing_path = unique_test_path("read-missing");
        let missing_store = FileBootstrapStore::new(&missing_path);
        assert_eq!(missing_store.bootstrap_state().await.unwrap(), None);

        let invalid_path = unique_test_path("read-invalid");
        fs::write(&invalid_path, b"{\"initialized_at\":").unwrap();
        let invalid_store = FileBootstrapStore::new(&invalid_path);
        assert_eq!(invalid_store.bootstrap_state().await.unwrap(), None);
        fs::remove_file(&invalid_path).unwrap();

        let valid_path = unique_test_path("read-valid");
        let valid_store = FileBootstrapStore::new(&valid_path);
        let expected_state = BootstrapState {
            initialized_at: "2026-04-16T17:00:00Z".into(),
            admin_email: "admin@example.com".into(),
            admin_name: "Admin User".into(),
            password_hash: "$argon2id$example".into(),
        };
        valid_store
            .initialize_bootstrap(expected_state.clone())
            .await
            .unwrap();

        assert_eq!(
            valid_store.bootstrap_state().await.unwrap(),
            Some(expected_state)
        );

        fs::remove_file(valid_path).unwrap();
    }

    #[tokio::test]
    async fn pg_bootstrap_store_requires_bootstrap_when_admin_row_is_missing() {
        let pool = bootstrap_test_pool().await;
        let store = PgBootstrapStore { pool: pool.clone() };

        assert!(store.bootstrap_status().await.unwrap().bootstrap_required);
        assert_eq!(store.bootstrap_state().await.unwrap(), None);
        assert_eq!(persisted_bootstrap_row(&pool).await, None);
    }

    #[tokio::test]
    async fn pg_bootstrap_store_treats_legacy_bootstrap_row_without_password_hash_as_still_requiring_bootstrap(
    ) {
        let pool = bootstrap_test_pool().await;
        sqlx::query(
            "INSERT INTO local_accounts (id, email, name, created_at) VALUES ($1, $2, $3, $4::timestamptz)",
        )
        .bind(LOCAL_BOOTSTRAP_ADMIN_USER_ID)
        .bind("legacy@example.com")
        .bind("Legacy Admin")
        .bind("2026-04-22T15:00:00Z")
        .execute(&pool)
        .await
        .unwrap();
        let store = PgBootstrapStore { pool: pool.clone() };

        assert!(store.bootstrap_status().await.unwrap().bootstrap_required);
        assert_eq!(store.bootstrap_state().await.unwrap(), None);

        let upgraded_state = BootstrapState {
            initialized_at: "2026-04-22T16:00:00Z".into(),
            admin_email: "bootstrap@example.com".into(),
            admin_name: "Bootstrap Admin".into(),
            password_hash: "$argon2id$bootstrap-hash".into(),
        };
        store.initialize_bootstrap(upgraded_state.clone()).await.unwrap();

        assert!(!store.bootstrap_status().await.unwrap().bootstrap_required);
        assert_eq!(store.bootstrap_state().await.unwrap(), Some(upgraded_state.clone()));
        assert_eq!(
            persisted_bootstrap_row(&pool).await,
            Some((
                "bootstrap@example.com".into(),
                "Bootstrap Admin".into(),
                "$argon2id$bootstrap-hash".into(),
                "2026-04-22T16:00:00Z".into(),
            ))
        );
    }

    #[tokio::test]
    async fn pg_bootstrap_store_initializes_bootstrap_admin_once() {
        let pool = bootstrap_test_pool().await;
        let store = PgBootstrapStore { pool: pool.clone() };
        let state = BootstrapState {
            initialized_at: "2026-04-22T16:00:00Z".into(),
            admin_email: "bootstrap@example.com".into(),
            admin_name: "Bootstrap Admin".into(),
            password_hash: "$argon2id$bootstrap-hash".into(),
        };

        store.initialize_bootstrap(state.clone()).await.unwrap();

        let error = store
            .initialize_bootstrap(BootstrapState {
                initialized_at: "2026-04-22T16:01:00Z".into(),
                admin_email: "other@example.com".into(),
                admin_name: "Other Admin".into(),
                password_hash: "$argon2id$other-hash".into(),
            })
            .await
            .unwrap_err();

        assert!(
            error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|io_error| io_error.kind() == ErrorKind::AlreadyExists)
        );
        assert!(!store.bootstrap_status().await.unwrap().bootstrap_required);
        assert_eq!(store.bootstrap_state().await.unwrap(), Some(state.clone()));
        assert_eq!(
            persisted_bootstrap_row(&pool).await,
            Some((
                "bootstrap@example.com".into(),
                "Bootstrap Admin".into(),
                "$argon2id$bootstrap-hash".into(),
                "2026-04-22T16:00:00Z".into(),
            ))
        );
    }

    #[tokio::test]
    async fn pg_bootstrap_store_reads_persisted_bootstrap_state() {
        let pool = bootstrap_test_pool().await;
        sqlx::query(
            "INSERT INTO local_accounts (id, email, name, password_hash, created_at) VALUES ($1, $2, $3, $4, $5::timestamptz)",
        )
        .bind(LOCAL_BOOTSTRAP_ADMIN_USER_ID)
        .bind("persisted@example.com")
        .bind("Persisted Admin")
        .bind("$argon2id$persisted-hash")
        .bind("2026-04-22T15:30:00Z")
        .execute(&pool)
        .await
        .unwrap();

        let store = PgBootstrapStore { pool: pool.clone() };

        assert!(!store.bootstrap_status().await.unwrap().bootstrap_required);
        assert_eq!(
            store.bootstrap_state().await.unwrap(),
            Some(BootstrapState {
                initialized_at: "2026-04-22T15:30:00Z".into(),
                admin_email: "persisted@example.com".into(),
                admin_name: "Persisted Admin".into(),
                password_hash: "$argon2id$persisted-hash".into(),
            })
        );
        assert_eq!(
            persisted_bootstrap_row(&pool).await,
            Some((
                "persisted@example.com".into(),
                "Persisted Admin".into(),
                "$argon2id$persisted-hash".into(),
                "2026-04-22T15:30:00Z".into(),
            ))
        );
    }

    #[tokio::test]
    async fn file_local_session_store_returns_none_when_session_file_is_missing() {
        let path = unique_test_path("session-missing");
        let store = FileLocalSessionStore::new(&path);

        assert_eq!(store.local_session("session_1").await.unwrap(), None);
    }

    #[tokio::test]
    async fn file_local_session_store_persists_and_reads_local_sessions() {
        let path = unique_test_path("session-persist");
        let store = FileLocalSessionStore::new(&path);
        let session = LocalSession {
            id: "session_1".into(),
            user_id: "local_user_bootstrap_admin".into(),
            secret_hash: "$argon2id$session-secret".into(),
            created_at: "2026-04-16T18:00:00Z".into(),
        };

        store.store_local_session(session.clone()).await.unwrap();

        let persisted: LocalSessionState =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(persisted.sessions, vec![session.clone()]);
        assert_eq!(
            store.local_session(&session.id).await.unwrap(),
            Some(session)
        );

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_local_session_store_rewrites_existing_session_with_same_id() {
        let path = unique_test_path("session-update");
        let store = FileLocalSessionStore::new(&path);

        store
            .store_local_session(LocalSession {
                id: "session_1".into(),
                user_id: "local_user_bootstrap_admin".into(),
                secret_hash: "$argon2id$original".into(),
                created_at: "2026-04-16T18:00:00Z".into(),
            })
            .await
            .unwrap();

        let updated = LocalSession {
            id: "session_1".into(),
            user_id: "local_user_bootstrap_admin".into(),
            secret_hash: "$argon2id$rotated".into(),
            created_at: "2026-04-16T19:00:00Z".into(),
        };

        store.store_local_session(updated.clone()).await.unwrap();

        let persisted: LocalSessionState =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(persisted.sessions, vec![updated.clone()]);
        assert_eq!(
            store.local_session("session_1").await.unwrap(),
            Some(updated)
        );

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_local_session_store_rejects_invalid_json_without_overwriting_existing_file() {
        let path = unique_test_path("session-invalid-json");
        fs::write(&path, b"{\"sessions\":").unwrap();
        let store = FileLocalSessionStore::new(&path);

        let error = store
            .store_local_session(LocalSession {
                id: "session_1".into(),
                user_id: "local_user_bootstrap_admin".into(),
                secret_hash: "$argon2id$session-secret".into(),
                created_at: "2026-04-16T18:00:00Z".into(),
            })
            .await
            .unwrap_err();

        assert!(error.to_string().contains("EOF while parsing"));
        assert_eq!(fs::read(&path).unwrap(), b"{\"sessions\":".to_vec());

        fs::remove_file(path).unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn file_local_session_store_preserves_distinct_sessions_across_concurrent_writes() {
        let path = unique_test_path("session-concurrent");
        let store_a = FileLocalSessionStore::new(&path);
        let store_b = FileLocalSessionStore::new(&path);
        let session_a = LocalSession {
            id: "session_1".into(),
            user_id: "local_user_bootstrap_admin".into(),
            secret_hash: "$argon2id$session-a".into(),
            created_at: "2026-04-16T18:00:00Z".into(),
        };
        let session_b = LocalSession {
            id: "session_2".into(),
            user_id: "local_user_bootstrap_admin".into(),
            secret_hash: "$argon2id$session-b".into(),
            created_at: "2026-04-16T18:01:00Z".into(),
        };

        let writer_a = {
            let session = session_a.clone();
            tokio::spawn(async move { store_a.store_local_session(session).await.unwrap() })
        };
        let writer_b = {
            let session = session_b.clone();
            tokio::spawn(async move { store_b.store_local_session(session).await.unwrap() })
        };

        writer_a.await.unwrap();
        writer_b.await.unwrap();

        let mut persisted = serde_json::from_slice::<LocalSessionState>(&fs::read(&path).unwrap())
            .unwrap()
            .sessions;
        persisted.sort_by(|left, right| left.id.cmp(&right.id));

        assert_eq!(persisted, vec![session_a, session_b]);
        assert!(!path
            .with_file_name(format!(
                ".{}.lock",
                path.file_name().unwrap().to_string_lossy()
            ))
            .exists());

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_local_session_store_times_out_when_lock_file_is_stale() {
        let path = unique_test_path("session-stale-lock");
        let lock_path = path.with_file_name(format!(
            ".{}.lock",
            path.file_name().unwrap().to_string_lossy()
        ));
        open_new_private_file(&lock_path).unwrap();
        let store = FileLocalSessionStore::new(&path);

        let result = tokio::time::timeout(
            Duration::from_millis(150),
            store.store_local_session(LocalSession {
                id: "session_1".into(),
                user_id: "local_user_bootstrap_admin".into(),
                secret_hash: "$argon2id$session-secret".into(),
                created_at: "2026-04-16T18:00:00Z".into(),
            }),
        )
        .await
        .expect("stale lock should fail instead of hanging forever")
        .expect_err("stale lock should return an error");

        assert!(result.to_string().contains("timed out"));
        assert!(lock_path.exists());
        assert!(!path.exists());

        fs::remove_file(lock_path).unwrap();
    }

    #[tokio::test]
    async fn file_local_session_store_deletes_only_the_requested_session() {
        let path = unique_test_path("session-delete");
        let store = FileLocalSessionStore::new(&path);
        let retained = LocalSession {
            id: "session_keep".into(),
            user_id: "local_user_bootstrap_admin".into(),
            secret_hash: "$argon2id$keep".into(),
            created_at: "2026-04-16T18:00:00Z".into(),
        };
        let deleted = LocalSession {
            id: "session_drop".into(),
            user_id: "local_user_bootstrap_admin".into(),
            secret_hash: "$argon2id$drop".into(),
            created_at: "2026-04-16T18:01:00Z".into(),
        };

        store.store_local_session(retained.clone()).await.unwrap();
        store.store_local_session(deleted.clone()).await.unwrap();

        assert!(store.delete_local_session(&deleted.id).await.unwrap());
        assert_eq!(store.local_session(&deleted.id).await.unwrap(), None);
        assert_eq!(
            store.local_session(&retained.id).await.unwrap(),
            Some(retained.clone())
        );

        let persisted: LocalSessionState =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(persisted.sessions, vec![retained]);

        fs::remove_file(path).unwrap();
    }

    async fn session_test_pool() -> sqlx::PgPool {
        let database_url = env::var("DATABASE_URL")
            .or_else(|_| env::var("TEST_DATABASE_URL"))
            .expect(
                "DATABASE_URL or TEST_DATABASE_URL must be set for Postgres session-store tests",
            );
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
            "TRUNCATE TABLE sessions, local_accounts, organizations RESTART IDENTITY CASCADE",
        )
        .execute(&pool)
        .await
        .expect("reset local session test tables");
        sqlx::query("INSERT INTO organizations (id, slug, name) VALUES ($1, $2, $3)")
            .bind("org_acme")
            .bind("acme")
            .bind("Acme")
            .execute(&pool)
            .await
            .expect("seed organization");
        sqlx::query(
            "INSERT INTO local_accounts (id, email, name, created_at) VALUES ($1, $2, $3, $4::timestamptz)",
        )
        .bind(LOCAL_BOOTSTRAP_ADMIN_USER_ID)
        .bind("admin@example.com")
        .bind("Bootstrap Admin")
        .bind("2026-04-16T17:59:00Z")
        .execute(&pool)
        .await
        .expect("seed local account");
        pool
    }

    async fn bootstrap_test_pool() -> sqlx::PgPool {
        let database_url = env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL must be set for Postgres bootstrap-store tests");
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
            "TRUNCATE TABLE sessions, local_accounts, organizations RESTART IDENTITY CASCADE",
        )
        .execute(&pool)
        .await
        .expect("reset bootstrap test tables");
        pool
    }

    async fn persisted_bootstrap_row(
        pool: &sqlx::PgPool,
    ) -> Option<(String, String, String, String)> {
        sqlx::query(
            "SELECT email, name, password_hash, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at FROM local_accounts WHERE id = $1",
        )
        .bind(LOCAL_BOOTSTRAP_ADMIN_USER_ID)
        .fetch_optional(pool)
        .await
        .expect("read persisted bootstrap account row")
        .map(|row| {
            (
                row.get("email"),
                row.get("name"),
                row.get("password_hash"),
                row.get("created_at"),
            )
        })
    }

    async fn persisted_sessions(pool: &sqlx::PgPool) -> Vec<LocalSession> {
        let rows = sqlx::query(
            "SELECT id, user_id, secret_hash, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at FROM sessions ORDER BY id",
        )
        .fetch_all(pool)
        .await
        .expect("read persisted sessions");

        rows.into_iter()
            .map(|row| LocalSession {
                id: row.get("id"),
                user_id: row.get("user_id"),
                secret_hash: row.get("secret_hash"),
                created_at: row.get("created_at"),
            })
            .collect()
    }

    #[tokio::test]
    async fn pg_local_session_store_persists_and_reads_local_sessions() {
        let pool = session_test_pool().await;
        let store = PgLocalSessionStore { pool: pool.clone() };
        let session = LocalSession {
            id: "session_1".into(),
            user_id: "local_user_bootstrap_admin".into(),
            secret_hash: "$argon2id$session-secret".into(),
            created_at: "2026-04-16T18:00:00Z".into(),
        };

        store.store_local_session(session.clone()).await.unwrap();

        assert_eq!(
            store.local_session(&session.id).await.unwrap(),
            Some(session.clone())
        );
        assert_eq!(persisted_sessions(&pool).await, vec![session]);
    }

    #[tokio::test]
    async fn pg_local_session_store_persists_truthful_local_account_rows() {
        let pool = session_test_pool().await;
        let store = PgLocalSessionStore { pool: pool.clone() };

        store
            .persist_local_session_account(LocalAccount {
                id: "local_user_bootstrap_admin".into(),
                email: "admin@example.com".into(),
                name: "Admin User".into(),
                password_hash: None,
                created_at: "2026-04-16T17:00:00Z".into(),
            })
            .await
            .unwrap();

        let row = sqlx::query(
            "SELECT email, name, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at FROM local_accounts WHERE id = $1",
        )
        .bind("local_user_bootstrap_admin")
        .fetch_one(&pool)
        .await
        .expect("read persisted local account row");

        assert_eq!(row.get::<String, _>("email"), "admin@example.com");
        assert_eq!(row.get::<String, _>("name"), "Admin User");
        assert_eq!(row.get::<String, _>("created_at"), "2026-04-16T17:00:00Z");
    }

    #[test]
    fn try_build_local_session_store_rejects_invalid_database_url() {
        let result =
            try_build_local_session_store(unique_test_path("invalid-database-url"), Some("not a database url"));

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn pg_local_session_store_rewrites_existing_session_with_same_id() {
        let pool = session_test_pool().await;
        let store = PgLocalSessionStore { pool: pool.clone() };

        store
            .store_local_session(LocalSession {
                id: "session_1".into(),
                user_id: "local_user_bootstrap_admin".into(),
                secret_hash: "$argon2id$original".into(),
                created_at: "2026-04-16T18:00:00Z".into(),
            })
            .await
            .unwrap();

        let updated = LocalSession {
            id: "session_1".into(),
            user_id: "local_user_bootstrap_admin".into(),
            secret_hash: "$argon2id$rotated".into(),
            created_at: "2026-04-16T19:00:00Z".into(),
        };

        store.store_local_session(updated.clone()).await.unwrap();

        assert_eq!(
            store.local_session("session_1").await.unwrap(),
            Some(updated.clone())
        );
        assert_eq!(persisted_sessions(&pool).await, vec![updated]);
    }

    #[tokio::test]
    async fn pg_local_session_store_deletes_only_requested_session() {
        let pool = session_test_pool().await;
        let store = PgLocalSessionStore { pool: pool.clone() };
        let retained = LocalSession {
            id: "session_keep".into(),
            user_id: "local_user_bootstrap_admin".into(),
            secret_hash: "$argon2id$keep".into(),
            created_at: "2026-04-16T18:00:00Z".into(),
        };
        let deleted = LocalSession {
            id: "session_drop".into(),
            user_id: "local_user_bootstrap_admin".into(),
            secret_hash: "$argon2id$drop".into(),
            created_at: "2026-04-16T18:01:00Z".into(),
        };

        store.store_local_session(retained.clone()).await.unwrap();
        store.store_local_session(deleted.clone()).await.unwrap();

        assert!(store.delete_local_session(&deleted.id).await.unwrap());
        assert_eq!(store.local_session(&deleted.id).await.unwrap(), None);
        assert_eq!(
            store.local_session(&retained.id).await.unwrap(),
            Some(retained.clone())
        );
        assert_eq!(persisted_sessions(&pool).await, vec![retained]);
    }

    #[tokio::test]
    async fn file_organization_store_persists_and_reads_organization_state() {
        let path = unique_test_path("organization-persist");
        let store = FileOrganizationStore::new(&path);
        let state = OrganizationState {
            organizations: vec![Organization {
                id: "org_acme".into(),
                name: "Acme".into(),
                slug: "acme".into(),
            }],
            connections: vec![Connection {
                id: "conn_github".into(),
                name: "GitHub Cloud".into(),
                kind: ConnectionKind::GitHub,
                config: Some(ConnectionConfig::GitHub {
                    base_url: "https://github.com".into(),
                }),
            }],
            memberships: vec![OrganizationMembership {
                organization_id: "org_acme".into(),
                user_id: "local_user_bootstrap_admin".into(),
                role: OrganizationRole::Admin,
                joined_at: "2026-04-16T20:00:00Z".into(),
            }],
            accounts: vec![LocalAccount {
                id: "local_user_bootstrap_admin".into(),
                email: "admin@example.com".into(),
                name: "Bootstrap Admin".into(),
                password_hash: None,
                created_at: "2026-04-16T19:58:00Z".into(),
            }],
            invites: vec![OrganizationInvite {
                id: "invite_member".into(),
                organization_id: "org_acme".into(),
                email: "member@example.com".into(),
                role: OrganizationRole::Viewer,
                invited_by_user_id: "local_user_bootstrap_admin".into(),
                created_at: "2026-04-16T20:05:00Z".into(),
                expires_at: "2026-04-23T20:05:00Z".into(),
                accepted_by_user_id: Some("local_user_member".into()),
                accepted_at: Some("2026-04-17T08:00:00Z".into()),
            }],
            api_keys: vec![ApiKey {
                id: "key_ci".into(),
                user_id: "local_user_bootstrap_admin".into(),
                name: "CI key".into(),
                secret_hash: "$argon2id$v=19$m=19456,t=2,p=1$ci$hash".into(),
                created_at: "2026-04-18T09:45:00Z".into(),
                revoked_at: Some("2026-04-19T09:45:00Z".into()),
                repo_scope: vec!["repo_sourcebot_rewrite".into()],
            }],
            oauth_clients: vec![OAuthClient {
                id: "oauth_client_acme_web".into(),
                organization_id: "org_acme".into(),
                name: "Acme Web App".into(),
                client_id: "acme-web-client".into(),
                client_secret_hash: "$argon2id$v=19$m=19456,t=2,p=1$oauth$hash".into(),
                redirect_uris: vec!["https://app.acme.test/callback".into()],
                created_by_user_id: "local_user_bootstrap_admin".into(),
                created_at: "2026-04-18T09:46:00Z".into(),
                revoked_at: Some("2026-04-20T09:46:00Z".into()),
            }],
            search_contexts: vec![SearchContext {
                id: "ctx_backend".into(),
                user_id: "local_user_bootstrap_admin".into(),
                name: "Backend repos".into(),
                repo_scope: vec!["repo_sourcebot_rewrite".into()],
                created_at: "2026-04-18T09:50:00Z".into(),
                updated_at: "2026-04-19T09:50:00Z".into(),
            }],
            audit_events: vec![AuditEvent {
                id: "audit_key_ci_created".into(),
                organization_id: "org_acme".into(),
                actor: AuditActor {
                    user_id: Some("local_user_bootstrap_admin".into()),
                    api_key_id: Some("key_ci".into()),
                },
                action: "auth.api_key.created".into(),
                target_type: "api_key".into(),
                target_id: "key_ci".into(),
                occurred_at: "2026-04-18T09:45:00Z".into(),
                metadata: serde_json::json!({
                    "name": "CI key",
                    "repo_scope": ["repo_sourcebot_rewrite"]
                }),
            }],
            analytics_records: vec![AnalyticsRecord {
                id: "analytics_api_key_count".into(),
                organization_id: "org_acme".into(),
                metric: "auth.api_key.count".into(),
                recorded_at: "2026-04-19T10:00:00Z".into(),
                value: serde_json::json!({
                    "count": 1
                }),
                dimensions: serde_json::json!({
                    "source": "migration_seed"
                }),
            }],
            review_webhooks: vec![ReviewWebhook {
                id: "webhook_review_1".into(),
                organization_id: "org_acme".into(),
                connection_id: "conn_github".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                events: vec!["pull_request".into()],
                secret_hash: "$argon2id$v=19$m=19456,t=2,p=1$review$hash".into(),
                created_by_user_id: "local_user_bootstrap_admin".into(),
                created_at: "2026-04-19T10:05:00Z".into(),
            }],
            review_webhook_delivery_attempts: vec![ReviewWebhookDeliveryAttempt {
                id: "delivery_attempt_1".into(),
                webhook_id: "webhook_review_1".into(),
                connection_id: "conn_github".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                event_type: "pull_request_review".into(),
                review_id: "review_123".into(),
                external_event_id: "evt_123".into(),
                accepted_at: "2026-04-25T00:10:00Z".into(),
            }],
            review_agent_runs: vec![ReviewAgentRun {
                id: "review_agent_run_1".into(),
                organization_id: "org_acme".into(),
                webhook_id: "webhook_review_1".into(),
                delivery_attempt_id: "delivery_attempt_1".into(),
                connection_id: "conn_github".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                review_id: "review_123".into(),
                status: ReviewAgentRunStatus::Queued,
                created_at: "2026-04-25T00:10:05Z".into(),
            }],
            repo_permissions: vec![RepositoryPermissionBinding {
                organization_id: "org_acme".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                synced_at: "2026-04-18T09:30:00Z".into(),
            }],
            repository_sync_jobs: vec![RepositorySyncJob {
                id: "sync_job_1".into(),
                organization_id: "org_acme".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                connection_id: "conn_github".into(),
                status: RepositorySyncJobStatus::Failed,
                queued_at: "2026-04-26T10:00:00Z".into(),
                started_at: Some("2026-04-26T10:01:00Z".into()),
                finished_at: Some("2026-04-26T10:02:00Z".into()),
                error: Some("remote rejected fetch".into()),
            }],
        };

        store.store_organization_state(state.clone()).await.unwrap();

        let persisted: OrganizationState =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(persisted, state);
        assert_eq!(store.organization_state().await.unwrap(), state);

        fs::remove_file(path).unwrap();
    }

    fn repository_sync_job(
        id: &str,
        status: RepositorySyncJobStatus,
        queued_at: &str,
    ) -> RepositorySyncJob {
        RepositorySyncJob {
            id: id.into(),
            organization_id: "org_acme".into(),
            repository_id: format!("repo_{id}"),
            connection_id: "conn_github".into(),
            status,
            queued_at: queued_at.into(),
            started_at: None,
            finished_at: None,
            error: None,
        }
    }

    #[tokio::test]
    async fn file_organization_store_stores_new_repository_sync_job_durably() {
        let path = unique_test_path("organization-store-new-repository-sync-job");
        let store = FileOrganizationStore::new(&path);
        let job = repository_sync_job(
            "sync_job_1",
            RepositorySyncJobStatus::Queued,
            "2026-04-26T10:00:00Z",
        );

        store.store_repository_sync_job(job.clone()).await.unwrap();

        let persisted: OrganizationState =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(persisted.repository_sync_jobs, vec![job.clone()]);
        assert_eq!(store.organization_state().await.unwrap(), persisted);

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_updates_repository_sync_job_in_place_by_id() {
        let path = unique_test_path("organization-store-update-repository-sync-job");
        let store = FileOrganizationStore::new(&path);
        let original = repository_sync_job(
            "sync_job_1",
            RepositorySyncJobStatus::Queued,
            "2026-04-26T10:00:00Z",
        );
        let updated = RepositorySyncJob {
            status: RepositorySyncJobStatus::Succeeded,
            started_at: Some("2026-04-26T10:01:00Z".into()),
            finished_at: Some("2026-04-26T10:02:00Z".into()),
            ..original.clone()
        };

        store.store_repository_sync_job(original).await.unwrap();
        store
            .store_repository_sync_job(updated.clone())
            .await
            .unwrap();

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(persisted.repository_sync_jobs, vec![updated]);

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_preserves_unrelated_repository_sync_jobs_when_upserting() {
        let path = unique_test_path("organization-store-preserve-unrelated-repository-sync-jobs");
        let store = FileOrganizationStore::new(&path);
        let retained = repository_sync_job(
            "sync_job_keep",
            RepositorySyncJobStatus::Failed,
            "2026-04-26T10:00:00Z",
        );
        let original = repository_sync_job(
            "sync_job_update",
            RepositorySyncJobStatus::Queued,
            "2026-04-26T10:01:00Z",
        );
        let updated = RepositorySyncJob {
            status: RepositorySyncJobStatus::Running,
            started_at: Some("2026-04-26T10:02:00Z".into()),
            ..original.clone()
        };

        store
            .store_organization_state(OrganizationState {
                repository_sync_jobs: vec![retained.clone(), original],
                ..OrganizationState::default()
            })
            .await
            .unwrap();

        store
            .store_repository_sync_job(updated.clone())
            .await
            .unwrap();

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(persisted.repository_sync_jobs, vec![retained, updated]);

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_claims_oldest_queued_repository_sync_job_and_persists_running_status(
    ) {
        let path = unique_test_path("organization-store-claim-repository-sync-job");
        let store = FileOrganizationStore::new(&path);
        let retained_running = RepositorySyncJob {
            status: RepositorySyncJobStatus::Running,
            queued_at: "2026-04-26T10:00:00Z".into(),
            started_at: Some("2026-04-26T10:01:00Z".into()),
            ..repository_sync_job(
                "sync_job_running",
                RepositorySyncJobStatus::Running,
                "2026-04-26T10:00:00Z",
            )
        };
        let queued_newer = repository_sync_job(
            "sync_job_newer",
            RepositorySyncJobStatus::Queued,
            "2026-04-26T10:02:00Z",
        );
        let queued_oldest = repository_sync_job(
            "sync_job_oldest",
            RepositorySyncJobStatus::Queued,
            "2026-04-26T10:01:00Z",
        );

        store
            .store_organization_state(OrganizationState {
                repository_sync_jobs: vec![retained_running.clone(), queued_newer, queued_oldest],
                ..OrganizationState::default()
            })
            .await
            .unwrap();

        let claimed_job = store
            .claim_next_repository_sync_job("2026-04-26T10:03:00Z")
            .await
            .unwrap()
            .expect("queued repository sync job to be claimed");

        assert_eq!(claimed_job.id, "sync_job_oldest");
        assert_eq!(claimed_job.status, RepositorySyncJobStatus::Running);
        assert_eq!(
            claimed_job.started_at.as_deref(),
            Some("2026-04-26T10:03:00Z")
        );
        assert_eq!(claimed_job.finished_at, None);
        assert_eq!(claimed_job.error, None);

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(persisted.repository_sync_jobs.len(), 3);
        assert_eq!(persisted.repository_sync_jobs[0], retained_running);
        assert_eq!(
            persisted.repository_sync_jobs[1].status,
            RepositorySyncJobStatus::Queued
        );
        assert_eq!(persisted.repository_sync_jobs[1].started_at, None);
        assert_eq!(persisted.repository_sync_jobs[2], claimed_job);

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_preserves_running_repository_sync_job_when_stale_state_is_written(
    ) {
        let path = unique_test_path("organization-store-preserves-running-repository-sync-job");
        let writer_store = FileOrganizationStore::new(&path);
        let worker_store = FileOrganizationStore::new(&path);

        writer_store
            .store_organization_state(OrganizationState {
                repository_sync_jobs: vec![repository_sync_job(
                    "sync_job_queued",
                    RepositorySyncJobStatus::Queued,
                    "2026-04-26T10:01:00Z",
                )],
                ..OrganizationState::default()
            })
            .await
            .unwrap();

        let stale_state = writer_store.organization_state().await.unwrap();

        let claimed_job = worker_store
            .claim_next_repository_sync_job("2026-04-26T10:03:00Z")
            .await
            .unwrap()
            .expect("queued repository sync job to be claimed");
        assert_eq!(claimed_job.status, RepositorySyncJobStatus::Running);
        assert_eq!(
            claimed_job.started_at.as_deref(),
            Some("2026-04-26T10:03:00Z")
        );

        writer_store
            .store_organization_state(stale_state)
            .await
            .unwrap();

        let persisted = writer_store.organization_state().await.unwrap();
        assert_eq!(persisted.repository_sync_jobs.len(), 1);
        assert_eq!(persisted.repository_sync_jobs[0], claimed_job);

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_preserves_new_repository_sync_job_when_stale_state_is_written()
    {
        let path = unique_test_path("organization-store-preserves-new-repository-sync-job");
        let writer_store = FileOrganizationStore::new(&path);
        let worker_store = FileOrganizationStore::new(&path);

        let original_job = repository_sync_job(
            "sync_job_original",
            RepositorySyncJobStatus::Queued,
            "2026-04-26T10:01:00Z",
        );
        writer_store
            .store_organization_state(OrganizationState {
                repository_sync_jobs: vec![original_job.clone()],
                ..OrganizationState::default()
            })
            .await
            .unwrap();

        let stale_state = writer_store.organization_state().await.unwrap();

        let inserted_job = repository_sync_job(
            "sync_job_inserted",
            RepositorySyncJobStatus::Queued,
            "2026-04-26T10:02:00Z",
        );
        worker_store
            .store_repository_sync_job(inserted_job.clone())
            .await
            .unwrap();

        writer_store
            .store_organization_state(stale_state)
            .await
            .unwrap();

        let persisted = writer_store.organization_state().await.unwrap();
        assert_eq!(
            persisted.repository_sync_jobs,
            vec![original_job, inserted_job]
        );

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_claims_one_oldest_queued_review_agent_run() {
        let path = unique_test_path("organization-claim-review-agent-run");
        let store = FileOrganizationStore::new(&path);
        let state = OrganizationState {
            review_agent_runs: vec![
                ReviewAgentRun {
                    id: "run_newer".into(),
                    organization_id: "org_acme".into(),
                    webhook_id: "webhook_review_1".into(),
                    delivery_attempt_id: "delivery_attempt_newer".into(),
                    connection_id: "conn_github".into(),
                    repository_id: "repo_sourcebot_rewrite".into(),
                    review_id: "review_newer".into(),
                    status: ReviewAgentRunStatus::Queued,
                    created_at: "2026-04-25T00:10:06Z".into(),
                },
                ReviewAgentRun {
                    id: "run_oldest".into(),
                    organization_id: "org_acme".into(),
                    webhook_id: "webhook_review_1".into(),
                    delivery_attempt_id: "delivery_attempt_oldest".into(),
                    connection_id: "conn_github".into(),
                    repository_id: "repo_sourcebot_rewrite".into(),
                    review_id: "review_oldest".into(),
                    status: ReviewAgentRunStatus::Queued,
                    created_at: "2026-04-25T00:10:05Z".into(),
                },
                ReviewAgentRun {
                    id: "run_already_claimed".into(),
                    organization_id: "org_acme".into(),
                    webhook_id: "webhook_review_1".into(),
                    delivery_attempt_id: "delivery_attempt_claimed".into(),
                    connection_id: "conn_github".into(),
                    repository_id: "repo_sourcebot_rewrite".into(),
                    review_id: "review_claimed".into(),
                    status: ReviewAgentRunStatus::Claimed,
                    created_at: "2026-04-25T00:10:04Z".into(),
                },
            ],
            ..OrganizationState::default()
        };

        store.store_organization_state(state).await.unwrap();

        let claimed_run = store
            .claim_next_review_agent_run()
            .await
            .unwrap()
            .expect("queued run to be claimed");

        assert_eq!(claimed_run.id, "run_oldest");
        assert_eq!(claimed_run.status, ReviewAgentRunStatus::Claimed);

        let persisted: OrganizationState =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(persisted.review_agent_runs.len(), 3);
        assert_eq!(
            persisted.review_agent_runs[0].status,
            ReviewAgentRunStatus::Queued
        );
        assert_eq!(
            persisted.review_agent_runs[1].status,
            ReviewAgentRunStatus::Claimed
        );
        assert_eq!(
            persisted.review_agent_runs[2].status,
            ReviewAgentRunStatus::Claimed
        );
        assert_eq!(store.organization_state().await.unwrap(), persisted);

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_preserves_claimed_review_agent_run_when_stale_state_is_written(
    ) {
        let path = unique_test_path("organization-store-preserves-claimed-review-agent-run");
        let writer_store = FileOrganizationStore::new(&path);
        let claimer_store = FileOrganizationStore::new(&path);
        let initial_state = OrganizationState {
            review_agent_runs: vec![ReviewAgentRun {
                id: "run_oldest".into(),
                organization_id: "org_acme".into(),
                webhook_id: "webhook_review_1".into(),
                delivery_attempt_id: "delivery_attempt_oldest".into(),
                connection_id: "conn_github".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                review_id: "review_oldest".into(),
                status: ReviewAgentRunStatus::Queued,
                created_at: "2026-04-25T00:10:05Z".into(),
            }],
            ..OrganizationState::default()
        };

        writer_store
            .store_organization_state(initial_state)
            .await
            .unwrap();

        let stale_state = writer_store.organization_state().await.unwrap();

        let claimed_run = claimer_store
            .claim_next_review_agent_run()
            .await
            .unwrap()
            .expect("queued run to be claimed");
        assert_eq!(claimed_run.status, ReviewAgentRunStatus::Claimed);

        writer_store
            .store_organization_state(stale_state)
            .await
            .unwrap();

        let persisted = writer_store.organization_state().await.unwrap();
        assert_eq!(persisted.review_agent_runs.len(), 1);
        assert_eq!(persisted.review_agent_runs[0].id, "run_oldest");
        assert_eq!(
            persisted.review_agent_runs[0].status,
            ReviewAgentRunStatus::Claimed
        );

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_completes_a_claimed_review_agent_run() {
        let path = unique_test_path("organization-complete-review-agent-run");
        let store = FileOrganizationStore::new(&path);
        let state = OrganizationState {
            review_agent_runs: vec![ReviewAgentRun {
                id: "run_claimed".into(),
                organization_id: "org_acme".into(),
                webhook_id: "webhook_review_1".into(),
                delivery_attempt_id: "delivery_attempt_claimed".into(),
                connection_id: "conn_github".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                review_id: "review_claimed".into(),
                status: ReviewAgentRunStatus::Claimed,
                created_at: "2026-04-25T00:10:05Z".into(),
            }],
            ..OrganizationState::default()
        };

        store.store_organization_state(state).await.unwrap();

        let completed_run = store
            .complete_review_agent_run("run_claimed")
            .await
            .unwrap()
            .expect("claimed run to be completed");

        assert_eq!(completed_run.id, "run_claimed");
        assert_eq!(completed_run.status, ReviewAgentRunStatus::Completed);

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(persisted.review_agent_runs.len(), 1);
        assert_eq!(persisted.review_agent_runs[0].id, "run_claimed");
        assert_eq!(
            persisted.review_agent_runs[0].status,
            ReviewAgentRunStatus::Completed
        );

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_fails_a_claimed_review_agent_run() {
        let path = unique_test_path("organization-fail-review-agent-run");
        let store = FileOrganizationStore::new(&path);
        let state = OrganizationState {
            review_agent_runs: vec![ReviewAgentRun {
                id: "run_claimed".into(),
                organization_id: "org_acme".into(),
                webhook_id: "webhook_review_1".into(),
                delivery_attempt_id: "delivery_attempt_claimed".into(),
                connection_id: "conn_github".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                review_id: "review_claimed".into(),
                status: ReviewAgentRunStatus::Claimed,
                created_at: "2026-04-25T00:10:05Z".into(),
            }],
            ..OrganizationState::default()
        };

        store.store_organization_state(state).await.unwrap();

        let failed_run = store
            .fail_review_agent_run("run_claimed")
            .await
            .unwrap()
            .expect("claimed run to be failed");

        assert_eq!(failed_run.id, "run_claimed");
        assert_eq!(failed_run.status, ReviewAgentRunStatus::Failed);

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(persisted.review_agent_runs.len(), 1);
        assert_eq!(persisted.review_agent_runs[0].id, "run_claimed");
        assert_eq!(
            persisted.review_agent_runs[0].status,
            ReviewAgentRunStatus::Failed
        );

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_preserves_completed_review_agent_run_when_stale_state_is_written(
    ) {
        let path = unique_test_path("organization-store-preserves-completed-review-agent-run");
        let writer_store = FileOrganizationStore::new(&path);
        let worker_store = FileOrganizationStore::new(&path);
        let initial_state = OrganizationState {
            review_agent_runs: vec![ReviewAgentRun {
                id: "run_claimed".into(),
                organization_id: "org_acme".into(),
                webhook_id: "webhook_review_1".into(),
                delivery_attempt_id: "delivery_attempt_claimed".into(),
                connection_id: "conn_github".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                review_id: "review_claimed".into(),
                status: ReviewAgentRunStatus::Claimed,
                created_at: "2026-04-25T00:10:05Z".into(),
            }],
            ..OrganizationState::default()
        };

        writer_store
            .store_organization_state(initial_state)
            .await
            .unwrap();

        let stale_state = writer_store.organization_state().await.unwrap();

        let completed_run = worker_store
            .complete_review_agent_run("run_claimed")
            .await
            .unwrap()
            .expect("claimed run to be completed");
        assert_eq!(completed_run.status, ReviewAgentRunStatus::Completed);

        writer_store
            .store_organization_state(stale_state)
            .await
            .unwrap();

        let persisted = writer_store.organization_state().await.unwrap();
        assert_eq!(persisted.review_agent_runs.len(), 1);
        assert_eq!(persisted.review_agent_runs[0].id, "run_claimed");
        assert_eq!(
            persisted.review_agent_runs[0].status,
            ReviewAgentRunStatus::Completed
        );

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_preserves_failed_review_agent_run_when_stale_state_is_written()
    {
        let path = unique_test_path("organization-store-preserves-failed-review-agent-run");
        let writer_store = FileOrganizationStore::new(&path);
        let worker_store = FileOrganizationStore::new(&path);
        let initial_state = OrganizationState {
            review_agent_runs: vec![ReviewAgentRun {
                id: "run_claimed".into(),
                organization_id: "org_acme".into(),
                webhook_id: "webhook_review_1".into(),
                delivery_attempt_id: "delivery_attempt_claimed".into(),
                connection_id: "conn_github".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                review_id: "review_claimed".into(),
                status: ReviewAgentRunStatus::Claimed,
                created_at: "2026-04-25T00:10:05Z".into(),
            }],
            ..OrganizationState::default()
        };

        writer_store
            .store_organization_state(initial_state)
            .await
            .unwrap();

        let stale_state = writer_store.organization_state().await.unwrap();

        let failed_run = worker_store
            .fail_review_agent_run("run_claimed")
            .await
            .unwrap()
            .expect("claimed run to be failed");
        assert_eq!(failed_run.status, ReviewAgentRunStatus::Failed);

        writer_store
            .store_organization_state(stale_state)
            .await
            .unwrap();

        let persisted = writer_store.organization_state().await.unwrap();
        assert_eq!(persisted.review_agent_runs.len(), 1);
        assert_eq!(persisted.review_agent_runs[0].id, "run_claimed");
        assert_eq!(
            persisted.review_agent_runs[0].status,
            ReviewAgentRunStatus::Failed
        );

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_preserves_persisted_failed_run_against_stale_completed_write()
    {
        let path = unique_test_path("organization-store-preserves-failed-over-completed");
        let writer_store = FileOrganizationStore::new(&path);
        let worker_store = FileOrganizationStore::new(&path);
        let initial_state = OrganizationState {
            review_agent_runs: vec![ReviewAgentRun {
                id: "run_terminal".into(),
                organization_id: "org_acme".into(),
                webhook_id: "webhook_review_1".into(),
                delivery_attempt_id: "delivery_attempt_terminal".into(),
                connection_id: "conn_github".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                review_id: "review_terminal".into(),
                status: ReviewAgentRunStatus::Claimed,
                created_at: "2026-04-25T00:10:05Z".into(),
            }],
            ..OrganizationState::default()
        };

        writer_store
            .store_organization_state(initial_state)
            .await
            .unwrap();

        let mut stale_state = writer_store.organization_state().await.unwrap();

        let failed_run = worker_store
            .fail_review_agent_run("run_terminal")
            .await
            .unwrap()
            .expect("claimed run to be failed");
        assert_eq!(failed_run.status, ReviewAgentRunStatus::Failed);

        stale_state.review_agent_runs[0].status = ReviewAgentRunStatus::Completed;
        writer_store
            .store_organization_state(stale_state)
            .await
            .unwrap();

        let persisted = writer_store.organization_state().await.unwrap();
        assert_eq!(persisted.review_agent_runs.len(), 1);
        assert_eq!(persisted.review_agent_runs[0].id, "run_terminal");
        assert_eq!(
            persisted.review_agent_runs[0].status,
            ReviewAgentRunStatus::Failed
        );

        fs::remove_file(path).unwrap();
    }
}
