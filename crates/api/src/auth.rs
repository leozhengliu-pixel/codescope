use anyhow::Result;
use async_trait::async_trait;
use sourcebot_core::BootstrapStore;
use sourcebot_models::{BootstrapState, BootstrapStatus};
use std::{
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

pub type DynBootstrapStore = Arc<dyn BootstrapStore>;

#[derive(Clone, Debug)]
pub struct FileBootstrapStore {
    state_path: PathBuf,
}

impl FileBootstrapStore {
    pub fn new(state_path: impl Into<PathBuf>) -> Self {
        Self {
            state_path: state_path.into(),
        }
    }

    fn temporary_state_path(&self) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let file_name = self
            .state_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("bootstrap-state.json");
        self.state_path
            .with_file_name(format!(".{file_name}.{nanos}.tmp"))
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

fn open_new_bootstrap_file(path: &Path) -> std::io::Result<File> {
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
        if let Some(parent) = self.state_path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }

        let payload = serde_json::to_vec_pretty(&state)?;
        let temp_path = self.temporary_state_path();
        let write_result = (|| -> std::io::Result<()> {
            let mut file = open_new_bootstrap_file(&temp_path)?;
            file.write_all(&payload)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            drop(file);

            fs::hard_link(&temp_path, &self.state_path)?;
            sync_parent_directory(&self.state_path)?;
            fs::remove_file(&temp_path)?;
            Ok(())
        })();

        if let Err(error) = write_result {
            match fs::remove_file(&temp_path) {
                Ok(()) => {}
                Err(remove_error) if remove_error.kind() == ErrorKind::NotFound => {}
                Err(remove_error) => return Err(remove_error.into()),
            }
            return Err(error.into());
        }

        Ok(())
    }
}

pub fn build_bootstrap_store(state_path: impl Into<PathBuf>) -> DynBootstrapStore {
    Arc::new(FileBootstrapStore::new(state_path))
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
