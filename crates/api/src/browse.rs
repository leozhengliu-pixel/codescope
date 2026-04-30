use anyhow::{Context, Result};
use async_trait::async_trait;
use globset::Glob;
use serde::Serialize;
use sourcebot_core::{
    BlobStore, GlobStore, GrepStore, RepositoryBlob, RepositoryGlob, RepositoryGrep,
    RepositoryGrepMatch, RepositoryTree, RepositoryTreeEntry, RepositoryTreeEntryKind, TreeStore,
};
use std::{
    collections::HashMap,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

pub type DynBrowseStore = Arc<dyn BrowseStore>;

const SOURCEBOT_REWRITE_REPO_ID: &str = "repo_sourcebot_rewrite";
const SOURCEBOT_REWRITE_ROOT: &str = "/opt/data/projects/sourcebot-rewrite";
const SKIPPED_DIR_NAMES: &[&str] = &[".git", "target", "node_modules", "dist"];
const BLOB_CONTENT_MAX_BYTES: usize = 8 * 1024 * 1024;

pub trait BrowseStore: Send + Sync {
    fn get_tree(&self, repo_id: &str, path: &str) -> Result<Option<TreeResponse>>;
    fn get_tree_at_revision(
        &self,
        repo_id: &str,
        path: &str,
        revision: Option<&str>,
    ) -> Result<Option<TreeResponse>>;
    #[allow(dead_code)]
    fn get_blob(&self, repo_id: &str, path: &str) -> Result<Option<BlobResponse>>;
    #[allow(dead_code)]
    fn glob_paths(&self, repo_id: &str, pattern: &str) -> Result<Option<GlobResponse>>;
    #[allow(dead_code)]
    fn grep(&self, repo_id: &str, query: &str) -> Result<Option<GrepResponse>>;
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
    pub is_binary: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GlobResponse {
    pub repo_id: String,
    pub pattern: String,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GrepMatchResponse {
    pub path: String,
    pub line_number: usize,
    pub line: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GrepResponse {
    pub repo_id: String,
    pub query: String,
    pub matches: Vec<GrepMatchResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceMatch {
    pub path: String,
    pub line_number: usize,
    pub line: String,
}

struct DecodedBlobContent {
    content: String,
    is_binary: bool,
}

fn decode_blob_content(bytes: Vec<u8>) -> DecodedBlobContent {
    if bytes.contains(&0) {
        return DecodedBlobContent {
            content: String::new(),
            is_binary: true,
        };
    }

    match String::from_utf8(bytes) {
        Ok(content) => DecodedBlobContent {
            content,
            is_binary: false,
        },
        Err(_) => DecodedBlobContent {
            content: String::new(),
            is_binary: true,
        },
    }
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

#[derive(Clone)]
#[allow(dead_code)]
pub struct BrowseGrepStoreAdapter {
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

#[allow(dead_code)]
impl BrowseGrepStoreAdapter {
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

    fn path_is_within_repo_root(root: &Path, path: &Path) -> bool {
        let Ok(canonical_root) = fs::canonicalize(root) else {
            return false;
        };

        let Ok(canonical_path) = fs::canonicalize(path) else {
            return false;
        };

        canonical_path.starts_with(canonical_root)
    }

    fn should_skip_directory(path: &Path) -> bool {
        path.file_name()
            .is_some_and(|name| SKIPPED_DIR_NAMES.iter().any(|skipped| name == *skipped))
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
                if Self::should_skip_directory(&path) {
                    continue;
                }
                self.collect_glob_matches(root, &path, matcher, matches)?;
                continue;
            }

            let relative_path = path
                .strip_prefix(root)
                .with_context(|| format!("failed to strip prefix for {}", path.display()))?
                .to_string_lossy()
                .replace('\\', "/");

            if !Self::path_is_within_repo_root(root, &path) {
                continue;
            }

            if matcher.is_match(&relative_path) {
                matches.push(relative_path);
            }
        }

        Ok(())
    }

    fn collect_grep_matches(
        &self,
        root: &Path,
        current_path: &Path,
        query: &str,
        matches: &mut Vec<GrepMatchResponse>,
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
                if Self::should_skip_directory(&path) {
                    continue;
                }
                self.collect_grep_matches(root, &path, query, matches)?;
                continue;
            }

            let relative_path = path
                .strip_prefix(root)
                .with_context(|| format!("failed to strip prefix for {}", path.display()))?
                .to_string_lossy()
                .replace('\\', "/");

            if !Self::path_is_within_repo_root(root, &path) {
                continue;
            }

            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };

            for (index, line) in content.lines().enumerate() {
                if line.contains(query) {
                    matches.push(GrepMatchResponse {
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

impl BrowseStore for LocalBrowseStore {
    fn get_tree(&self, repo_id: &str, path: &str) -> Result<Option<TreeResponse>> {
        let Some(full_path) = self.resolve_path(repo_id, path)? else {
            return Ok(None);
        };

        let Some(repo_root) = self.repo_root(repo_id) else {
            return Ok(None);
        };

        if !full_path.exists()
            || !full_path.is_dir()
            || !Self::path_is_within_repo_root(repo_root, &full_path)
        {
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

    fn get_tree_at_revision(
        &self,
        repo_id: &str,
        path: &str,
        revision: Option<&str>,
    ) -> Result<Option<TreeResponse>> {
        let Some(revision) = revision else {
            return self.get_tree(repo_id, path);
        };

        let Some(repo_root) = self.repo_root(repo_id) else {
            return Ok(None);
        };

        let Some(revision) = resolve_single_commit(repo_root, revision)? else {
            return Ok(None);
        };

        let Some(entries) = run_git_list_tree_entries_at_revision(repo_root, &revision, path)?
        else {
            return Ok(None);
        };

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

    fn grep(&self, repo_id: &str, query: &str) -> Result<Option<GrepResponse>> {
        let Some(repo_root) = self.repo_root(repo_id) else {
            return Ok(None);
        };

        let mut matches = Vec::new();
        self.collect_grep_matches(repo_root, repo_root, query, &mut matches)?;
        matches.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then(left.line_number.cmp(&right.line_number))
        });

        Ok(Some(GrepResponse {
            repo_id: repo_id.to_string(),
            query: query.to_string(),
            matches,
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

        let (content, size_bytes, is_binary) = match revision {
            Some(revision) => {
                let Some(repo_root) = self.repo_roots.get(repo_id) else {
                    return Ok(None);
                };
                let Some(revision) = resolve_single_commit(repo_root, revision)? else {
                    return Ok(None);
                };

                let normalized_path = normalize_relative_path(path)?;
                let Some(bytes) = run_git_show_blob(repo_root, &revision, &normalized_path)? else {
                    return Ok(None);
                };
                let size_bytes = bytes.len() as u64;
                let blob_content = decode_blob_content(bytes);
                (blob_content.content, size_bytes, blob_content.is_binary)
            }
            None => {
                let Some(repo_root) = self.repo_roots.get(repo_id) else {
                    return Ok(None);
                };
                if !full_path.exists()
                    || !full_path.is_file()
                    || !Self::path_is_within_repo_root(repo_root, &full_path)
                {
                    return Ok(None);
                }

                let size_bytes = fs::metadata(&full_path)
                    .with_context(|| {
                        format!("failed to read metadata for {}", full_path.display())
                    })?
                    .len();
                if size_bytes > BLOB_CONTENT_MAX_BYTES as u64 {
                    anyhow::bail!(
                        "blob exceeds {} byte read limit: {}",
                        BLOB_CONTENT_MAX_BYTES,
                        path
                    );
                }
                let bytes = fs::read(&full_path)
                    .with_context(|| format!("failed to read file {}", full_path.display()))?;
                let blob_content = decode_blob_content(bytes);
                (blob_content.content, size_bytes, blob_content.is_binary)
            }
        };

        Ok(Some(BlobResponse {
            repo_id: repo_id.to_string(),
            path: path.to_string(),
            content,
            size_bytes,
            is_binary,
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
        let Some(revision) = resolve_single_commit(repo_root, revision)? else {
            return Ok(None);
        };
        if symbol.trim().is_empty() {
            return Ok(Some(Vec::new()));
        }

        let Some(paths) = run_git_list_files_at_revision(repo_root, &revision)? else {
            return Ok(None);
        };

        let mut matches = Vec::new();
        for path in paths.into_iter().filter(|path| {
            supports_text_reference_scan(path) && !path_contains_skipped_directory(path)
        }) {
            let normalized_path = normalize_relative_path(&path)?;
            let Some(bytes) = run_git_show_blob(repo_root, &revision, &normalized_path)? else {
                continue;
            };
            let blob_content = decode_blob_content(bytes);
            if blob_content.is_binary {
                continue;
            }

            for (index, line) in blob_content.content.lines().enumerate() {
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
                is_binary: blob.is_binary,
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

#[async_trait]
impl GrepStore for BrowseGrepStoreAdapter {
    async fn grep(&self, repo_id: &str, query: &str) -> Result<Option<RepositoryGrep>> {
        Ok(self
            .browse
            .grep(repo_id, query)?
            .map(|grep| RepositoryGrep {
                repo_id: grep.repo_id,
                query: grep.query,
                matches: grep
                    .matches
                    .into_iter()
                    .map(|entry| RepositoryGrepMatch {
                        path: entry.path,
                        line_number: entry.line_number,
                        line: entry.line,
                    })
                    .collect(),
            }))
    }
}

fn supports_text_reference_scan(path: &str) -> bool {
    matches!(
        Path::new(path)
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("rs" | "ts" | "tsx" | "js" | "jsx" | "mts" | "cts" | "mjs" | "cjs" | "py" | "go",)
    )
}

fn path_contains_skipped_directory(path: &str) -> bool {
    Path::new(path).components().any(|component| {
        matches!(
            component,
            Component::Normal(name)
                if SKIPPED_DIR_NAMES.iter().any(|skipped| name == *skipped)
        )
    })
}

fn resolve_single_commit(repo_root: &PathBuf, revision: &str) -> Result<Option<String>> {
    if !is_safe_revision_selector(revision) {
        return Ok(None);
    }

    let verify_arg = format!("{revision}^{{commit}}");
    let output = bounded_git_output(
        repo_root,
        &["rev-parse", "--verify", "--end-of-options", &verify_arg],
    )?;

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).context("git output was not utf-8")?;
        let resolved = stdout.trim();
        return if resolved.is_empty() {
            Ok(None)
        } else {
            Ok(Some(resolved.to_string()))
        };
    }

    if git_object_not_found_output(&output) {
        return Ok(None);
    }

    Err(git_command_error(
        repo_root,
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            "<revision>^{commit}",
        ],
        &output,
    ))
}

fn is_safe_revision_selector(revision: &str) -> bool {
    !revision.is_empty()
        && !revision.starts_with('-')
        && !revision.contains("..")
        && !revision.contains("@{")
        && !revision.chars().any(char::is_control)
}

fn run_git_show_blob(
    repo_root: &PathBuf,
    revision: &str,
    normalized_path: &Path,
) -> Result<Option<Vec<u8>>> {
    if normalized_path.as_os_str().is_empty() {
        return Ok(None);
    }

    let path = normalized_path.to_string_lossy().replace('\\', "/");
    let Some(mode) = run_git_object_mode(repo_root, revision, &path)? else {
        return Ok(None);
    };
    if mode == "120000" {
        return Ok(None);
    }

    let object = format!("{revision}:{path}");
    let Some(object_type) = run_git_object_type(repo_root, &object)? else {
        return Ok(None);
    };
    if object_type != "blob" {
        return Ok(None);
    }

    let output = bounded_git_output(repo_root, &["show", &object])?;

    if output.status.success() {
        return Ok(Some(output.stdout));
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

fn run_git_object_mode(repo_root: &PathBuf, revision: &str, path: &str) -> Result<Option<String>> {
    let output = bounded_git_output(repo_root, &["ls-tree", "-z", revision, "--", path])?;

    if output.status.success() {
        if output.stdout.is_empty() {
            return Ok(None);
        }
        let stdout = String::from_utf8(output.stdout).context("git output was not utf-8")?;
        let first_entry = stdout
            .split('\0')
            .find(|entry| !entry.is_empty())
            .ok_or_else(|| anyhow::anyhow!("unexpected empty git ls-tree output"))?;
        let Some((metadata, _path)) = first_entry.split_once('\t') else {
            anyhow::bail!("unexpected git ls-tree output: {first_entry}");
        };
        let mode = metadata
            .split_whitespace()
            .next()
            .ok_or_else(|| anyhow::anyhow!("unexpected git ls-tree metadata: {metadata}"))?;
        return Ok(Some(mode.to_string()));
    }

    if git_object_not_found_output(&output) {
        return Ok(None);
    }

    Err(git_command_error(
        repo_root,
        &["ls-tree", "-z", "<revision>", "--", "<path>"],
        &output,
    ))
}

fn run_git_object_type(repo_root: &PathBuf, object: &str) -> Result<Option<String>> {
    let output = bounded_git_output(repo_root, &["cat-file", "-t", object])?;

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).context("git output was not utf-8")?;
        return Ok(Some(stdout.trim().to_string()));
    }

    if git_object_not_found_output(&output) {
        return Ok(None);
    }

    Err(git_command_error(
        repo_root,
        &["cat-file", "-t", "<revision>:<path>"],
        &output,
    ))
}

fn run_git_list_tree_entries_at_revision(
    repo_root: &PathBuf,
    revision: &str,
    path: &str,
) -> Result<Option<Vec<TreeEntry>>> {
    let normalized_path = normalize_relative_path(path)?;
    let treeish = if normalized_path.as_os_str().is_empty() {
        revision.to_string()
    } else {
        format!(
            "{revision}:{}",
            normalized_path.to_string_lossy().replace('\\', "/")
        )
    };
    let output = bounded_git_output(repo_root, &["ls-tree", "-z", &treeish])?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut entries = stdout
            .split('\0')
            .filter(|record| !record.is_empty())
            .map(|record| parse_git_tree_entry(&normalized_path, record))
            .collect::<Result<Vec<_>>>()?;
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        return Ok(Some(entries));
    }

    if git_object_not_found_output(&output) {
        return Ok(None);
    }

    Err(git_command_error(
        repo_root,
        &["ls-tree", "<revision>[:<path>]"],
        &output,
    ))
}

fn parse_git_tree_entry(parent_path: &Path, line: &str) -> Result<TreeEntry> {
    let Some((metadata, name)) = line.split_once('\t') else {
        anyhow::bail!("unexpected git ls-tree output: {line}");
    };
    let kind = if metadata.split_whitespace().nth(1) == Some("tree") {
        EntryKind::Dir
    } else {
        EntryKind::File
    };
    let path = join_relative_path(&parent_path.to_string_lossy(), name);

    Ok(TreeEntry {
        name: name.to_string(),
        path,
        kind,
    })
}

fn run_git_list_files_at_revision(
    repo_root: &PathBuf,
    revision: &str,
) -> Result<Option<Vec<String>>> {
    let output = bounded_git_output(repo_root, &["ls-tree", "-rz", "--name-only", revision])?;

    if output.status.success() {
        let files = output
            .stdout
            .split(|byte| *byte == b'\0')
            .filter(|path| !path.is_empty())
            .filter_map(|path| std::str::from_utf8(path).ok().map(ToOwned::to_owned))
            .collect();
        return Ok(Some(files));
    }

    if git_object_not_found_output(&output) {
        return Ok(None);
    }

    Err(git_command_error(
        repo_root,
        &["ls-tree", "-rz", "--name-only", "<revision>"],
        &output,
    ))
}

fn bounded_git_output(repo_root: &PathBuf, args: &[&str]) -> Result<Output> {
    const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
    const GIT_STDERR_CAPTURE_MAX_BYTES: usize = 64 * 1024;

    let mut child = Command::new("git")
        .args(args)
        .env("GIT_LITERAL_PATHSPECS", "1")
        .current_dir(repo_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to start git in {}", repo_root.display()))?;
    let stdout = child.stdout.take().ok_or_else(|| {
        anyhow::anyhow!("failed to capture git stdout in {}", repo_root.display())
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        anyhow::anyhow!("failed to capture git stderr in {}", repo_root.display())
    })?;
    let stdout_reader = thread::spawn(move || read_stream_bounded(stdout, BLOB_CONTENT_MAX_BYTES));
    let stderr_reader =
        thread::spawn(move || read_stream_bounded(stderr, GIT_STDERR_CAPTURE_MAX_BYTES));

    let started_at = Instant::now();
    loop {
        if child
            .try_wait()
            .with_context(|| format!("failed to poll git in {}", repo_root.display()))?
            .is_some()
        {
            let status = child.wait().with_context(|| {
                format!("failed to collect git status in {}", repo_root.display())
            })?;
            let stdout = stdout_reader
                .join()
                .map_err(|_| anyhow::anyhow!("failed to join git stdout reader"))??;
            let stderr = stderr_reader
                .join()
                .map_err(|_| anyhow::anyhow!("failed to join git stderr reader"))??;
            return Ok(Output {
                status,
                stdout,
                stderr,
            });
        }
        if started_at.elapsed() >= GIT_COMMAND_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(anyhow::anyhow!(
                "git command timed out after {:?} in {}: git {}",
                GIT_COMMAND_TIMEOUT,
                repo_root.display(),
                args.join(" ")
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn read_stream_bounded(mut stream: impl Read, max_bytes: usize) -> Result<Vec<u8>> {
    let mut captured = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut exceeded_limit = false;
    loop {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            if exceeded_limit {
                return Err(anyhow::anyhow!(
                    "git output exceeded {max_bytes} byte capture limit"
                ));
            }
            return Ok(captured);
        }

        let remaining = max_bytes.saturating_sub(captured.len());
        if read > remaining {
            if remaining > 0 {
                captured.extend_from_slice(&buffer[..remaining]);
            }
            exceeded_limit = true;
            continue;
        }
        if !exceeded_limit {
            captured.extend_from_slice(&buffer[..read]);
        }
    }
}

fn git_object_not_found_output(output: &Output) -> bool {
    let stderr = String::from_utf8_lossy(&output.stderr);
    stderr.contains("exists on disk, but not in")
        || stderr.contains("does not exist in")
        || stderr.contains("pathspec")
        || stderr.contains("unknown revision")
        || stderr.contains("bad object")
        || stderr.contains("fatal: invalid object name")
        || stderr.contains("ambiguous argument")
        || stderr.contains("not a tree object")
        || stderr.contains("expected commit type")
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
    if relative_path.contains('\0') {
        anyhow::bail!("invalid relative path: {relative_path:?}");
    }

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
    mod repo_tree_fixture {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../test_support/repo_tree_fixture.rs"
        ));
    }
    use sourcebot_core::{
        BlobStore, GlobStore, GrepStore, RepositoryBlob, RepositoryGlob, RepositoryGrep,
        RepositoryGrepMatch, RepositoryTreeEntryKind, TreeStore,
    };

    fn create_test_store() -> (LocalBrowseStore, PathBuf) {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse",
            "hello world\n",
            "fn main() {}\n",
            "target/generated.rs",
        );
        let root = fixture.root;

        let store = LocalBrowseStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));
        (store, root)
    }

    fn create_test_store_with_common_ignored_dirs() -> (LocalBrowseStore, PathBuf) {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-ignored-dirs",
            "hello world\n",
            "fn main() { /* shared_scale_marker */ }\n",
            "target/generated.rs",
        );
        let root = fixture.root;

        fs::write(
            root.join("target").join("ignored.rs"),
            "pub fn ignored_target() { /* shared_scale_marker */ }\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("node_modules").join("pkg")).unwrap();
        fs::write(
            root.join("node_modules").join("pkg").join("index.rs"),
            "pub fn ignored_node_modules() { /* shared_scale_marker */ }\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("dist")).unwrap();
        fs::write(
            root.join("dist").join("bundle.rs"),
            "pub fn ignored_dist() { /* shared_scale_marker */ }\n",
        )
        .unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(
            root.join(".git").join("ignored.rs"),
            "pub fn ignored_git() { /* shared_scale_marker */ }\n",
        )
        .unwrap();

        let store = LocalBrowseStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));
        (store, root)
    }

    fn git_in(repo_root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo_root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn initialize_git_repo(repo_root: &Path) {
        git_in(repo_root, &["init"]);
        git_in(repo_root, &["config", "user.name", "Hermes Test"]);
        git_in(
            repo_root,
            &["config", "user.email", "hermes-test@example.com"],
        );
        git_in(repo_root, &["add", "-A"]);
        git_in(repo_root, &["commit", "-m", "base"]);
    }

    fn git_stdout_trimmed(repo_root: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo_root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    }

    #[test]
    fn shared_repo_tree_fixture_exposes_browse_common_layout() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-common-layout",
            "hello world\n",
            "fn main() {}\n",
            "target/generated.rs",
        );

        repo_tree_fixture::assert_common_layout(&fixture.root, "target/generated.rs");

        fs::remove_dir_all(fixture.root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn shared_repo_tree_fixture_can_add_browse_symlink_variants() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-symlink-variants",
            "hello world\n",
            "fn main() {}\n",
            "target/generated.rs",
        );

        let outside_path = fixture.add_browse_symlink_variants();

        assert!(fixture.root.join("src").join("readme-link.rs").exists());
        let outside_link = fixture.root.join("src").join("outside-secret.rs");
        assert!(outside_link.exists());
        assert_eq!(
            fs::read_to_string(&outside_link).unwrap(),
            "secret generated token\n"
        );

        fs::remove_file(outside_path).unwrap();
        fs::remove_dir_all(fixture.root).unwrap();
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
    fn local_browse_store_lists_tree_entries_from_requested_revision() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-revision-tree",
            "hello world\n",
            "fn main() {}\n",
            "target/generated.rs",
        );
        let root = fixture.root;
        initialize_git_repo(&root);
        git_in(&root, &["checkout", "-b", "feature/revision-tree"]);
        fs::remove_file(root.join("README.md")).unwrap();
        fs::write(root.join("FEATURE.md"), "feature branch\n").unwrap();
        git_in(&root, &["add", "-A"]);
        git_in(&root, &["commit", "-m", "feature"]);

        let store = LocalBrowseStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));
        let tree = store
            .get_tree_at_revision("repo_test", "", Some("HEAD"))
            .unwrap()
            .unwrap();

        assert!(tree.entries.iter().any(|entry| entry.path == "FEATURE.md"));
        assert!(!tree.entries.iter().any(|entry| entry.path == "README.md"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_browse_store_keeps_repo_relative_paths_for_nested_revision_tree_entries() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-revision-nested-tree",
            "hello world\n",
            "fn main() {}\n",
            "target/generated.rs",
        );
        let root = fixture.root;
        initialize_git_repo(&root);
        git_in(&root, &["checkout", "-b", "feature/nested-revision-tree"]);
        fs::write(
            root.join("src").join("revision.rs"),
            "pub fn revision() {}\n",
        )
        .unwrap();
        git_in(&root, &["add", "-A"]);
        git_in(&root, &["commit", "-m", "nested feature"]);

        let store = LocalBrowseStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));
        let tree = store
            .get_tree_at_revision("repo_test", "src", Some("HEAD"))
            .unwrap()
            .unwrap();

        assert!(tree
            .entries
            .iter()
            .any(|entry| entry.path == "src/revision.rs"));
        assert!(tree
            .entries
            .iter()
            .all(|entry| entry.path.starts_with("src/")));

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn local_browse_store_preserves_newline_paths_in_revision_tree_entries() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-revision-newline-tree",
            "hello world\n",
            "fn main() {}\n",
            "target/generated.rs",
        );
        let root = fixture.root;
        let newline_path = root.join("src").join("generated\nview.rs");
        fs::write(&newline_path, "pub fn generated_view() {}\n").unwrap();
        initialize_git_repo(&root);

        let store = LocalBrowseStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));
        let tree = store
            .get_tree_at_revision("repo_test", "src", Some("HEAD"))
            .unwrap()
            .unwrap();

        assert!(tree.entries.iter().any(|entry| {
            entry.name == "generated\nview.rs"
                && entry.path == "src/generated\nview.rs"
                && entry.kind == EntryKind::File
        }));

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn local_browse_store_tolerates_non_utf8_paths_in_revision_tree_entries() {
        use std::os::unix::ffi::OsStringExt;

        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-revision-non-utf8-tree",
            "hello world\n",
            "fn main() {}\n",
            "target/generated.rs",
        );
        let root = fixture.root;
        let non_utf8_name = std::ffi::OsString::from_vec(b"invalid-\xff.rs".to_vec());
        fs::write(
            root.join("src").join(non_utf8_name),
            "pub fn generated_view() {}\n",
        )
        .unwrap();
        initialize_git_repo(&root);

        let store = LocalBrowseStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));
        let tree = store
            .get_tree_at_revision("repo_test", "src", Some("HEAD"))
            .unwrap()
            .unwrap();

        assert!(tree.entries.iter().any(|entry| {
            entry.name == "invalid-�.rs"
                && entry.path == "src/invalid-�.rs"
                && entry.kind == EntryKind::File
        }));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_browse_store_rejects_non_commit_revision_objects() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-revision-non-commit",
            "hello world\n",
            "fn main() {}\n",
            "target/generated.rs",
        );
        let root = fixture.root;
        initialize_git_repo(&root);
        let tree_id = git_stdout_trimmed(&root, &["rev-parse", "HEAD^{tree}"]);
        let blob_id = git_stdout_trimmed(&root, &["rev-parse", "HEAD:README.md"]);

        let store = LocalBrowseStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));

        for non_commit_revision in [&tree_id, &blob_id] {
            assert!(store
                .get_tree_at_revision("repo_test", "", Some(non_commit_revision))
                .unwrap()
                .is_none());
            assert!(store
                .get_blob_at_revision("repo_test", "README.md", Some(non_commit_revision))
                .unwrap()
                .is_none());
            assert!(store
                .find_text_references_at_revision("repo_test", "main", non_commit_revision)
                .unwrap()
                .is_none());
        }

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_browse_store_rejects_empty_explicit_revision_without_head_fallback() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-empty-explicit-revision",
            "hello world\n",
            "pub fn main() { /* empty_revision_symbol */ }\n",
            "target/generated.rs",
        );
        let root = fixture.root;
        initialize_git_repo(&root);

        let store = LocalBrowseStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));

        assert!(store
            .get_tree_at_revision("repo_test", "", Some(""))
            .unwrap()
            .is_none());
        assert!(store
            .get_blob_at_revision("repo_test", "README.md", Some(""))
            .unwrap()
            .is_none());
        assert!(store
            .find_text_references_at_revision("repo_test", "empty_revision_symbol", "")
            .unwrap()
            .is_none());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_browse_store_rejects_unsafe_explicit_revision_selectors() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-unsafe-explicit-revision",
            "hello world\n",
            "pub fn main() { /* unsafe_revision_symbol */ }\n",
            "target/generated.rs",
        );
        let root = fixture.root;
        initialize_git_repo(&root);

        let store = LocalBrowseStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));

        for revision in ["HEAD@{0}", "HEAD..HEAD", "-HEAD"] {
            assert!(
                store
                    .get_tree_at_revision("repo_test", "", Some(revision))
                    .unwrap()
                    .is_none(),
                "tree reads must reject unsafe revision selector {revision:?}"
            );
            assert!(
                store
                    .get_blob_at_revision("repo_test", "README.md", Some(revision))
                    .unwrap()
                    .is_none(),
                "blob reads must reject unsafe revision selector {revision:?}"
            );
            assert!(
                store
                    .find_text_references_at_revision(
                        "repo_test",
                        "unsafe_revision_symbol",
                        revision
                    )
                    .unwrap()
                    .is_none(),
                "code-navigation reads must reject unsafe revision selector {revision:?}"
            );
        }

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
    fn glob_paths_skips_common_build_artifact_directories() {
        let (store, root) = create_test_store_with_common_ignored_dirs();

        let glob = store.glob_paths("repo_test", "**/*.rs").unwrap().unwrap();

        assert!(glob.paths.iter().any(|path| path == "src/main.rs"));
        assert!(glob.paths.iter().all(|path| !path.starts_with("target/")));
        assert!(glob
            .paths
            .iter()
            .all(|path| !path.starts_with("node_modules/")));
        assert!(glob.paths.iter().all(|path| !path.starts_with("dist/")));
        assert!(glob.paths.iter().all(|path| !path.starts_with(".git/")));

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
        assert!(glob.paths.is_empty());

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn local_browse_store_globs_symlink_paths_visible_in_tree_entries() {
        let (store, root) = create_test_store();
        let outside_path = repo_tree_fixture::CanonicalRepoTreeRoot { root: root.clone() }
            .add_browse_symlink_variants();

        let tree = store.get_tree("repo_test", "src").unwrap().unwrap();
        assert!(tree.entries.iter().any(|entry| {
            entry.name == "readme-link.rs"
                && entry.path == "src/readme-link.rs"
                && entry.kind == EntryKind::File
        }));

        let glob = store.glob_paths("repo_test", "src/*.rs").unwrap().unwrap();
        assert!(glob.paths.contains(&"src/readme-link.rs".to_string()));
        let outside_symlink_path = fs::read_dir(root.join("src"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| fs::read_link(path).is_ok_and(|target| !target.starts_with(&root)))
            .unwrap()
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        assert!(
            !glob.paths.contains(&outside_symlink_path),
            "glob results must not expose symlinked files outside the repo root"
        );

        fs::remove_file(outside_path).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_browse_store_reads_blob_contents() {
        let (store, root) = create_test_store();

        let blob = store.get_blob("repo_test", "README.md").unwrap().unwrap();

        assert_eq!(blob.path, "README.md");
        assert_eq!(blob.content, "hello world\n");
        assert_eq!(blob.size_bytes, 12);
        assert!(!blob.is_binary);

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn local_browse_store_rejects_blob_symlinked_outside_repo_root() {
        let (store, root) = create_test_store();
        let outside_path = repo_tree_fixture::CanonicalRepoTreeRoot { root: root.clone() }
            .add_browse_symlink_variants();

        assert!(store
            .get_blob("repo_test", "src/outside-secret.rs")
            .unwrap()
            .is_none());

        fs::remove_file(outside_path).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn local_browse_store_rejects_tree_symlinked_outside_repo_root() {
        let (store, root) = create_test_store();
        let outside_dir = root.with_extension("outside-tree-secret");
        fs::create_dir_all(&outside_dir).unwrap();
        fs::write(outside_dir.join("outside.txt"), "outside tree contents\n").unwrap();
        std::os::unix::fs::symlink(&outside_dir, root.join("src").join("outside-dir")).unwrap();

        assert!(store
            .get_tree("repo_test", "src/outside-dir")
            .unwrap()
            .is_none());

        fs::remove_dir_all(outside_dir).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_browse_store_returns_binary_blob_metadata_without_decoding_contents() {
        let (store, root) = create_test_store();
        fs::write(root.join("assets.bin"), [0xff, 0x00, 0x41, 0x42]).unwrap();

        let blob = store.get_blob("repo_test", "assets.bin").unwrap().unwrap();

        assert_eq!(blob.path, "assets.bin");
        assert_eq!(blob.content, "");
        assert_eq!(blob.size_bytes, 4);
        assert!(blob.is_binary);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_browse_store_bounds_worktree_blob_file_reads() {
        let (store, root) = create_test_store();
        fs::write(
            root.join("huge-worktree.txt"),
            "x".repeat(8 * 1024 * 1024 + 1),
        )
        .unwrap();

        let error = store
            .get_blob("repo_test", "huge-worktree.txt")
            .expect_err("worktree blob file reads must be size-bounded");

        assert!(
            error.to_string().contains("blob exceeds"),
            "unexpected error: {error:#}"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_browse_store_treats_utf8_nul_bytes_as_binary_blob_metadata() {
        let (store, root) = create_test_store();
        fs::write(root.join("nul-delimited.dat"), b"version\0payload\n").unwrap();

        let blob = store
            .get_blob("repo_test", "nul-delimited.dat")
            .unwrap()
            .unwrap();

        assert_eq!(blob.path, "nul-delimited.dat");
        assert_eq!(blob.content, "");
        assert_eq!(blob.size_bytes, 16);
        assert!(blob.is_binary);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_browse_store_returns_binary_blob_metadata_at_revision() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-revision-binary-blob",
            "hello world\n",
            "fn main() {}\n",
            "target/generated.rs",
        );
        let root = fixture.root;
        fs::write(root.join("assets.bin"), [0xff, 0x00, 0x41, 0x42]).unwrap();
        initialize_git_repo(&root);

        let store = LocalBrowseStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));
        let blob = store
            .get_blob_at_revision("repo_test", "assets.bin", Some("HEAD"))
            .unwrap()
            .unwrap();

        assert_eq!(blob.path, "assets.bin");
        assert_eq!(blob.content, "");
        assert_eq!(blob.size_bytes, 4);
        assert!(blob.is_binary);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_browse_store_treats_utf8_nul_bytes_as_binary_blob_metadata_at_revision() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-revision-nul-binary-blob",
            "hello world\n",
            "fn main() {}\n",
            "target/generated.rs",
        );
        let root = fixture.root;
        fs::write(root.join("nul-delimited.dat"), b"version\0payload\n").unwrap();
        initialize_git_repo(&root);

        let store = LocalBrowseStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));
        let blob = store
            .get_blob_at_revision("repo_test", "nul-delimited.dat", Some("HEAD"))
            .unwrap()
            .unwrap();

        assert_eq!(blob.path, "nul-delimited.dat");
        assert_eq!(blob.content, "");
        assert_eq!(blob.size_bytes, 16);
        assert!(blob.is_binary);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_browse_store_bounds_revision_blob_git_output() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-revision-huge-blob",
            "hello world\n",
            "fn main() {}\n",
            "target/generated.rs",
        );
        let root = fixture.root;
        fs::write(root.join("huge.txt"), "x".repeat(8 * 1024 * 1024 + 1)).unwrap();
        initialize_git_repo(&root);

        let store = LocalBrowseStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));
        let error = store
            .get_blob_at_revision("repo_test", "huge.txt", Some("HEAD"))
            .expect_err("revision blob git output must be capture-bounded");

        assert!(
            error.to_string().contains("git output exceeded"),
            "unexpected error: {error:#}"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_browse_store_does_not_return_revision_tree_as_blob() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-revision-tree-as-blob",
            "hello world\n",
            "fn main() {}\n",
            "target/generated.rs",
        );
        let root = fixture.root;
        initialize_git_repo(&root);

        let store = LocalBrowseStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));
        let blob = store
            .get_blob_at_revision("repo_test", "src", Some("HEAD"))
            .unwrap();

        assert_eq!(blob, None);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_browse_store_rejects_nul_bytes_before_revision_blob_git_lookup() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-revision-nul-path-blob",
            "hello world\n",
            "fn main() {}\n",
            "target/generated.rs",
        );
        let root = fixture.root;
        initialize_git_repo(&root);

        let store = LocalBrowseStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));
        let error = store
            .get_blob_at_revision("repo_test", "README.md\0secret", Some("HEAD"))
            .expect_err("NUL bytes must be rejected before invoking git");

        assert!(error.to_string().contains("invalid relative path"));

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn local_browse_store_does_not_return_revision_symlink_as_blob() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-revision-symlink-as-blob",
            "hello world\n",
            "fn main() {}\n",
            "target/generated.rs",
        );
        let root = fixture.root;
        std::os::unix::fs::symlink("README.md", root.join("readme-link.md")).unwrap();
        initialize_git_repo(&root);

        let store = LocalBrowseStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));
        let blob = store
            .get_blob_at_revision("repo_test", "readme-link.md", Some("HEAD"))
            .unwrap();

        assert_eq!(blob, None);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_browse_store_does_not_return_revision_blob_as_tree() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-revision-blob-as-tree",
            "hello world\n",
            "fn main() {}\n",
            "target/generated.rs",
        );
        let root = fixture.root;
        initialize_git_repo(&root);

        let store = LocalBrowseStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));
        let tree = store
            .get_tree_at_revision("repo_test", "README.md", Some("HEAD"))
            .unwrap();

        assert_eq!(tree, None);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_browse_store_returns_no_revision_references_for_blank_symbol() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-revision-blank-reference-symbol",
            "hello world\n",
            "pub fn main() { /* visible_symbol */ }\n",
            "target/generated.rs",
        );
        let root = fixture.root;
        initialize_git_repo(&root);

        let store = LocalBrowseStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));
        let references = store
            .find_text_references_at_revision("repo_test", "", "HEAD")
            .unwrap()
            .unwrap();

        assert!(
            references.is_empty(),
            "blank symbols must not match every scannable source line"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_browse_store_finds_text_references_in_revision_paths_with_newlines() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-revision-newline-path-references",
            "hello world\n",
            "fn main() {}\n",
            "target/generated.rs",
        );
        let root = fixture.root;
        let newline_path = root.join("src").join("generated\nreference.rs");
        fs::write(
            &newline_path,
            "pub fn generated_reference() { /* newline_path_symbol */ }\n",
        )
        .unwrap();
        initialize_git_repo(&root);

        let store = LocalBrowseStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));
        let references = store
            .find_text_references_at_revision("repo_test", "newline_path_symbol", "HEAD")
            .unwrap()
            .unwrap();

        assert_eq!(
            references,
            vec![ReferenceMatch {
                path: "src/generated\nreference.rs".into(),
                line_number: 1,
                line: "pub fn generated_reference() { /* newline_path_symbol */ }".into(),
            }]
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn local_browse_store_skips_non_utf8_paths_during_revision_reference_scan() {
        use std::os::unix::ffi::OsStringExt;

        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-revision-non-utf8-reference-scan",
            "hello world\n",
            "pub fn visible_reference() { /* non_utf8_scan_symbol */ }\n",
            "target/generated.rs",
        );
        let root = fixture.root;
        let non_utf8_name = std::ffi::OsString::from_vec(b"invalid-\xff.rs".to_vec());
        fs::write(
            root.join("src").join(non_utf8_name),
            "pub fn hidden_reference() { /* non_utf8_scan_symbol */ }\n",
        )
        .unwrap();
        initialize_git_repo(&root);

        let store = LocalBrowseStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));
        let references = store
            .find_text_references_at_revision("repo_test", "non_utf8_scan_symbol", "HEAD")
            .unwrap()
            .unwrap();

        assert_eq!(
            references,
            vec![ReferenceMatch {
                path: "src/main.rs".into(),
                line_number: 1,
                line: "pub fn visible_reference() { /* non_utf8_scan_symbol */ }".into(),
            }],
            "non-UTF-8 source paths should fail closed without aborting references for valid paths"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_browse_store_finds_text_references_in_typescript_revision_files() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-revision-typescript-references",
            "hello world\n",
            "fn main() {}\n",
            "target/generated.rs",
        );
        let root = fixture.root;
        fs::create_dir_all(root.join("web").join("src")).unwrap();
        fs::write(
            root.join("web").join("src").join("App.tsx"),
            "export function App() {\n  return <main><AppShell /></main>;\n}\n",
        )
        .unwrap();
        fs::write(
            root.join("web").join("src").join("AppShell.ts"),
            "export const AppShell = () => 'ready';\n",
        )
        .unwrap();
        initialize_git_repo(&root);

        let store = LocalBrowseStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));
        let references = store
            .find_text_references_at_revision("repo_test", "AppShell", "HEAD")
            .unwrap()
            .unwrap();

        assert!(references.iter().any(|reference| {
            reference.path == "web/src/App.tsx"
                && reference.line_number == 2
                && reference.line.contains("AppShell")
        }));
        assert!(references.iter().any(|reference| {
            reference.path == "web/src/AppShell.ts"
                && reference.line_number == 1
                && reference.line.contains("AppShell")
        }));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_browse_store_finds_text_references_in_python_and_go_revision_files() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-revision-python-go-references",
            "hello world\n",
            "fn main() {}\n",
            "target/generated.rs",
        );
        let root = fixture.root;
        fs::create_dir_all(root.join("service")).unwrap();
        fs::write(
            root.join("service").join("handler.py"),
            "def build_index():\n    return navigation_symbol\n",
        )
        .unwrap();
        fs::write(
            root.join("service").join("handler.go"),
            "package service\n\nfunc UseNavigationSymbol() string { return navigation_symbol }\n",
        )
        .unwrap();
        initialize_git_repo(&root);

        let store = LocalBrowseStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));
        let references = store
            .find_text_references_at_revision("repo_test", "navigation_symbol", "HEAD")
            .unwrap()
            .unwrap();

        assert!(references.iter().any(|reference| {
            reference.path == "service/handler.py"
                && reference.line_number == 2
                && reference.line.contains("navigation_symbol")
        }));
        assert!(references.iter().any(|reference| {
            reference.path == "service/handler.go"
                && reference.line_number == 3
                && reference.line.contains("navigation_symbol")
        }));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_browse_store_skips_ignored_directories_for_revision_references() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-revision-reference-ignored-dirs",
            "hello world\n",
            "pub fn uses_shared_symbol() { /* shared_reference_symbol */ }\n",
            "target/generated.rs",
        );
        let root = fixture.root;
        fs::write(
            root.join("target").join("generated.rs"),
            "pub fn generated() { /* shared_reference_symbol */ }\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("node_modules").join("pkg")).unwrap();
        fs::write(
            root.join("node_modules").join("pkg").join("index.ts"),
            "export const generated = 'shared_reference_symbol';\n",
        )
        .unwrap();
        initialize_git_repo(&root);

        let store = LocalBrowseStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));
        let references = store
            .find_text_references_at_revision("repo_test", "shared_reference_symbol", "HEAD")
            .unwrap()
            .unwrap();

        assert!(references
            .iter()
            .any(|reference| reference.path == "src/main.rs"));
        assert!(references
            .iter()
            .all(|reference| !reference.path.starts_with("target/")));
        assert!(references
            .iter()
            .all(|reference| !reference.path.starts_with("node_modules/")));

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

    #[test]
    fn local_browse_store_greps_matching_lines() {
        let (store, root) = create_test_store();

        let grep = store.grep("repo_test", "generated").unwrap().unwrap();

        assert_eq!(grep.repo_id, "repo_test");
        assert_eq!(grep.query, "generated");
        assert!(grep.matches.is_empty());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn grep_skips_common_build_artifact_directories() {
        let (store, root) = create_test_store_with_common_ignored_dirs();

        let grep = store
            .grep("repo_test", "shared_scale_marker")
            .unwrap()
            .unwrap();

        assert!(grep.matches.iter().any(|entry| entry.path == "src/main.rs"));
        assert!(grep
            .matches
            .iter()
            .all(|entry| !entry.path.starts_with("target/")));
        assert!(grep
            .matches
            .iter()
            .all(|entry| !entry.path.starts_with("node_modules/")));
        assert!(grep
            .matches
            .iter()
            .all(|entry| !entry.path.starts_with("dist/")));
        assert!(grep
            .matches
            .iter()
            .all(|entry| !entry.path.starts_with(".git/")));

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn local_browse_store_grep_skips_symlinked_files_outside_repo_root() {
        let (store, root) = create_test_store();
        let outside_path = repo_tree_fixture::CanonicalRepoTreeRoot { root: root.clone() }
            .add_browse_symlink_variants();

        let grep = store.grep("repo_test", "generated").unwrap().unwrap();

        assert!(grep.matches.is_empty());

        fs::remove_file(outside_path).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn browse_glob_and_grep_scale_sanity_skip_ignored_dirs_across_multiple_repositories() {
        let repo_alpha = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-scale-alpha",
            "alpha\n",
            "fn main() { /* shared_scale_marker */ }\n",
            "target/generated.rs",
        )
        .root;
        let repo_beta = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "browse-scale-beta",
            "beta\n",
            "fn main() { /* shared_scale_marker */ }\n",
            "target/generated.rs",
        )
        .root;

        for root in [&repo_alpha, &repo_beta] {
            fs::write(
                root.join("target").join("ignored.rs"),
                "pub fn ignored_target() { /* shared_scale_marker */ }\n",
            )
            .unwrap();
            fs::create_dir_all(root.join("node_modules").join("pkg")).unwrap();
            fs::write(
                root.join("node_modules").join("pkg").join("index.rs"),
                "pub fn ignored_node_modules() { /* shared_scale_marker */ }\n",
            )
            .unwrap();
            fs::create_dir_all(root.join("dist")).unwrap();
            fs::write(
                root.join("dist").join("bundle.rs"),
                "pub fn ignored_dist() { /* shared_scale_marker */ }\n",
            )
            .unwrap();
            fs::create_dir_all(root.join(".git")).unwrap();
            fs::write(
                root.join(".git").join("ignored.rs"),
                "pub fn ignored_git() { /* shared_scale_marker */ }\n",
            )
            .unwrap();
        }

        let browse: DynBrowseStore = Arc::new(LocalBrowseStore::new(HashMap::from([
            ("repo_alpha".to_string(), repo_alpha.clone()),
            ("repo_beta".to_string(), repo_beta.clone()),
        ])));
        let glob_adapter = BrowseGlobStoreAdapter::new(Arc::clone(&browse));
        let grep_adapter = BrowseGrepStoreAdapter::new(Arc::clone(&browse));

        for repo_id in ["repo_alpha", "repo_beta"] {
            let glob = GlobStore::glob_paths(&glob_adapter, repo_id, "**/*.rs")
                .await
                .unwrap()
                .unwrap();
            assert!(glob.paths.iter().any(|path| path == "src/main.rs"));
            assert!(glob.paths.iter().all(|path| !path.starts_with("target/")));
            assert!(glob
                .paths
                .iter()
                .all(|path| !path.starts_with("node_modules/")));
            assert!(glob.paths.iter().all(|path| !path.starts_with("dist/")));
            assert!(glob.paths.iter().all(|path| !path.starts_with(".git/")));

            let grep = GrepStore::grep(&grep_adapter, repo_id, "shared_scale_marker")
                .await
                .unwrap()
                .unwrap();
            assert!(grep.matches.iter().any(|entry| entry.path == "src/main.rs"));
            assert!(grep
                .matches
                .iter()
                .all(|entry| !entry.path.starts_with("target/")));
            assert!(grep
                .matches
                .iter()
                .all(|entry| !entry.path.starts_with("node_modules/")));
            assert!(grep
                .matches
                .iter()
                .all(|entry| !entry.path.starts_with("dist/")));
            assert!(grep
                .matches
                .iter()
                .all(|entry| !entry.path.starts_with(".git/")));
        }

        fs::remove_dir_all(repo_alpha).unwrap();
        fs::remove_dir_all(repo_beta).unwrap();
    }

    #[tokio::test]
    async fn browse_grep_store_adapter_converts_browse_grep_for_core_retrieval() {
        let (store, root) = create_test_store();
        let adapter = BrowseGrepStoreAdapter::new(Arc::new(store));

        let grep = GrepStore::grep(&adapter, "repo_test", "main")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            grep,
            RepositoryGrep {
                repo_id: "repo_test".into(),
                query: "main".into(),
                matches: vec![RepositoryGrepMatch {
                    path: "src/main.rs".into(),
                    line_number: 1,
                    line: "fn main() {}".into(),
                }],
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
                is_binary: false,
            }
        );

        fs::remove_dir_all(root).unwrap();
    }
}
