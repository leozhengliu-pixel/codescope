use anyhow::{Context, Result};
use async_trait::async_trait;
use globset::Glob;
use serde::Serialize;
use sourcebot_core::{
    BlobStore, GlobStore, RepositoryBlob, RepositoryGlob, RepositoryTree, RepositoryTreeEntry,
    RepositoryTreeEntryKind, TreeStore,
};
use std::{
    collections::HashMap,
    fs,
    path::{Component, Path, PathBuf},
    process::{Command, Output},
    sync::Arc,
};

pub type DynBrowseStore = Arc<dyn BrowseStore>;

const SOURCEBOT_REWRITE_REPO_ID: &str = "repo_sourcebot_rewrite";
const SOURCEBOT_REWRITE_ROOT: &str = "/opt/data/projects/sourcebot-rewrite";

pub trait BrowseStore: Send + Sync {
    fn get_tree(&self, repo_id: &str, path: &str) -> Result<Option<TreeResponse>>;
    #[allow(dead_code)]
    fn get_blob(&self, repo_id: &str, path: &str) -> Result<Option<BlobResponse>>;
    #[allow(dead_code)]
    fn glob_paths(&self, repo_id: &str, pattern: &str) -> Result<Option<GlobResponse>>;
    fn get_blob_at_revision(
        &self,
        repo_id: &str,
        path: &str,
        revision: Option<&str>,
    ) -> Result<Option<BlobResponse>>;
    fn find_text_references_at_revision(
        &self,
        repo_id: &str,
        symbol: &str,
        revision: &str,
    ) -> Result<Option<Vec<ReferenceMatch>>>;
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GlobResponse {
    pub repo_id: String,
    pub pattern: String,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceMatch {
    pub path: String,
    pub line_number: usize,
    pub line: String,
}

#[derive(Clone, Default)]
pub struct LocalBrowseStore {
    repo_roots: HashMap<String, PathBuf>,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct BrowseTreeStoreAdapter {
    browse: DynBrowseStore,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct BrowseBlobStoreAdapter {
    browse: DynBrowseStore,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct BrowseGlobStoreAdapter {
    browse: DynBrowseStore,
}

#[allow(dead_code)]
impl BrowseTreeStoreAdapter {
    pub fn new(browse: DynBrowseStore) -> Self {
        Self { browse }
    }
}

#[allow(dead_code)]
impl BrowseBlobStoreAdapter {
    pub fn new(browse: DynBrowseStore) -> Self {
        Self { browse }
    }
}

#[allow(dead_code)]
impl BrowseGlobStoreAdapter {
    pub fn new(browse: DynBrowseStore) -> Self {
        Self { browse }
    }
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

    fn repo_root(&self, repo_id: &str) -> Option<&PathBuf> {
        self.repo_roots.get(repo_id)
    }

    fn collect_glob_matches(
        &self,
        root: &Path,
        current_path: &Path,
        matcher: &globset::GlobMatcher,
        matches: &mut Vec<String>,
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
                self.collect_glob_matches(root, &path, matcher, matches)?;
                continue;
            }

            let relative_path = path
                .strip_prefix(root)
                .with_context(|| format!("failed to strip prefix for {}", path.display()))?
                .to_string_lossy()
                .replace('\\', "/");

            if matcher.is_match(&relative_path) {
                matches.push(relative_path);
            }
        }

        Ok(())
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
        self.get_blob_at_revision(repo_id, path, None)
    }

    fn glob_paths(&self, repo_id: &str, pattern: &str) -> Result<Option<GlobResponse>> {
        let Some(repo_root) = self.repo_root(repo_id) else {
            return Ok(None);
        };

        let matcher = Glob::new(pattern)
            .with_context(|| format!("invalid glob pattern: {pattern}"))?
            .compile_matcher();
        let mut paths = Vec::new();
        self.collect_glob_matches(repo_root, repo_root, &matcher, &mut paths)?;
        paths.sort();

        Ok(Some(GlobResponse {
            repo_id: repo_id.to_string(),
            pattern: pattern.to_string(),
            paths,
        }))
    }

    fn get_blob_at_revision(
        &self,
        repo_id: &str,
        path: &str,
        revision: Option<&str>,
    ) -> Result<Option<BlobResponse>> {
        let Some(full_path) = self.resolve_path(repo_id, path)? else {
            return Ok(None);
        };

        let (content, size_bytes) = match revision {
            Some(revision) => {
                let Some(repo_root) = self.repo_roots.get(repo_id) else {
                    return Ok(None);
                };

                let Some(content) = run_git_show_blob(repo_root, revision, path)? else {
                    return Ok(None);
                };
                let size_bytes = content.len() as u64;
                (content, size_bytes)
            }
            None => {
                if !full_path.exists() || !full_path.is_file() {
                    return Ok(None);
                }

                let content = fs::read_to_string(&full_path)
                    .with_context(|| format!("failed to read file {}", full_path.display()))?;
                let size_bytes = fs::metadata(&full_path)
                    .with_context(|| {
                        format!("failed to read metadata for {}", full_path.display())
                    })?
                    .len();
                (content, size_bytes)
            }
        };

        Ok(Some(BlobResponse {
            repo_id: repo_id.to_string(),
            path: path.to_string(),
            content,
            size_bytes,
        }))
    }

    fn find_text_references_at_revision(
        &self,
        repo_id: &str,
        symbol: &str,
        revision: &str,
    ) -> Result<Option<Vec<ReferenceMatch>>> {
        let Some(repo_root) = self.repo_root(repo_id) else {
            return Ok(None);
        };

        let Some(paths) = run_git_list_files_at_revision(repo_root, revision)? else {
            return Ok(None);
        };

        let mut matches = Vec::new();
        for path in paths.into_iter().filter(|path| path.ends_with(".rs")) {
            let Some(content) = run_git_show_blob(repo_root, revision, &path)? else {
                continue;
            };

            for (index, line) in content.lines().enumerate() {
                if line.contains(symbol) {
                    matches.push(ReferenceMatch {
                        path: path.clone(),
                        line_number: index + 1,
                        line: line.to_string(),
                    });
                }
            }
        }

        Ok(Some(matches))
    }
}

#[async_trait]
impl TreeStore for BrowseTreeStoreAdapter {
    async fn get_tree(&self, repo_id: &str, path: &str) -> Result<Option<RepositoryTree>> {
        Ok(self
            .browse
            .get_tree(repo_id, path)?
            .map(|tree| RepositoryTree {
                repo_id: tree.repo_id,
                path: tree.path,
                entries: tree
                    .entries
                    .into_iter()
                    .map(|entry| RepositoryTreeEntry {
                        name: entry.name,
                        path: entry.path,
                        kind: match entry.kind {
                            EntryKind::File => RepositoryTreeEntryKind::File,
                            EntryKind::Dir => RepositoryTreeEntryKind::Dir,
                        },
                    })
                    .collect(),
            }))
    }
}

#[async_trait]
impl BlobStore for BrowseBlobStoreAdapter {
    async fn get_blob(&self, repo_id: &str, path: &str) -> Result<Option<RepositoryBlob>> {
        Ok(self
            .browse
            .get_blob(repo_id, path)?
            .map(|blob| RepositoryBlob {
                repo_id: blob.repo_id,
                path: blob.path,
                content: blob.content,
                size_bytes: blob.size_bytes,
            }))
    }
}

#[async_trait]
impl GlobStore for BrowseGlobStoreAdapter {
    async fn glob_paths(&self, repo_id: &str, pattern: &str) -> Result<Option<RepositoryGlob>> {
        Ok(self
            .browse
            .glob_paths(repo_id, pattern)?
            .map(|glob| RepositoryGlob {
                repo_id: glob.repo_id,
                pattern: glob.pattern,
                paths: glob.paths,
            }))
    }
}

fn run_git_show_blob(repo_root: &PathBuf, revision: &str, path: &str) -> Result<Option<String>> {
    let object = format!("{revision}:{path}");
    let output = Command::new("git")
        .args(["show", &object])
        .current_dir(repo_root)
        .output()
        .with_context(|| format!("failed to run git show in {}", repo_root.display()))?;

    if output.status.success() {
        return Ok(Some(
            String::from_utf8(output.stdout).context("git output was not utf-8")?,
        ));
    }

    if git_object_not_found_output(&output) {
        return Ok(None);
    }

    Err(git_command_error(
        repo_root,
        &["show", "<revision>:<path>"],
        &output,
    ))
}

fn run_git_list_files_at_revision(
    repo_root: &PathBuf,
    revision: &str,
) -> Result<Option<Vec<String>>> {
    let output = Command::new("git")
        .args(["ls-tree", "-r", "--name-only", revision])
        .current_dir(repo_root)
        .output()
        .with_context(|| format!("failed to run git ls-tree in {}", repo_root.display()))?;

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).context("git output was not utf-8")?;
        let files = stdout
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        return Ok(Some(files));
    }

    if git_object_not_found_output(&output) {
        return Ok(None);
    }

    Err(git_command_error(
        repo_root,
        &["ls-tree", "-r", "--name-only", "<revision>"],
        &output,
    ))
}

fn git_object_not_found_output(output: &Output) -> bool {
    let stderr = String::from_utf8_lossy(&output.stderr);
    stderr.contains("exists on disk, but not in")
        || stderr.contains("pathspec")
        || stderr.contains("unknown revision")
        || stderr.contains("bad object")
        || stderr.contains("fatal: invalid object name")
        || stderr.contains("ambiguous argument")
}

fn git_command_error(repo_root: &PathBuf, args: &[&str], output: &Output) -> anyhow::Error {
    anyhow::anyhow!(
        "git {:?} failed in {}: {}",
        args,
        repo_root.display(),
        String::from_utf8_lossy(&output.stderr).trim()
    )
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
    use sourcebot_core::{BlobStore, GlobStore, RepositoryTreeEntryKind, TreeStore};
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[cfg(unix)]
    use std::os::unix::fs::symlink;

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
        fs::create_dir_all(root.join("target")).unwrap();
        fs::write(root.join("README.md"), "hello world\n").unwrap();
        fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
        fs::write(
            root.join("target").join("generated.rs"),
            "pub fn generated() {}\n",
        )
        .unwrap();

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
    fn local_browse_store_globs_matching_paths() {
        let (store, root) = create_test_store();

        let glob = store.glob_paths("repo_test", "src/*.rs").unwrap().unwrap();

        assert_eq!(glob.repo_id, "repo_test");
        assert_eq!(glob.pattern, "src/*.rs");
        assert_eq!(glob.paths, vec!["src/main.rs"]);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_browse_store_globs_paths_visible_in_tree_entries() {
        let (store, root) = create_test_store();

        let tree = store.get_tree("repo_test", "").unwrap().unwrap();
        assert!(tree.entries.iter().any(|entry| {
            entry.name == "target" && entry.path == "target" && entry.kind == EntryKind::Dir
        }));

        let glob = store
            .glob_paths("repo_test", "target/*.rs")
            .unwrap()
            .unwrap();
        assert_eq!(glob.paths, vec!["target/generated.rs"]);

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn local_browse_store_globs_symlink_paths_visible_in_tree_entries() {
        let (store, root) = create_test_store();
        symlink(
            root.join("README.md"),
            root.join("src").join("readme-link.rs"),
        )
        .unwrap();

        let tree = store.get_tree("repo_test", "src").unwrap().unwrap();
        assert!(tree.entries.iter().any(|entry| {
            entry.name == "readme-link.rs"
                && entry.path == "src/readme-link.rs"
                && entry.kind == EntryKind::File
        }));

        let glob = store.glob_paths("repo_test", "src/*.rs").unwrap().unwrap();
        assert!(glob.paths.contains(&"src/readme-link.rs".to_string()));

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

    #[tokio::test]
    async fn browse_tree_store_adapter_converts_browse_tree_for_core_retrieval() {
        let (store, root) = create_test_store();
        let adapter = BrowseTreeStoreAdapter::new(Arc::new(store));

        let tree = TreeStore::get_tree(&adapter, "repo_test", "src")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(tree.repo_id, "repo_test");
        assert_eq!(tree.path, "src");
        assert_eq!(tree.entries.len(), 1);
        assert_eq!(tree.entries[0].name, "main.rs");
        assert_eq!(tree.entries[0].path, "src/main.rs");
        assert_eq!(tree.entries[0].kind, RepositoryTreeEntryKind::File);

        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn browse_glob_store_adapter_converts_browse_glob_for_core_retrieval() {
        let (store, root) = create_test_store();
        let adapter = BrowseGlobStoreAdapter::new(Arc::new(store));

        let glob = GlobStore::glob_paths(&adapter, "repo_test", "src/*.rs")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            glob,
            RepositoryGlob {
                repo_id: "repo_test".into(),
                pattern: "src/*.rs".into(),
                paths: vec!["src/main.rs".into()],
            }
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn browse_blob_store_adapter_converts_browse_blob_for_core_retrieval() {
        let (store, root) = create_test_store();
        let adapter = BrowseBlobStoreAdapter::new(Arc::new(store));

        let blob = BlobStore::get_blob(&adapter, "repo_test", "README.md")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            blob,
            RepositoryBlob {
                repo_id: "repo_test".into(),
                path: "README.md".into(),
                content: "hello world\n".into(),
                size_bytes: 12,
            }
        );

        fs::remove_dir_all(root).unwrap();
    }
}
