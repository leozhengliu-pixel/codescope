use anyhow::{Context, Result};
use serde::Serialize;
use std::{
    collections::HashMap,
    fs,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

pub type DynBrowseStore = Arc<dyn BrowseStore>;

const SOURCEBOT_REWRITE_REPO_ID: &str = "repo_sourcebot_rewrite";
const SOURCEBOT_REWRITE_ROOT: &str = "/opt/data/projects/sourcebot-rewrite";

pub trait BrowseStore: Send + Sync {
    fn get_tree(&self, repo_id: &str, path: &str) -> Result<Option<TreeResponse>>;
    fn get_blob(&self, repo_id: &str, path: &str) -> Result<Option<BlobResponse>>;
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TreeEntry {
    pub name: String,
    pub path: String,
    pub kind: EntryKind,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    File,
    Dir,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TreeResponse {
    pub repo_id: String,
    pub path: String,
    pub entries: Vec<TreeEntry>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BlobResponse {
    pub repo_id: String,
    pub path: String,
    pub content: String,
    pub size_bytes: u64,
}

#[derive(Clone, Default)]
pub struct LocalBrowseStore {
    repo_roots: HashMap<String, PathBuf>,
}

impl LocalBrowseStore {
    pub fn new(repo_roots: HashMap<String, PathBuf>) -> Self {
        Self { repo_roots }
    }

    pub fn seeded() -> Self {
        Self::new(HashMap::from([(
            SOURCEBOT_REWRITE_REPO_ID.to_string(),
            PathBuf::from(SOURCEBOT_REWRITE_ROOT),
        )]))
    }

    fn resolve_path(&self, repo_id: &str, relative_path: &str) -> Result<Option<PathBuf>> {
        let Some(root) = self.repo_roots.get(repo_id) else {
            return Ok(None);
        };

        let safe_relative = normalize_relative_path(relative_path)?;
        Ok(Some(root.join(safe_relative)))
    }
}

impl BrowseStore for LocalBrowseStore {
    fn get_tree(&self, repo_id: &str, path: &str) -> Result<Option<TreeResponse>> {
        let Some(full_path) = self.resolve_path(repo_id, path)? else {
            return Ok(None);
        };

        if !full_path.exists() || !full_path.is_dir() {
            return Ok(None);
        }

        let mut entries = fs::read_dir(&full_path)
            .with_context(|| format!("failed to read directory {}", full_path.display()))?
            .map(|entry| {
                let entry = entry?;
                let file_type = entry.file_type()?;
                let name = entry.file_name().to_string_lossy().into_owned();
                let entry_path = join_relative_path(path, &name);
                let kind = if file_type.is_dir() {
                    EntryKind::Dir
                } else {
                    EntryKind::File
                };

                Ok(TreeEntry {
                    name,
                    path: entry_path,
                    kind,
                })
            })
            .collect::<Result<Vec<_>, std::io::Error>>()?;

        entries.sort_by(|left, right| left.path.cmp(&right.path));

        Ok(Some(TreeResponse {
            repo_id: repo_id.to_string(),
            path: path.to_string(),
            entries,
        }))
    }

    fn get_blob(&self, repo_id: &str, path: &str) -> Result<Option<BlobResponse>> {
        let Some(full_path) = self.resolve_path(repo_id, path)? else {
            return Ok(None);
        };

        if !full_path.exists() || !full_path.is_file() {
            return Ok(None);
        }

        let content = fs::read_to_string(&full_path)
            .with_context(|| format!("failed to read file {}", full_path.display()))?;
        let size_bytes = fs::metadata(&full_path)
            .with_context(|| format!("failed to read metadata for {}", full_path.display()))?
            .len();

        Ok(Some(BlobResponse {
            repo_id: repo_id.to_string(),
            path: path.to_string(),
            content,
            size_bytes,
        }))
    }
}

fn normalize_relative_path(relative_path: &str) -> Result<PathBuf> {
    let path = Path::new(relative_path);
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                anyhow::bail!("invalid relative path: {relative_path}");
            }
        }
    }

    Ok(normalized)
}

fn join_relative_path(parent: &str, child: &str) -> String {
    if parent.is_empty() {
        child.to_string()
    } else {
        format!("{parent}/{child}")
    }
}

pub fn build_browse_store() -> DynBrowseStore {
    Arc::new(LocalBrowseStore::seeded())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    fn unique_temp_dir() -> PathBuf {
        let suffix = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("sourcebot-browse-test-{nanos}-{suffix}"))
    }

    fn create_test_store() -> (LocalBrowseStore, PathBuf) {
        let root = unique_temp_dir();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("README.md"), "hello world\n").unwrap();
        fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();

        let store = LocalBrowseStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));
        (store, root)
    }

    #[test]
    fn local_browse_store_lists_tree_entries() {
        let (store, root) = create_test_store();

        let tree = store.get_tree("repo_test", "").unwrap().unwrap();

        assert_eq!(tree.path, "");
        assert!(tree.entries.iter().any(|entry| {
            entry.name == "README.md" && entry.path == "README.md" && entry.kind == EntryKind::File
        }));
        assert!(tree.entries.iter().any(|entry| {
            entry.name == "src" && entry.path == "src" && entry.kind == EntryKind::Dir
        }));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_browse_store_reads_blob_contents() {
        let (store, root) = create_test_store();

        let blob = store.get_blob("repo_test", "README.md").unwrap().unwrap();

        assert_eq!(blob.path, "README.md");
        assert_eq!(blob.content, "hello world\n");
        assert_eq!(blob.size_bytes, 12);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_browse_store_rejects_parent_directory_components() {
        let (store, root) = create_test_store();

        let error = store.get_tree("repo_test", "../etc").unwrap_err();
        assert!(error.to_string().contains("invalid relative path"));

        fs::remove_dir_all(root).unwrap();
    }
}
