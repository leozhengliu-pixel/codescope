use anyhow::Result;
use async_trait::async_trait;
use sourcebot_core::BootstrapStore;
use sourcebot_models::BootstrapStatus;
use std::{path::PathBuf, sync::Arc};

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
}

#[async_trait]
impl BootstrapStore for FileBootstrapStore {
    async fn bootstrap_status(&self) -> Result<BootstrapStatus> {
        Ok(BootstrapStatus {
            bootstrap_required: !self.state_path.is_file(),
        })
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
    async fn file_bootstrap_store_disables_bootstrap_when_state_file_exists() {
        let path = unique_test_path("present");
        fs::write(&path, b"initialized").unwrap();

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
}
