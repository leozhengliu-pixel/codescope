use anyhow::{anyhow, Result};
use async_trait::async_trait;
use sourcebot_core::{BootstrapStore, LocalSessionStore, OrganizationStore};
use sourcebot_models::{
    BootstrapState, BootstrapStatus, LocalSession, LocalSessionState, OrganizationState,
};
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

#[derive(Debug)]
struct LocalSessionWriteLock {
    file: File,
    lock_path: PathBuf,
}

impl Drop for LocalSessionWriteLock {
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

    fn acquire_write_lock(&self) -> Result<LocalSessionWriteLock> {
        const MAX_LOCK_WAIT: Duration = Duration::from_millis(100);
        const LOCK_RETRY_DELAY: Duration = Duration::from_millis(10);

        ensure_parent_directory(&self.state_path)?;
        let lock_path = self.lock_path();
        let start = SystemTime::now();

        loop {
            match open_new_private_file(&lock_path) {
                Ok(file) => return Ok(LocalSessionWriteLock { file, lock_path }),
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

impl FileOrganizationStore {
    pub fn new(state_path: impl Into<PathBuf>) -> Self {
        Self {
            state_path: state_path.into(),
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
impl OrganizationStore for FileOrganizationStore {
    async fn organization_state(&self) -> Result<OrganizationState> {
        self.read_persisted_state()
    }

    async fn store_organization_state(&self, state: OrganizationState) -> Result<()> {
        let payload = serde_json::to_vec_pretty(&state)?;
        write_json_file(&self.state_path, &payload, true)?;
        Ok(())
    }
}

pub fn build_bootstrap_store(state_path: impl Into<PathBuf>) -> DynBootstrapStore {
    Arc::new(FileBootstrapStore::new(state_path))
}

pub fn build_local_session_store(state_path: impl Into<PathBuf>) -> DynLocalSessionStore {
    Arc::new(FileLocalSessionStore::new(state_path))
}

pub fn build_organization_store(state_path: impl Into<PathBuf>) -> DynOrganizationStore {
    Arc::new(FileOrganizationStore::new(state_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sourcebot_models::{
        AnalyticsRecord, ApiKey, AuditActor, AuditEvent, LocalAccount, OAuthClient, Organization,
        OrganizationInvite, OrganizationMembership, OrganizationRole, RepositoryPermissionBinding,
        SearchContext,
    };
    use std::{
        fs,
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
            repo_permissions: vec![RepositoryPermissionBinding {
                organization_id: "org_acme".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                synced_at: "2026-04-18T09:30:00Z".into(),
            }],
        };

        store.store_organization_state(state.clone()).await.unwrap();

        let persisted: OrganizationState =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(persisted, state);
        assert_eq!(store.organization_state().await.unwrap(), state);

        fs::remove_file(path).unwrap();
    }
}
