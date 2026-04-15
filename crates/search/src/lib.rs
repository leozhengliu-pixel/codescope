use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

const DEFAULT_MAX_FILE_SIZE_BYTES: u64 = 1_000_000;
const SKIPPED_DIR_NAMES: &[&str] = &[".git", "target", "node_modules", "dist"];
const SOURCEBOT_REWRITE_REPO_ID: &str = "repo_sourcebot_rewrite";
const SOURCEBOT_REWRITE_ROOT: &str = "/opt/data/projects/sourcebot-rewrite";

pub type DynSearchStore = Arc<dyn SearchStore>;

pub trait SearchStore: Send + Sync {
    fn search(&self, query: &str, repo_id: Option<&str>) -> Result<SearchResponse>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchResult {
    pub repo_id: String,
    pub path: String,
    pub line_number: usize,
    pub line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchResponse {
    pub query: String,
    pub repo_id: Option<String>,
    pub results: Vec<SearchResult>,
}

#[derive(Clone)]
pub struct LocalSearchStore {
    repo_roots: HashMap<String, PathBuf>,
    max_file_size_bytes: u64,
}

impl LocalSearchStore {
    pub fn new(repo_roots: HashMap<String, PathBuf>) -> Self {
        Self {
            repo_roots,
            max_file_size_bytes: DEFAULT_MAX_FILE_SIZE_BYTES,
        }
    }

    pub fn seeded() -> Self {
        Self::new(HashMap::from([(
            SOURCEBOT_REWRITE_REPO_ID.to_string(),
            PathBuf::from(SOURCEBOT_REWRITE_ROOT),
        )]))
    }

    pub fn with_max_file_size_bytes(mut self, max_file_size_bytes: u64) -> Self {
        self.max_file_size_bytes = max_file_size_bytes;
        self
    }

    fn search_repo(&self, repo_id: &str, root: &Path, query: &str) -> Result<Vec<SearchResult>> {
        let mut results = Vec::new();
        self.collect_matches(repo_id, root, root, query, &mut results)?;
        Ok(results)
    }

    fn collect_matches(
        &self,
        repo_id: &str,
        root: &Path,
        current_path: &Path,
        query: &str,
        results: &mut Vec<SearchResult>,
    ) -> Result<()> {
        let entries = fs::read_dir(current_path)
            .with_context(|| format!("failed to read directory {}", current_path.display()))?;

        for entry in entries {
            let entry = entry.with_context(|| {
                format!(
                    "failed to read directory entry under {}",
                    current_path.display()
                )
            })?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .with_context(|| format!("failed to read file type for {}", path.display()))?;

            if file_type.is_dir() {
                if should_skip_directory(&path) {
                    continue;
                }

                self.collect_matches(repo_id, root, &path, query, results)?;
                continue;
            }

            if !file_type.is_file() || should_skip_file(&path) {
                continue;
            }

            let metadata = fs::metadata(&path)
                .with_context(|| format!("failed to read metadata for {}", path.display()))?;
            if metadata.len() > self.max_file_size_bytes {
                continue;
            }

            let Ok(contents) = fs::read_to_string(&path) else {
                continue;
            };

            if is_obviously_binary(&contents) {
                continue;
            }

            let relative_path = path
                .strip_prefix(root)
                .with_context(|| format!("failed to strip prefix for {}", path.display()))?
                .to_string_lossy()
                .replace('\\', "/");

            for (index, line) in contents.lines().enumerate() {
                if line.contains(query) {
                    results.push(SearchResult {
                        repo_id: repo_id.to_string(),
                        path: relative_path.clone(),
                        line_number: index + 1,
                        line: line.to_string(),
                    });
                }
            }
        }

        Ok(())
    }
}

impl SearchStore for LocalSearchStore {
    fn search(&self, query: &str, repo_id: Option<&str>) -> Result<SearchResponse> {
        let query = query.trim();
        let requested_repo_id = repo_id.map(str::trim).filter(|value| !value.is_empty());

        let repos: Vec<(&String, &PathBuf)> = match requested_repo_id {
            Some(repo_id) => self.repo_roots.get_key_value(repo_id).into_iter().collect(),
            None => self.repo_roots.iter().collect(),
        };

        let mut results = Vec::new();
        for (repo_id, root) in repos {
            results.extend(self.search_repo(repo_id, root, query)?);
        }

        Ok(SearchResponse {
            query: query.to_string(),
            repo_id: requested_repo_id.map(ToOwned::to_owned),
            results,
        })
    }
}

fn should_skip_file(path: &Path) -> bool {
    path.components().any(|component| {
        SKIPPED_DIR_NAMES
            .iter()
            .any(|name| component.as_os_str() == *name)
    }) || path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|ext| {
            matches!(
                ext,
                "png"
                    | "jpg"
                    | "jpeg"
                    | "gif"
                    | "webp"
                    | "ico"
                    | "pdf"
                    | "zip"
                    | "gz"
                    | "tar"
                    | "jar"
                    | "wasm"
                    | "so"
                    | "dll"
                    | "exe"
                    | "class"
                    | "lock"
            )
        })
}

fn should_skip_directory(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| SKIPPED_DIR_NAMES.iter().any(|skipped| name == *skipped))
}

fn is_obviously_binary(contents: &str) -> bool {
    contents.contains('\0')
}

pub fn build_search_store() -> DynSearchStore {
    Arc::new(LocalSearchStore::seeded())
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
        std::env::temp_dir().join(format!("sourcebot-search-test-{nanos}-{suffix}"))
    }

    fn create_test_store() -> (LocalSearchStore, PathBuf) {
        let root = unique_temp_dir();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join("target")).unwrap();
        fs::write(
            root.join("src").join("main.rs"),
            "fn build_router() {}\nfn other() {}\n",
        )
        .unwrap();
        fs::write(root.join("README.md"), "build_router is documented here\n").unwrap();
        fs::write(
            root.join(".git").join("HEAD"),
            "build_router should be ignored\n",
        )
        .unwrap();
        fs::write(
            root.join("target").join("generated.txt"),
            "build_router should also be ignored\n",
        )
        .unwrap();
        fs::write(root.join("image.png"), b"not really an image").unwrap();
        fs::write(root.join("binary.dat"), [0_u8, 159, 146, 150]).unwrap();

        let store = LocalSearchStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));
        (store, root)
    }

    #[test]
    fn local_search_store_returns_line_matches() {
        let (store, root) = create_test_store();

        let response = store.search("build_router", Some("repo_test")).unwrap();

        assert_eq!(response.query, "build_router");
        assert_eq!(response.repo_id.as_deref(), Some("repo_test"));
        assert!(response.results.iter().any(|result| {
            result.path == "src/main.rs"
                && result.line_number == 1
                && result.line == "fn build_router() {}"
        }));
        assert!(response
            .results
            .iter()
            .any(|result| result.path == "README.md" && result.line_number == 1));
        assert!(response
            .results
            .iter()
            .all(|result| result.path != "target/generated.txt"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_skips_git_binary_and_large_files() {
        let (store, root) = create_test_store();
        fs::write(root.join("large.txt"), "build_router\n".repeat(10)).unwrap();

        let response = store
            .with_max_file_size_bytes(5)
            .search("build_router", Some("repo_test"))
            .unwrap();

        assert!(response
            .results
            .iter()
            .all(|result| result.path != ".git/HEAD"));
        assert!(response
            .results
            .iter()
            .all(|result| result.path != "binary.dat"));
        assert!(response
            .results
            .iter()
            .all(|result| result.path != "large.txt"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_returns_empty_results_for_unknown_repo() {
        let (store, root) = create_test_store();

        let response = store.search("build_router", Some("missing_repo")).unwrap();

        assert!(response.results.is_empty());
        assert_eq!(response.repo_id.as_deref(), Some("missing_repo"));

        fs::remove_dir_all(root).unwrap();
    }
}
