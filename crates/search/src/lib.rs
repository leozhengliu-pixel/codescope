use anyhow::{Context, Result};
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::{
    collections::{BTreeSet, HashMap},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::Arc,
};

const DEFAULT_MAX_FILE_SIZE_BYTES: u64 = 1_000_000;
const MAX_INDEX_ARTIFACT_SIZE_BYTES: u64 = 10 * 1024 * 1024;
const MAX_REGEX_QUERY_BYTES: usize = 1024;
const MAX_REGEX_COMPILED_SIZE_BYTES: usize = 256 * 1024;
#[cfg(unix)]
const O_NONBLOCK: i32 = 0o4000;
#[cfg(unix)]
const O_CLOEXEC: i32 = 0o2000000;
#[cfg(unix)]
const O_NOFOLLOW: i32 = 0o400000;
const SKIPPED_DIR_NAMES: &[&str] = &[".git", ".sourcebot", "target", "node_modules", "dist"];
const SOURCEBOT_REWRITE_REPO_ID: &str = "repo_sourcebot_rewrite";
const SOURCEBOT_REWRITE_ROOT: &str = "/opt/data/projects/sourcebot-rewrite";

pub type DynSearchStore = Arc<dyn SearchStore>;

pub trait SearchStore: Send + Sync {
    fn search(&self, query: &str, repo_id: Option<&str>) -> Result<SearchResponse> {
        self.search_with_mode(query, repo_id, SearchMode::Boolean)
    }
    fn search_with_mode(
        &self,
        query: &str,
        repo_id: Option<&str>,
        mode: SearchMode,
    ) -> Result<SearchResponse>;
    fn repository_index_status(&self, repo_id: &str) -> Result<Option<RepositoryIndexStatus>>;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SearchMode {
    Boolean,
    Literal,
    Regex,
}

impl Default for SearchMode {
    fn default() -> Self {
        Self::Boolean
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryIndexState {
    Indexed,
    IndexedEmpty,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RepositoryIndexStatus {
    pub repo_id: String,
    pub status: RepositoryIndexState,
    pub indexed_file_count: usize,
    pub indexed_line_count: usize,
    pub skipped_file_count: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchResult {
    pub repo_id: String,
    pub path: String,
    pub line_number: usize,
    pub line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchPagination {
    pub limit: usize,
    pub offset: usize,
    pub total_count: usize,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchResponse {
    pub query: String,
    pub mode: SearchMode,
    pub repo_id: Option<String>,
    pub results: Vec<SearchResult>,
    pub pagination: SearchPagination,
}

impl SearchResponse {
    pub fn unpaginated(query: String, repo_id: Option<String>, results: Vec<SearchResult>) -> Self {
        Self::unpaginated_with_mode(query, SearchMode::Boolean, repo_id, results)
    }

    pub fn unpaginated_with_mode(
        query: String,
        mode: SearchMode,
        repo_id: Option<String>,
        results: Vec<SearchResult>,
    ) -> Self {
        let total_count = results.len();
        Self {
            query,
            mode,
            repo_id,
            results,
            pagination: SearchPagination {
                limit: total_count,
                offset: 0,
                total_count,
                has_more: false,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Class,
    Interface,
    TypeAlias,
    Constant,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SymbolRange {
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SymbolDefinition {
    pub path: String,
    pub name: String,
    pub kind: SymbolKind,
    pub range: SymbolRange,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status")]
pub enum SymbolExtraction {
    #[serde(rename = "supported")]
    Supported { symbols: Vec<SymbolDefinition> },
    #[serde(rename = "unsupported")]
    Unsupported {
        capability: String,
        symbols: Vec<SymbolDefinition>,
    },
}

#[derive(Clone)]
pub struct LocalSearchStore {
    repo_roots: HashMap<String, PathBuf>,
    max_file_size_bytes: u64,
    indexed_lines: HashMap<String, Vec<IndexedLine>>,
    index_statuses: HashMap<String, RepositoryIndexStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IndexedLine {
    path: String,
    line_number: usize,
    line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistedIndexArtifact {
    indexed_lines: Vec<IndexedLine>,
    index_status: RepositoryIndexStatus,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ParsedQuery {
    line_term_groups: Vec<Vec<String>>,
    path_filters: Vec<String>,
    language_filters: Vec<String>,
    invalid_filter: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedTerm {
    value: String,
    quoted: bool,
}

impl LocalSearchStore {
    pub fn new(repo_roots: HashMap<String, PathBuf>) -> Self {
        Self::build(repo_roots, DEFAULT_MAX_FILE_SIZE_BYTES)
    }

    fn build(repo_roots: HashMap<String, PathBuf>, max_file_size_bytes: u64) -> Self {
        let mut store = Self {
            repo_roots,
            max_file_size_bytes,
            indexed_lines: HashMap::new(),
            index_statuses: HashMap::new(),
        };
        store.rebuild_index();
        store
    }

    pub fn seeded() -> Self {
        Self::new(HashMap::from([(
            SOURCEBOT_REWRITE_REPO_ID.to_string(),
            PathBuf::from(SOURCEBOT_REWRITE_ROOT),
        )]))
    }

    fn open_index_artifact(artifact_path: &Path) -> Result<File> {
        validate_index_artifact_parent_components(artifact_path)?;

        let artifact_symlink_metadata = fs::symlink_metadata(artifact_path).with_context(|| {
            format!(
                "failed to read local search index artifact metadata {}",
                artifact_path.display()
            )
        })?;
        if !artifact_symlink_metadata.is_file() {
            anyhow::bail!(
                "search index artifact is not a regular file: {}",
                artifact_path.display()
            );
        }

        #[cfg(unix)]
        let artifact_file = OpenOptions::new()
            .read(true)
            .custom_flags(O_NONBLOCK | O_CLOEXEC | O_NOFOLLOW)
            .open(artifact_path)
            .with_context(|| {
                format!(
                    "failed to open local search index artifact {}",
                    artifact_path.display()
                )
            })?;

        #[cfg(not(unix))]
        let artifact_file = OpenOptions::new()
            .read(true)
            .open(artifact_path)
            .with_context(|| {
                format!(
                    "failed to open local search index artifact {}",
                    artifact_path.display()
                )
            })?;

        Ok(artifact_file)
    }

    pub fn from_index_artifact(repo_id: &str, artifact_path: &Path) -> Result<Self> {
        let artifact_file = Self::open_index_artifact(artifact_path)?;
        let artifact_metadata = artifact_file.metadata().with_context(|| {
            format!(
                "failed to read local search index artifact metadata {}",
                artifact_path.display()
            )
        })?;
        if !artifact_metadata.is_file() {
            anyhow::bail!(
                "search index artifact is not a regular file: {}",
                artifact_path.display()
            );
        }
        if artifact_metadata.len() > MAX_INDEX_ARTIFACT_SIZE_BYTES {
            anyhow::bail!(
                "search index artifact is too large: {} is {} bytes, maximum is {} bytes",
                artifact_path.display(),
                artifact_metadata.len(),
                MAX_INDEX_ARTIFACT_SIZE_BYTES
            );
        }

        let mut artifact_bytes = Vec::with_capacity(artifact_metadata.len() as usize);
        artifact_file
            .take(MAX_INDEX_ARTIFACT_SIZE_BYTES + 1)
            .read_to_end(&mut artifact_bytes)
            .with_context(|| {
                format!(
                    "failed to read local search index artifact {}",
                    artifact_path.display()
                )
            })?;
        if artifact_bytes.len() as u64 > MAX_INDEX_ARTIFACT_SIZE_BYTES {
            anyhow::bail!(
                "search index artifact is too large: {} exceeded maximum of {} bytes while reading",
                artifact_path.display(),
                MAX_INDEX_ARTIFACT_SIZE_BYTES
            );
        }
        let artifact: PersistedIndexArtifact = serde_json::from_slice(&artifact_bytes)
            .with_context(|| {
                format!(
                    "failed to parse local search index artifact {}",
                    artifact_path.display()
                )
            })?;
        validate_persisted_index_artifact(&artifact, repo_id, artifact_path)?;

        Ok(Self {
            repo_roots: HashMap::from([(repo_id.to_string(), PathBuf::new())]),
            max_file_size_bytes: DEFAULT_MAX_FILE_SIZE_BYTES,
            indexed_lines: HashMap::from([(repo_id.to_string(), artifact.indexed_lines)]),
            index_statuses: HashMap::from([(repo_id.to_string(), artifact.index_status)]),
        })
    }

    pub fn write_index_artifact(&self, repo_id: &str, artifact_path: &Path) -> Result<()> {
        let mut indexed_lines = self
            .indexed_lines
            .get(repo_id)
            .cloned()
            .with_context(|| format!("missing search index for repository {repo_id}"))?;
        indexed_lines.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then(left.line_number.cmp(&right.line_number))
        });
        let index_status = self
            .index_statuses
            .get(repo_id)
            .cloned()
            .with_context(|| format!("missing index status for repository {repo_id}"))?;
        let artifact = PersistedIndexArtifact {
            indexed_lines,
            index_status,
        };
        if let Some(parent) = artifact_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create local search index artifact directory {}",
                    parent.display()
                )
            })?;
        }
        validate_index_artifact_parent_components(artifact_path)?;

        let tmp_path = artifact_path.with_extension("json.tmp");
        if tmp_path.exists() {
            let tmp_metadata = fs::symlink_metadata(&tmp_path).with_context(|| {
                format!(
                    "failed to read local search index temp artifact metadata {}",
                    tmp_path.display()
                )
            })?;
            if tmp_metadata.file_type().is_symlink() || !tmp_metadata.is_file() {
                anyhow::bail!(
                    "search index temp artifact already exists and is not a regular file: {}",
                    tmp_path.display()
                );
            }
            fs::remove_file(&tmp_path).with_context(|| {
                format!(
                    "failed to remove stale local search index temp artifact {}",
                    tmp_path.display()
                )
            })?;
        }

        let artifact_json =
            serde_json::to_vec(&artifact).context("failed to serialize search index artifact")?;
        let mut tmp_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
            .with_context(|| {
                format!(
                    "failed to create local search index temp artifact {}",
                    tmp_path.display()
                )
            })?;
        tmp_file.write_all(&artifact_json).with_context(|| {
            format!(
                "failed to write local search index artifact {}",
                tmp_path.display()
            )
        })?;
        tmp_file.sync_all().with_context(|| {
            format!(
                "failed to sync local search index temp artifact {}",
                tmp_path.display()
            )
        })?;
        drop(tmp_file);
        fs::rename(&tmp_path, artifact_path).with_context(|| {
            format!(
                "failed to finalize local search index artifact {}",
                artifact_path.display()
            )
        })?;
        Ok(())
    }

    pub fn with_max_file_size_bytes(mut self, max_file_size_bytes: u64) -> Self {
        self.max_file_size_bytes = max_file_size_bytes;
        self.rebuild_index();
        self
    }

    fn rebuild_index(&mut self) {
        self.indexed_lines.clear();
        self.index_statuses.clear();

        for (repo_id, root) in &self.repo_roots {
            match self.index_repo(root) {
                Ok((lines, indexed_file_count, indexed_line_count, skipped_file_count)) => {
                    self.indexed_lines.insert(repo_id.clone(), lines);
                    self.index_statuses.insert(
                        repo_id.clone(),
                        RepositoryIndexStatus {
                            repo_id: repo_id.clone(),
                            status: if indexed_file_count == 0 {
                                RepositoryIndexState::IndexedEmpty
                            } else {
                                RepositoryIndexState::Indexed
                            },
                            indexed_file_count,
                            indexed_line_count,
                            skipped_file_count,
                            error: None,
                        },
                    );
                }
                Err(error) => {
                    self.indexed_lines.insert(repo_id.clone(), Vec::new());
                    self.index_statuses.insert(
                        repo_id.clone(),
                        RepositoryIndexStatus {
                            repo_id: repo_id.clone(),
                            status: RepositoryIndexState::Error,
                            indexed_file_count: 0,
                            indexed_line_count: 0,
                            skipped_file_count: 0,
                            error: Some(error.to_string()),
                        },
                    );
                }
            }
        }
    }

    fn index_repo(&self, root: &Path) -> Result<(Vec<IndexedLine>, usize, usize, usize)> {
        validate_repository_root(root)?;

        let mut lines = Vec::new();
        let mut indexed_file_count = 0;
        let mut indexed_line_count = 0;
        let mut skipped_file_count = 0;
        self.collect_indexed_lines(
            root,
            root,
            &mut lines,
            &mut indexed_file_count,
            &mut indexed_line_count,
            &mut skipped_file_count,
        )?;
        lines.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then(left.line_number.cmp(&right.line_number))
        });
        Ok((
            lines,
            indexed_file_count,
            indexed_line_count,
            skipped_file_count,
        ))
    }

    fn search_repo(&self, repo_id: &str, matcher: &QueryMatcher) -> Vec<SearchResult> {
        self.indexed_lines
            .get(repo_id)
            .into_iter()
            .flatten()
            .filter(|indexed_line| indexed_line_matches_query(indexed_line, matcher))
            .map(|indexed_line| SearchResult {
                repo_id: repo_id.to_string(),
                path: indexed_line.path.clone(),
                line_number: indexed_line.line_number,
                line: indexed_line.line.clone(),
            })
            .collect()
    }

    fn collect_indexed_lines(
        &self,
        root: &Path,
        current_path: &Path,
        lines: &mut Vec<IndexedLine>,
        indexed_file_count: &mut usize,
        indexed_line_count: &mut usize,
        skipped_file_count: &mut usize,
    ) -> Result<()> {
        let mut entries: Vec<_> = fs::read_dir(current_path)
            .with_context(|| format!("failed to read directory {}", current_path.display()))?
            .collect::<std::result::Result<_, _>>()
            .with_context(|| {
                format!(
                    "failed to read directory entry under {}",
                    current_path.display()
                )
            })?;
        entries.sort_by_key(|entry: &fs::DirEntry| entry.path());

        for entry in entries {
            let path = entry.path();
            let file_type = entry
                .file_type()
                .with_context(|| format!("failed to read file type for {}", path.display()))?;

            if file_type.is_dir() {
                if should_skip_directory(&path) {
                    *skipped_file_count += 1;
                    continue;
                }

                self.collect_indexed_lines(
                    root,
                    &path,
                    lines,
                    indexed_file_count,
                    indexed_line_count,
                    skipped_file_count,
                )?;
                continue;
            }

            if !file_type.is_file() {
                continue;
            }

            let relative_path = path
                .strip_prefix(root)
                .with_context(|| format!("failed to strip prefix for {}", path.display()))?;

            if should_skip_file(relative_path) {
                *skipped_file_count += 1;
                continue;
            }

            let metadata = fs::metadata(&path)
                .with_context(|| format!("failed to read metadata for {}", path.display()))?;
            if metadata.len() > self.max_file_size_bytes {
                *skipped_file_count += 1;
                continue;
            }

            let Ok(contents) = fs::read_to_string(&path) else {
                *skipped_file_count += 1;
                continue;
            };

            if is_obviously_binary(&contents) {
                *skipped_file_count += 1;
                continue;
            }

            let relative_path = relative_path.to_string_lossy().replace('\\', "/");

            *indexed_file_count += 1;
            for (index, line) in contents.lines().enumerate() {
                *indexed_line_count += 1;
                lines.push(IndexedLine {
                    path: relative_path.clone(),
                    line_number: index + 1,
                    line: line.to_string(),
                });
            }
        }

        Ok(())
    }
}

fn validate_repository_root(root: &Path) -> Result<()> {
    let metadata_root = repository_root_metadata_path(root);
    let root_metadata = fs::symlink_metadata(&metadata_root).with_context(|| {
        format!(
            "failed to read search repository root metadata {}",
            root.display()
        )
    })?;
    let root_file_type = root_metadata.file_type();
    if root_file_type.is_symlink() {
        anyhow::bail!("search repository root is a symlink: {}", root.display());
    }
    if !root_file_type.is_dir() {
        anyhow::bail!(
            "search repository root is not a directory: {}",
            root.display()
        );
    }

    Ok(())
}

#[cfg(unix)]
fn repository_root_metadata_path(root: &Path) -> PathBuf {
    root.components()
        .fold(PathBuf::new(), |mut path, component| {
            path.push(component.as_os_str());
            path
        })
}

#[cfg(not(unix))]
fn repository_root_metadata_path(root: &Path) -> PathBuf {
    root.to_path_buf()
}

fn validate_index_artifact_parent_components(artifact_path: &Path) -> Result<()> {
    for parent in artifact_path.ancestors().skip(1) {
        if parent.as_os_str().is_empty() {
            continue;
        }

        let parent_metadata = fs::symlink_metadata(parent).with_context(|| {
            format!(
                "failed to read local search index artifact parent metadata {}",
                parent.display()
            )
        })?;
        if parent_metadata.file_type().is_symlink() {
            anyhow::bail!(
                "search index artifact parent contains a symlink: {}",
                parent.display()
            );
        }
    }

    Ok(())
}

fn validate_persisted_index_artifact(
    artifact: &PersistedIndexArtifact,
    expected_repo_id: &str,
    artifact_path: &Path,
) -> Result<()> {
    if artifact.index_status.repo_id != expected_repo_id {
        anyhow::bail!(
            "search index artifact repo_id mismatch in {}: expected '{}', found '{}'",
            artifact_path.display(),
            expected_repo_id,
            artifact.index_status.repo_id
        );
    }

    let actual_indexed_line_count = artifact.indexed_lines.len();
    if artifact.index_status.indexed_line_count != actual_indexed_line_count {
        anyhow::bail!(
            "search index artifact status count mismatch in {}: status reports {} indexed lines, artifact contains {} indexed lines",
            artifact_path.display(),
            artifact.index_status.indexed_line_count,
            actual_indexed_line_count
        );
    }

    let unique_indexed_paths: BTreeSet<&str> = artifact
        .indexed_lines
        .iter()
        .map(|indexed_line| indexed_line.path.as_str())
        .collect();
    if artifact.index_status.indexed_file_count < unique_indexed_paths.len() {
        anyhow::bail!(
            "search index artifact status file count undercounts stored paths in {}: status reports {} indexed files, artifact contains indexed lines from {} unique paths",
            artifact_path.display(),
            artifact.index_status.indexed_file_count,
            unique_indexed_paths.len()
        );
    }

    match artifact.index_status.status {
        RepositoryIndexState::Indexed if artifact.index_status.indexed_file_count == 0 => {
            anyhow::bail!(
                "search index artifact indexed status has no indexed files in {}: writer should persist indexed_empty for empty indexes",
                artifact_path.display()
            );
        }
        RepositoryIndexState::IndexedEmpty
            if artifact.index_status.indexed_file_count != 0
                || artifact.index_status.indexed_line_count != 0 =>
        {
            anyhow::bail!(
                "search index artifact status state mismatch in {}: indexed_empty status reports {} indexed files and {} indexed lines",
                artifact_path.display(),
                artifact.index_status.indexed_file_count,
                artifact.index_status.indexed_line_count
            );
        }
        RepositoryIndexState::Error
            if artifact.index_status.indexed_file_count != 0
                || artifact.index_status.indexed_line_count != 0
                || !artifact.indexed_lines.is_empty() =>
        {
            anyhow::bail!(
                "search index artifact error status carries indexed content in {}: {} indexed files, {} indexed lines, {} stored lines",
                artifact_path.display(),
                artifact.index_status.indexed_file_count,
                artifact.index_status.indexed_line_count,
                artifact.indexed_lines.len()
            );
        }
        RepositoryIndexState::Error
            if artifact
                .index_status
                .error
                .as_deref()
                .is_none_or(|error| error.trim().is_empty()) =>
        {
            anyhow::bail!(
                "search index artifact error status is missing an operator-visible error in {}",
                artifact_path.display()
            );
        }
        RepositoryIndexState::Indexed | RepositoryIndexState::IndexedEmpty
            if artifact.index_status.error.is_some() =>
        {
            anyhow::bail!(
                "search index artifact success status carries an error in {}",
                artifact_path.display()
            );
        }
        _ => {}
    }

    let mut seen_line_keys = BTreeSet::new();
    let mut previous_line_key: Option<(&str, usize)> = None;
    for indexed_line in &artifact.indexed_lines {
        if !is_safe_persisted_index_path(&indexed_line.path) {
            anyhow::bail!(
                "unsafe search index artifact path '{}' in {}",
                indexed_line.path,
                artifact_path.display()
            );
        }
        if indexed_line.line_number == 0 {
            anyhow::bail!(
                "invalid search index artifact line number 0 for '{}' in {}",
                indexed_line.path,
                artifact_path.display()
            );
        }
        if indexed_line.line_number > artifact.index_status.indexed_line_count {
            anyhow::bail!(
                "search index artifact line number exceeds indexed line count for '{}:{}' in {}: status reports {} indexed lines",
                indexed_line.path,
                indexed_line.line_number,
                artifact_path.display(),
                artifact.index_status.indexed_line_count
            );
        }

        let line_key = (indexed_line.path.as_str(), indexed_line.line_number);
        if !seen_line_keys.insert(line_key) {
            anyhow::bail!(
                "duplicate search index artifact line key '{}:{}' in {}",
                indexed_line.path,
                indexed_line.line_number,
                artifact_path.display()
            );
        }
        if previous_line_key.is_some_and(|previous_line_key| line_key < previous_line_key) {
            anyhow::bail!(
                "unsorted search index artifact line key '{}:{}' in {}: persisted lines must be ordered by path then line number",
                indexed_line.path,
                indexed_line.line_number,
                artifact_path.display()
            );
        }
        match previous_line_key {
            Some((previous_path, previous_line_number)) if previous_path == indexed_line.path => {
                if indexed_line.line_number != previous_line_number + 1 {
                    anyhow::bail!(
                        "non-contiguous search index artifact line key '{}:{}' in {}: expected next line number {}",
                        indexed_line.path,
                        indexed_line.line_number,
                        artifact_path.display(),
                        previous_line_number + 1
                    );
                }
            }
            _ if indexed_line.line_number != 1 => {
                anyhow::bail!(
                    "non-contiguous search index artifact line key '{}:{}' in {}: first stored line for each path must be line 1",
                    indexed_line.path,
                    indexed_line.line_number,
                    artifact_path.display()
                );
            }
            _ => {}
        }
        previous_line_key = Some(line_key);
    }
    Ok(())
}

fn is_safe_persisted_index_path(path: &str) -> bool {
    if path.is_empty()
        || path.contains('\\')
        || path.contains("//")
        || path.ends_with('/')
        || path == "."
        || path.starts_with("./")
        || path.ends_with("/.")
        || path.contains("/./")
    {
        return false;
    }

    let mut saw_component = false;
    for component in Path::new(path).components() {
        match component {
            Component::Normal(_) => saw_component = true,
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => return false,
        }
    }
    saw_component
}

impl SearchStore for LocalSearchStore {
    fn search_with_mode(
        &self,
        query: &str,
        repo_id: Option<&str>,
        mode: SearchMode,
    ) -> Result<SearchResponse> {
        let query = query.trim();
        let requested_repo_id = repo_id.map(str::trim).filter(|value| !value.is_empty());
        let matcher = QueryMatcher::from_query(query, mode);

        let mut repos: Vec<&String> = match requested_repo_id {
            Some(repo_id) => self
                .repo_roots
                .get_key_value(repo_id)
                .map(|(repo_id, _)| repo_id)
                .into_iter()
                .collect(),
            None => self.repo_roots.keys().collect(),
        };
        repos.sort();

        let mut results = Vec::new();
        for repo_id in repos {
            results.extend(self.search_repo(repo_id, &matcher));
        }

        Ok(SearchResponse::unpaginated_with_mode(
            query.to_string(),
            mode,
            requested_repo_id.map(ToOwned::to_owned),
            results,
        ))
    }

    fn repository_index_status(&self, repo_id: &str) -> Result<Option<RepositoryIndexStatus>> {
        Ok(self.index_statuses.get(repo_id).cloned())
    }
}

enum QueryMatcher {
    Boolean(ParsedQuery),
    Literal(String),
    Regex(Option<Regex>),
}

impl QueryMatcher {
    fn from_query(query: &str, mode: SearchMode) -> Self {
        match mode {
            SearchMode::Boolean => Self::Boolean(parse_query(&query.to_lowercase())),
            SearchMode::Literal => Self::Literal(query.to_lowercase()),
            SearchMode::Regex => {
                if query.is_empty() || query.len() > MAX_REGEX_QUERY_BYTES {
                    return Self::Regex(None);
                }

                Self::Regex(
                    RegexBuilder::new(query)
                        .case_insensitive(true)
                        .size_limit(MAX_REGEX_COMPILED_SIZE_BYTES)
                        .build()
                        .ok(),
                )
            }
        }
    }
}

fn indexed_line_matches_query(indexed_line: &IndexedLine, matcher: &QueryMatcher) -> bool {
    let normalized_line = indexed_line.line.to_lowercase();

    match matcher {
        QueryMatcher::Boolean(parsed_query) => {
            if parsed_query.invalid_filter || parsed_query.line_term_groups.is_empty() {
                return false;
            }
            let normalized_path = indexed_line.path.to_lowercase();
            parsed_query
                .line_term_groups
                .iter()
                .any(|group| group.iter().all(|term| normalized_line.contains(term)))
                && parsed_query
                    .path_filters
                    .iter()
                    .all(|filter| path_matches_filter(&normalized_path, filter))
                && parsed_query
                    .language_filters
                    .iter()
                    .all(|language| path_matches_language(&normalized_path, language))
        }
        QueryMatcher::Literal(literal) => {
            !literal.is_empty() && normalized_line.contains(literal.as_str())
        }
        QueryMatcher::Regex(Some(regex)) => regex.is_match(&indexed_line.line),
        QueryMatcher::Regex(None) => false,
    }
}

fn parse_query(normalized_query: &str) -> ParsedQuery {
    let mut parsed = ParsedQuery::default();
    let mut current_line_terms = Vec::new();
    for term in parse_query_terms(normalized_query) {
        if !term.quoted && term.value == "or" {
            if current_line_terms.is_empty() {
                parsed.invalid_filter = true;
            } else {
                parsed.line_term_groups.push(current_line_terms);
                current_line_terms = Vec::new();
            }
            continue;
        }

        if !term.quoted {
            if let Some(path_filter) = term.value.strip_prefix("path:") {
                if path_filter.trim().is_empty() {
                    parsed.invalid_filter = true;
                } else {
                    parsed.path_filters.push(path_filter.trim().to_string());
                }
                continue;
            }
            if let Some(language_filter) = term.value.strip_prefix("lang:") {
                if language_filter.trim().is_empty() {
                    parsed.invalid_filter = true;
                } else {
                    parsed
                        .language_filters
                        .push(language_filter.trim().to_string());
                }
                continue;
            }
        }
        current_line_terms.push(term.value);
    }
    if current_line_terms.is_empty() {
        if !parsed.line_term_groups.is_empty() {
            parsed.invalid_filter = true;
        }
    } else {
        parsed.line_term_groups.push(current_line_terms);
    }
    parsed
}

fn path_matches_filter(normalized_path: &str, filter: &str) -> bool {
    if filter.contains('*') || filter.contains('?') {
        return wildcard_path_matches(normalized_path.as_bytes(), filter.as_bytes());
    }

    normalized_path.contains(filter)
}

fn wildcard_path_matches(path: &[u8], pattern: &[u8]) -> bool {
    let mut memo = vec![vec![None; pattern.len() + 1]; path.len() + 1];
    wildcard_path_matches_from(path, pattern, 0, 0, &mut memo)
}

fn wildcard_path_matches_from(
    path: &[u8],
    pattern: &[u8],
    path_index: usize,
    pattern_index: usize,
    memo: &mut [Vec<Option<bool>>],
) -> bool {
    if let Some(matched) = memo[path_index][pattern_index] {
        return matched;
    }

    let matched = if pattern_index == pattern.len() {
        path_index == path.len()
    } else if pattern[pattern_index] == b'*' {
        let is_double_star = pattern.get(pattern_index + 1) == Some(&b'*');
        let next_pattern_index = pattern_index + if is_double_star { 2 } else { 1 };
        wildcard_path_matches_from(path, pattern, path_index, next_pattern_index, memo)
            || (path_index < path.len()
                && (is_double_star || path[path_index] != b'/')
                && wildcard_path_matches_from(path, pattern, path_index + 1, pattern_index, memo))
    } else if path_index < path.len()
        && (pattern[pattern_index] == path[path_index]
            || (pattern[pattern_index] == b'?' && path[path_index] != b'/'))
    {
        wildcard_path_matches_from(path, pattern, path_index + 1, pattern_index + 1, memo)
    } else {
        false
    };

    memo[path_index][pattern_index] = Some(matched);
    matched
}

fn path_matches_language(normalized_path: &str, language: &str) -> bool {
    let Some(extension) = language_to_extension(language) else {
        return false;
    };
    normalized_path.ends_with(extension)
}

fn language_to_extension(language: &str) -> Option<&'static str> {
    match language {
        "rust" | "rs" => Some(".rs"),
        "tsx" => Some(".tsx"),
        "javascript" | "js" => Some(".js"),
        "jsx" => Some(".jsx"),
        "python" | "py" => Some(".py"),
        "markdown" | "md" => Some(".md"),
        "go" => Some(".go"),
        _ => None,
    }
}

fn parse_query_terms(normalized_query: &str) -> Vec<ParsedTerm> {
    let mut terms = Vec::new();
    let mut pending = String::new();
    let mut chars = normalized_query.chars();

    while let Some(ch) = chars.next() {
        if ch == '"' {
            if !pending.trim().is_empty() {
                terms.extend(pending.split_whitespace().map(|value| ParsedTerm {
                    value: value.to_string(),
                    quoted: false,
                }));
                pending.clear();
            }

            let mut phrase = String::new();
            let mut closed = false;
            for phrase_ch in chars.by_ref() {
                if phrase_ch == '"' {
                    closed = true;
                    break;
                }
                phrase.push(phrase_ch);
            }

            if closed {
                let phrase = phrase.trim();
                if !phrase.is_empty() {
                    terms.push(ParsedTerm {
                        value: phrase.to_string(),
                        quoted: true,
                    });
                }
            } else {
                pending.push('"');
                pending.push_str(&phrase);
            }
            continue;
        }

        pending.push(ch);
    }

    if !pending.trim().is_empty() {
        terms.extend(pending.split_whitespace().map(|value| ParsedTerm {
            value: value.to_string(),
            quoted: false,
        }));
    }

    terms
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

pub fn extract_symbols(path: &str, content: &str) -> SymbolExtraction {
    let extension = Path::new(path).extension().and_then(|value| value.to_str());

    match extension {
        Some("rs") => SymbolExtraction::Supported {
            symbols: extract_rust_symbols(path, content),
        },
        Some("ts" | "tsx" | "js" | "jsx") => SymbolExtraction::Supported {
            symbols: extract_typescript_like_symbols(path, content),
        },
        Some(ext) => SymbolExtraction::Unsupported {
            capability: format!("symbol extraction is not supported for .{ext} files"),
            symbols: Vec::new(),
        },
        None => SymbolExtraction::Unsupported {
            capability: "symbol extraction is not supported for files without an extension"
                .to_string(),
            symbols: Vec::new(),
        },
    }
}

fn extract_rust_symbols(path: &str, content: &str) -> Vec<SymbolDefinition> {
    let lines: Vec<&str> = content.lines().collect();
    let mut symbols = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        if line.trim().is_empty() || line.trim_start() != *line {
            continue;
        }

        let Some((kind, name)) = parse_rust_symbol_line(line) else {
            continue;
        };

        symbols.push(SymbolDefinition {
            path: path.to_string(),
            name,
            kind,
            range: SymbolRange {
                start_line: index + 1,
                end_line: find_braced_symbol_end_line(&lines, index),
            },
        });
    }

    symbols
}

fn extract_typescript_like_symbols(path: &str, content: &str) -> Vec<SymbolDefinition> {
    let lines: Vec<&str> = content.lines().collect();
    let mut symbols = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        if line.trim().is_empty() || line.trim_start() != *line {
            continue;
        }

        let Some((kind, name)) = parse_typescript_like_symbol_line(line) else {
            continue;
        };

        symbols.push(SymbolDefinition {
            path: path.to_string(),
            name,
            kind,
            range: SymbolRange {
                start_line: index + 1,
                end_line: if line.contains('{') {
                    find_braced_symbol_end_line(&lines, index)
                } else {
                    index + 1
                },
            },
        });
    }

    symbols
}

fn parse_rust_symbol_line(line: &str) -> Option<(SymbolKind, String)> {
    let trimmed = line.trim();

    for (keyword, kind) in [
        ("fn", SymbolKind::Function),
        ("struct", SymbolKind::Struct),
        ("enum", SymbolKind::Enum),
        ("trait", SymbolKind::Trait),
    ] {
        if let Some(name) = extract_rust_symbol_name(trimmed, keyword) {
            return Some((kind, name));
        }
    }

    None
}

fn parse_typescript_like_symbol_line(line: &str) -> Option<(SymbolKind, String)> {
    let mut trimmed = line.trim();

    for prefix in ["export default ", "export ", "declare ", "abstract "] {
        trimmed = trimmed.strip_prefix(prefix).unwrap_or(trimmed).trim_start();
    }
    trimmed = trimmed
        .strip_prefix("async ")
        .unwrap_or(trimmed)
        .trim_start();

    for (keyword, kind) in [
        ("function", SymbolKind::Function),
        ("class", SymbolKind::Class),
        ("interface", SymbolKind::Interface),
        ("enum", SymbolKind::Enum),
        ("type", SymbolKind::TypeAlias),
    ] {
        if let Some(name) = extract_typescript_like_symbol_name(trimmed, keyword) {
            return Some((kind, name));
        }
    }

    for keyword in ["const", "let", "var"] {
        let Some(remainder) = trimmed.strip_prefix(keyword) else {
            continue;
        };
        let declaration = remainder.trim_start();
        let name = declaration
            .chars()
            .take_while(|ch| ch.is_alphanumeric() || *ch == '_' || *ch == '$')
            .collect::<String>();
        if name.is_empty() {
            return None;
        }
        let remainder = declaration[name.len()..].trim_start();
        if remainder.contains("=>") || remainder.starts_with("= function") {
            return Some((SymbolKind::Function, name));
        }
        if keyword == "const" {
            return Some((SymbolKind::Constant, name));
        }
    }

    None
}

fn extract_rust_symbol_name(line: &str, keyword: &str) -> Option<String> {
    let remainder = strip_rust_visibility(line).unwrap_or(line).trim_start();
    let remainder = remainder.strip_prefix("async ").unwrap_or(remainder);
    let remainder = remainder.strip_prefix(keyword)?;

    let name = remainder
        .trim_start()
        .chars()
        .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
        .collect::<String>();

    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn extract_typescript_like_symbol_name(line: &str, keyword: &str) -> Option<String> {
    let remainder = line.strip_prefix(keyword)?.trim_start();
    let name = remainder
        .chars()
        .take_while(|ch| ch.is_alphanumeric() || *ch == '_' || *ch == '$')
        .collect::<String>();

    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn strip_rust_visibility(line: &str) -> Option<&str> {
    if let Some(remainder) = line.strip_prefix("pub ") {
        Some(remainder)
    } else if let Some(remainder) = line.strip_prefix("pub(crate) ") {
        Some(remainder)
    } else if let Some(remainder) = line.strip_prefix("pub(super) ") {
        Some(remainder)
    } else if let Some(remainder) = line.strip_prefix("pub(self) ") {
        Some(remainder)
    } else if let Some(remainder) = line.strip_prefix("pub(in ") {
        Some(remainder.split_once(')')?.1.trim_start())
    } else {
        None
    }
}

fn find_braced_symbol_end_line(lines: &[&str], start_index: usize) -> usize {
    let mut brace_depth = 0usize;
    let mut saw_open_brace = false;

    for (index, line) in lines.iter().enumerate().skip(start_index) {
        for ch in line.chars() {
            match ch {
                '{' => {
                    brace_depth += 1;
                    saw_open_brace = true;
                }
                '}' => {
                    brace_depth = brace_depth.saturating_sub(1);
                }
                _ => {}
            }
        }

        if saw_open_brace && brace_depth == 0 {
            return index + 1;
        }
    }

    start_index + 1
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

    fn create_test_store() -> (LocalSearchStore, PathBuf) {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "search",
            "build_router is documented here\n",
            "fn build_router() {}\nfn other() {}\n",
            "target/generated.txt",
        );
        fixture.add_search_ignored_and_binary_variants();
        let root = fixture.root;

        let store = LocalSearchStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));
        (store, root)
    }

    #[test]
    fn shared_repo_tree_fixture_exposes_search_common_layout() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "search-common-layout",
            "build_router is documented here\n",
            "fn build_router() {}\nfn other() {}\n",
            "target/generated.txt",
        );

        repo_tree_fixture::assert_common_layout(&fixture.root, "target/generated.txt");

        fs::remove_dir_all(fixture.root).unwrap();
    }

    #[test]
    fn shared_repo_tree_fixture_rejects_parent_directory_escapes_in_generated_paths() {
        let panic = std::panic::catch_unwind(|| {
            repo_tree_fixture::CanonicalRepoTreeRoot::create(
                "search-invalid-generated-path",
                "build_router is documented here\n",
                "fn build_router() {}\nfn other() {}\n",
                "target/../escape.txt",
            )
        });

        assert!(panic.is_err());
    }

    #[test]
    fn shared_repo_tree_fixture_can_add_search_ignored_and_binary_variants() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "search-variants",
            "build_router is documented here\n",
            "fn build_router() {}\nfn other() {}\n",
            "target/generated.txt",
        );

        fixture.add_search_ignored_and_binary_variants();

        assert_eq!(
            fs::read_to_string(fixture.root.join(".git").join("HEAD")).unwrap(),
            "build_router should be ignored\n"
        );
        assert_eq!(
            fs::read_to_string(fixture.root.join("target").join("generated.txt")).unwrap(),
            "build_router should also be ignored\n"
        );
        assert!(fixture.root.join("image.png").is_file());
        assert_eq!(
            fs::read(fixture.root.join("binary.dat")).unwrap(),
            vec![0_u8, 159, 146, 150]
        );

        fs::remove_dir_all(fixture.root).unwrap();
    }

    #[test]
    fn local_search_store_returns_line_matches() {
        let (store, root) = create_test_store();
        fs::write(root.join("post_index.txt"), "post_index_marker\n").unwrap();

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

        let post_index_response = store
            .search("post_index_marker", Some("repo_test"))
            .unwrap();
        assert!(
            post_index_response.results.is_empty(),
            "search must use the startup-built index rather than rewalking the filesystem per query"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_matches_queries_case_insensitively() {
        let (store, root) = create_test_store();

        let response = store.search("BUILD_ROUTER", Some("repo_test")).unwrap();

        assert_eq!(response.query, "BUILD_ROUTER");
        assert!(response.results.iter().any(|result| {
            result.path == "src/main.rs"
                && result.line_number == 1
                && result.line == "fn build_router() {}"
        }));
        assert!(response
            .results
            .iter()
            .any(|result| result.path == "README.md" && result.line_number == 1));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_matches_all_space_separated_query_terms() {
        let (store, root) = create_test_store();

        let response = store.search("build documented", Some("repo_test")).unwrap();

        assert_eq!(response.query, "build documented");
        assert!(response.results.iter().any(|result| {
            result.path == "README.md"
                && result.line_number == 1
                && result.line == "build_router is documented here"
        }));
        assert!(response
            .results
            .iter()
            .all(|result| result.path != "src/main.rs"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_matches_quoted_phrases_and_unquoted_terms() {
        let (store, root) = create_test_store();

        let exact_phrase_response = store.search("\"build router\"", Some("repo_test")).unwrap();
        assert!(
            exact_phrase_response.results.is_empty(),
            "quoted phrases should not be split into broad all-term matches"
        );

        let empty_phrase_response = store.search("\"\"", Some("repo_test")).unwrap();
        assert!(
            empty_phrase_response.results.is_empty(),
            "empty quoted phrases should not broaden into all indexed lines"
        );

        let mixed_phrase_response = store
            .search("\"build_router is documented\" here", Some("repo_test"))
            .unwrap();
        assert_eq!(
            mixed_phrase_response.query,
            "\"build_router is documented\" here"
        );
        assert!(mixed_phrase_response.results.iter().any(|result| {
            result.path == "README.md"
                && result.line_number == 1
                && result.line == "build_router is documented here"
        }));
        assert!(mixed_phrase_response
            .results
            .iter()
            .all(|result| result.path != "src/main.rs"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_matches_bounded_boolean_or_groups() {
        let (store, root) = create_test_store();

        let response = store
            .search("build_router OR documented", Some("repo_test"))
            .unwrap();

        assert_eq!(response.query, "build_router OR documented");
        assert!(response.results.iter().any(|result| {
            result.path == "src/main.rs"
                && result.line_number == 1
                && result.line == "fn build_router() {}"
        }));
        assert!(response.results.iter().any(|result| {
            result.path == "README.md"
                && result.line_number == 1
                && result.line == "build_router is documented here"
        }));

        let filtered_response = store
            .search("lang:rust documented OR here", Some("repo_test"))
            .unwrap();
        assert!(
            filtered_response.results.is_empty(),
            "global filters should constrain every boolean OR group instead of widening across paths/languages"
        );

        let malformed_response = store.search("OR build_router", Some("repo_test")).unwrap();
        assert!(
            malformed_response.results.is_empty(),
            "empty boolean OR groups should fail closed instead of being ignored"
        );

        let quoted_or_response = store
            .search("\"build_router or documented\"", Some("repo_test"))
            .unwrap();
        assert!(
            quoted_or_response.results.is_empty(),
            "quoted OR text should remain a literal phrase rather than becoming a boolean operator"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_supports_explicit_literal_and_regex_modes() {
        let (store, root) = create_test_store();

        let literal_response = store
            .search_with_mode(
                "lang:rust path:src build_router",
                Some("repo_test"),
                SearchMode::Literal,
            )
            .unwrap();
        assert_eq!(literal_response.mode, SearchMode::Literal);
        assert!(
            literal_response.results.is_empty(),
            "literal mode should treat filter-looking text as the exact line substring instead of parser syntax"
        );

        let literal_phrase_response = store
            .search_with_mode(
                "build_router is documented",
                Some("repo_test"),
                SearchMode::Literal,
            )
            .unwrap();
        assert!(literal_phrase_response.results.iter().any(|result| {
            result.path == "README.md" && result.line == "build_router is documented here"
        }));

        let regex_response = store
            .search_with_mode(r"fn\s+build_router", Some("repo_test"), SearchMode::Regex)
            .unwrap();
        assert_eq!(regex_response.mode, SearchMode::Regex);
        assert_eq!(regex_response.results.len(), 1);
        assert_eq!(regex_response.results[0].path, "src/main.rs");

        let invalid_regex_response = store
            .search_with_mode("[", Some("repo_test"), SearchMode::Regex)
            .unwrap();
        assert!(
            invalid_regex_response.results.is_empty(),
            "invalid regex mode queries should fail closed to no matches"
        );

        let empty_regex_response = store
            .search_with_mode("   ", Some("repo_test"), SearchMode::Regex)
            .unwrap();
        assert!(
            empty_regex_response.results.is_empty(),
            "empty regex mode queries should fail closed instead of matching every indexed line"
        );

        let oversized_matching_regex = format!("build_router|{}", "a".repeat(2_048));
        let oversized_regex_response = store
            .search_with_mode(
                &oversized_matching_regex,
                Some("repo_test"),
                SearchMode::Regex,
            )
            .unwrap();
        assert!(
            oversized_regex_response.results.is_empty(),
            "oversized regex mode queries should fail closed before compiling or matching"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_applies_lang_and_path_filters_without_line_term_broadening() {
        let (store, root) = create_test_store();

        let response = store
            .search("lang:rust path:src build_router", Some("repo_test"))
            .unwrap();

        assert_eq!(response.query, "lang:rust path:src build_router");
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].path, "src/main.rs");
        assert_eq!(response.results[0].line, "fn build_router() {}");

        let missing_path_response = store
            .search("path:README build_router", Some("repo_test"))
            .unwrap();
        assert!(
            missing_path_response
                .results
                .iter()
                .all(|result| result.path == "README.md"),
            "path filters should constrain result paths rather than acting as line terms"
        );
        assert!(missing_path_response
            .results
            .iter()
            .all(|result| result.path != "src/main.rs"));

        let unknown_language_response = store
            .search("lang:python build_router", Some("repo_test"))
            .unwrap();
        assert!(
            unknown_language_response.results.is_empty(),
            "unsupported or unmatched language filters should fail closed to empty results"
        );

        let malformed_language_response = store
            .search("lang: build_router", Some("repo_test"))
            .unwrap();
        assert!(
            malformed_language_response.results.is_empty(),
            "empty language filters should fail closed instead of broadening to line terms"
        );

        let quoted_filter_response = store
            .search("\"path:src\" build_router", Some("repo_test"))
            .unwrap();
        assert!(
            quoted_filter_response.results.is_empty(),
            "quoted path:/lang: text should remain a literal phrase rather than becoming a filter"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_applies_path_glob_filters_to_result_paths() {
        let root = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "search-path-glob-filter",
            "path_glob_marker in readme\n",
            "fn path_glob_marker() {}\n",
            "target/generated.txt",
        )
        .root;
        fs::create_dir_all(root.join("src").join("nested")).unwrap();
        fs::write(
            root.join("src").join("nested").join("lib.rs"),
            "fn path_glob_marker_nested() {}\n",
        )
        .unwrap();
        fs::write(root.join("src").join("main.ts"), "path_glob_marker ts\n").unwrap();

        let store = LocalSearchStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));
        let response = store
            .search("path:src/*.rs path_glob_marker", Some("repo_test"))
            .unwrap();

        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].path, "src/main.rs");
        assert!(
            response
                .results
                .iter()
                .all(|result| result.path != "src/nested/lib.rs" && result.path != "src/main.ts"),
            "single-star path globs should stay within one path component"
        );

        let recursive_response = store
            .search("path:src/**/*.rs path_glob_marker", Some("repo_test"))
            .unwrap();
        assert_eq!(recursive_response.results.len(), 1);
        assert_eq!(recursive_response.results[0].path, "src/nested/lib.rs");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_returns_deterministic_repo_path_line_order() {
        let root_a = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "search-order-a",
            "common_marker in readme\n",
            "common_marker in source\ncommon_marker second source line\n",
            "target/generated.txt",
        )
        .root;
        fs::create_dir_all(root_a.join("docs")).unwrap();
        fs::write(
            root_a.join("docs").join("guide.md"),
            "common_marker in docs\n",
        )
        .unwrap();
        fs::create_dir_all(root_a.join("a")).unwrap();
        fs::write(root_a.join("a").join("file.rs"), "common_marker nested\n").unwrap();
        fs::write(root_a.join("a.rs"), "common_marker sibling\n").unwrap();

        let root_b = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "search-order-b",
            "common_marker in other repo\n",
            "common_marker in other source\n",
            "target/generated.txt",
        )
        .root;

        let store = LocalSearchStore::new(HashMap::from([
            ("repo_z".to_string(), root_b.clone()),
            ("repo_a".to_string(), root_a.clone()),
        ]));

        let response = store.search("common_marker", None).unwrap();
        let actual_order: Vec<_> = response
            .results
            .iter()
            .map(|result| {
                (
                    result.repo_id.as_str(),
                    result.path.as_str(),
                    result.line_number,
                )
            })
            .collect();

        assert_eq!(
            actual_order,
            vec![
                ("repo_a", "README.md", 1),
                ("repo_a", "a.rs", 1),
                ("repo_a", "a/file.rs", 1),
                ("repo_a", "docs/guide.md", 1),
                ("repo_a", "src/main.rs", 1),
                ("repo_a", "src/main.rs", 2),
                ("repo_z", "README.md", 1),
                ("repo_z", "src/main.rs", 1),
            ],
            "local search results should be stable by repo id, path, then line number even when file and directory names share a prefix"
        );

        fs::remove_dir_all(root_a).unwrap();
        fs::remove_dir_all(root_b).unwrap();
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
    fn local_search_store_skips_sourcebot_runtime_artifacts() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "search-skip-sourcebot-artifacts",
            "build_router is documented here\n",
            "fn build_router() {}\n",
            "target/generated.txt",
        );
        let artifact_dir = fixture.root.join(".sourcebot");
        fs::create_dir_all(&artifact_dir).unwrap();
        fs::write(
            artifact_dir.join("local-sync-index.json"),
            "build_router should never leak from runtime metadata\n",
        )
        .unwrap();

        let store = LocalSearchStore::new(HashMap::from([(
            "repo_test".to_string(),
            fixture.root.clone(),
        )]));
        let response = store.search("runtime metadata", Some("repo_test")).unwrap();
        assert!(
            response.results.is_empty(),
            "search must not expose local_sync runtime artifacts as repository content"
        );
        let status = store.repository_index_status("repo_test").unwrap().unwrap();
        assert_eq!(status.indexed_file_count, 2);
        assert_eq!(status.indexed_line_count, 2);
        assert_eq!(status.skipped_file_count, 2);

        fs::remove_dir_all(fixture.root).unwrap();
    }

    #[test]
    fn local_search_store_returns_empty_results_for_unknown_repo() {
        let (store, root) = create_test_store();

        let response = store.search("build_router", Some("missing_repo")).unwrap();

        assert!(response.results.is_empty());
        assert_eq!(response.repo_id.as_deref(), Some("missing_repo"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_reports_truthful_index_status_counts() {
        let (store, root) = create_test_store();

        let status = store.repository_index_status("repo_test").unwrap().unwrap();

        assert_eq!(status.repo_id, "repo_test");
        assert_eq!(status.status, RepositoryIndexState::Indexed);
        assert_eq!(status.indexed_file_count, 2);
        assert_eq!(status.indexed_line_count, 3);
        assert_eq!(status.skipped_file_count, 4);
        assert_eq!(status.error, None);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_distinguishes_indexed_empty_repositories() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "search-empty-status",
            "",
            "",
            "target/generated.txt",
        );
        fs::remove_file(fixture.root.join("README.md")).unwrap();
        fs::remove_dir_all(fixture.root.join("src")).unwrap();

        let store = LocalSearchStore::new(HashMap::from([(
            "repo_empty".to_string(),
            fixture.root.clone(),
        )]));
        let status = store
            .repository_index_status("repo_empty")
            .unwrap()
            .unwrap();

        assert_eq!(status.repo_id, "repo_empty");
        assert_eq!(status.status, RepositoryIndexState::IndexedEmpty);
        assert_eq!(status.indexed_file_count, 0);
        assert_eq!(status.indexed_line_count, 0);
        assert_eq!(status.error, None);
        assert!(store
            .search("anything", Some("repo_empty"))
            .unwrap()
            .results
            .is_empty());

        fs::remove_dir_all(fixture.root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn local_search_store_rejects_symlinked_repository_roots() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "search-symlinked-root-target",
            "build_router is documented here\n",
            "fn build_router() {}\n",
            "target/generated.txt",
        );
        let symlinked_root = fixture.root.with_extension("symlink");
        std::os::unix::fs::symlink(&fixture.root, &symlinked_root).unwrap();

        let store = LocalSearchStore::new(HashMap::from([(
            "repo_symlink".to_string(),
            symlinked_root.clone(),
        )]));
        let status = store
            .repository_index_status("repo_symlink")
            .unwrap()
            .unwrap();

        assert_eq!(status.status, RepositoryIndexState::Error);
        assert_eq!(status.indexed_file_count, 0);
        assert_eq!(status.indexed_line_count, 0);
        assert!(
            status
                .error
                .as_deref()
                .is_some_and(|error| error.contains("search repository root is a symlink")),
            "unexpected status: {status:?}"
        );
        assert!(store
            .search("build_router", Some("repo_symlink"))
            .unwrap()
            .results
            .is_empty());

        fs::remove_file(symlinked_root).unwrap();
        fs::remove_dir_all(fixture.root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn local_search_store_rejects_symlinked_repository_roots_with_trailing_separator() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "search-symlinked-root-trailing-target",
            "build_router is documented here\n",
            "fn build_router() {}\n",
            "target/generated.txt",
        );
        let symlinked_root = fixture.root.with_extension("symlink-trailing");
        std::os::unix::fs::symlink(&fixture.root, &symlinked_root).unwrap();
        let symlinked_root_with_separator = PathBuf::from(format!("{}/", symlinked_root.display()));

        let store = LocalSearchStore::new(HashMap::from([(
            "repo_symlink_trailing".to_string(),
            symlinked_root_with_separator,
        )]));
        let status = store
            .repository_index_status("repo_symlink_trailing")
            .unwrap()
            .unwrap();

        assert_eq!(status.status, RepositoryIndexState::Error);
        assert_eq!(status.indexed_file_count, 0);
        assert_eq!(status.indexed_line_count, 0);
        assert!(
            status
                .error
                .as_deref()
                .is_some_and(|error| error.contains("search repository root is a symlink")),
            "unexpected status: {status:?}"
        );
        assert!(store
            .search("build_router", Some("repo_symlink_trailing"))
            .unwrap()
            .results
            .is_empty());

        fs::remove_file(symlinked_root).unwrap();
        fs::remove_dir_all(fixture.root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn local_search_store_rejects_symlinked_repository_roots_with_trailing_dot_component() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "search-symlinked-root-dot-target",
            "build_router is documented here\n",
            "fn build_router() {}\n",
            "target/generated.txt",
        );
        let symlinked_root = fixture.root.with_extension("symlink-dot");
        std::os::unix::fs::symlink(&fixture.root, &symlinked_root).unwrap();
        let symlinked_root_with_dot = symlinked_root.join(".");

        let store = LocalSearchStore::new(HashMap::from([(
            "repo_symlink_dot".to_string(),
            symlinked_root_with_dot,
        )]));
        let status = store
            .repository_index_status("repo_symlink_dot")
            .unwrap()
            .unwrap();

        assert_eq!(status.status, RepositoryIndexState::Error);
        assert_eq!(status.indexed_file_count, 0);
        assert_eq!(status.indexed_line_count, 0);
        assert!(
            status
                .error
                .as_deref()
                .is_some_and(|error| error.contains("search repository root is a symlink")),
            "unexpected status: {status:?}"
        );
        assert!(store
            .search("build_router", Some("repo_symlink_dot"))
            .unwrap()
            .results
            .is_empty());

        fs::remove_file(symlinked_root).unwrap();
        fs::remove_dir_all(fixture.root).unwrap();
    }

    #[test]
    fn local_search_store_can_round_trip_persisted_index_artifact_without_rewalking_snapshot() {
        let (store, root) = create_test_store();
        let artifact_path = root.join(".sourcebot").join("local-sync-index.json");

        store
            .write_index_artifact("repo_test", &artifact_path)
            .unwrap();
        fs::write(root.join("README.md"), "new_marker_after_artifact\n").unwrap();

        let artifact_store =
            LocalSearchStore::from_index_artifact("repo_test", &artifact_path).unwrap();

        let original_response = artifact_store
            .search("build_router", Some("repo_test"))
            .unwrap();
        assert!(original_response
            .results
            .iter()
            .any(|result| result.path == "README.md"
                && result.line == "build_router is documented here"));
        let post_artifact_response = artifact_store
            .search("new_marker_after_artifact", Some("repo_test"))
            .unwrap();
        assert!(post_artifact_response.results.is_empty());
        let status = artifact_store
            .repository_index_status("repo_test")
            .unwrap()
            .unwrap();
        assert_eq!(status.indexed_file_count, 2);
        assert_eq!(status.indexed_line_count, 3);
        assert_eq!(status.skipped_file_count, 4);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_writes_persisted_index_artifact_in_validated_order() {
        let root = repo_tree_fixture::unique_temp_dir("search-artifact-sorted-writer");
        fs::create_dir_all(root.join("a")).unwrap();
        fs::write(root.join("a").join("file.rs"), "nested marker\n").unwrap();
        fs::write(root.join("a.rs"), "sibling marker\n").unwrap();
        let artifact_path = root.join(".sourcebot").join("local-sync-index.json");
        let store = LocalSearchStore::new(HashMap::from([("repo_test".to_string(), root.clone())]));

        store
            .write_index_artifact("repo_test", &artifact_path)
            .unwrap();
        let artifact_store =
            LocalSearchStore::from_index_artifact("repo_test", &artifact_path).unwrap();

        let results = artifact_store
            .search("marker", Some("repo_test"))
            .unwrap()
            .results;
        assert_eq!(
            results
                .iter()
                .map(|result| result.path.as_str())
                .collect::<Vec<_>>(),
            vec!["a.rs", "a/file.rs"]
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn local_search_store_rejects_symlinked_persisted_index_artifacts() {
        let (store, root) = create_test_store();
        let real_artifact_path = root.join(".sourcebot").join("real-local-sync-index.json");
        let symlink_artifact_path = root.join(".sourcebot").join("local-sync-index.json");

        store
            .write_index_artifact("repo_test", &real_artifact_path)
            .unwrap();
        std::os::unix::fs::symlink(&real_artifact_path, &symlink_artifact_path).unwrap();

        let error = match LocalSearchStore::from_index_artifact("repo_test", &symlink_artifact_path)
        {
            Ok(_) => panic!("symlinked persisted index artifact must fail closed"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("search index artifact is not a regular file"),
            "unexpected error: {error:#}"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn local_search_store_rejects_symlinked_persisted_index_artifact_parent() {
        let (store, root) = create_test_store();
        let real_artifact_path = root.join("real-sourcebot").join("local-sync-index.json");
        let symlinked_parent = root.join(".sourcebot");
        let symlinked_artifact_path = symlinked_parent.join("local-sync-index.json");

        store
            .write_index_artifact("repo_test", &real_artifact_path)
            .unwrap();
        std::os::unix::fs::symlink(real_artifact_path.parent().unwrap(), &symlinked_parent)
            .unwrap();

        let error =
            match LocalSearchStore::from_index_artifact("repo_test", &symlinked_artifact_path) {
                Ok(_) => panic!("persisted index artifact under symlinked parent must fail closed"),
                Err(error) => error,
            };
        assert!(
            error
                .to_string()
                .contains("search index artifact parent contains a symlink"),
            "unexpected error: {error:#}"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn local_search_store_write_rejects_symlinked_temp_artifacts_without_clobbering_target() {
        let (store, root) = create_test_store();
        let artifact_path = root.join(".sourcebot").join("local-sync-index.json");
        let tmp_path = artifact_path.with_extension("json.tmp");
        let external_target = root.join("external-target.json");
        fs::create_dir_all(tmp_path.parent().unwrap()).unwrap();
        fs::write(&external_target, "must not be overwritten").unwrap();
        std::os::unix::fs::symlink(&external_target, &tmp_path).unwrap();

        let error = match store.write_index_artifact("repo_test", &artifact_path) {
            Ok(_) => panic!("persisted index writer must fail closed on symlinked temp artifact"),
            Err(error) => error,
        };

        assert!(
            error
                .to_string()
                .contains("search index temp artifact already exists"),
            "unexpected error: {error:#}"
        );
        assert_eq!(
            fs::read_to_string(&external_target).unwrap(),
            "must not be overwritten"
        );
        assert!(!artifact_path.exists());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_rejects_persisted_index_artifact_paths_that_escape_repo() {
        let (store, root) = create_test_store();
        let artifact_path = root.join(".sourcebot").join("local-sync-index.json");

        store
            .write_index_artifact("repo_test", &artifact_path)
            .unwrap();
        let artifact_json: serde_json::Value =
            serde_json::from_slice(&fs::read(&artifact_path).unwrap()).unwrap();

        for unsafe_path in ["../secret.rs", "/tmp/secret.rs", "src\\secret.rs"] {
            let mut unsafe_artifact_json = artifact_json.clone();
            unsafe_artifact_json["indexed_lines"][0]["path"] =
                serde_json::Value::String(unsafe_path.to_string());
            fs::write(
                &artifact_path,
                serde_json::to_vec(&unsafe_artifact_json).unwrap(),
            )
            .unwrap();

            let error = match LocalSearchStore::from_index_artifact("repo_test", &artifact_path) {
                Ok(_) => panic!("persisted index path {unsafe_path:?} must fail closed"),
                Err(error) => error,
            };
            assert!(
                error
                    .to_string()
                    .contains("unsafe search index artifact path"),
                "unexpected error for {unsafe_path:?}: {error:#}"
            );
        }

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_rejects_persisted_index_artifact_dot_component_paths() {
        let (store, root) = create_test_store();
        let artifact_path = root.join(".sourcebot").join("local-sync-index.json");

        store
            .write_index_artifact("repo_test", &artifact_path)
            .unwrap();
        let mut artifact_json: serde_json::Value =
            serde_json::from_slice(&fs::read(&artifact_path).unwrap()).unwrap();
        artifact_json["indexed_lines"][0]["path"] =
            serde_json::Value::String("src/./main.rs".to_string());
        fs::write(&artifact_path, serde_json::to_vec(&artifact_json).unwrap()).unwrap();

        let error = match LocalSearchStore::from_index_artifact("repo_test", &artifact_path) {
            Ok(_) => panic!("persisted index dot-component paths must fail closed"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("unsafe search index artifact path"),
            "unexpected error: {error:#}"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_rejects_persisted_index_artifact_blank_path_components() {
        let (store, root) = create_test_store();
        let artifact_path = root.join(".sourcebot").join("local-sync-index.json");

        store
            .write_index_artifact("repo_test", &artifact_path)
            .unwrap();
        let artifact_json: serde_json::Value =
            serde_json::from_slice(&fs::read(&artifact_path).unwrap()).unwrap();

        for unsafe_path in ["src//main.rs", "src/main.rs/"] {
            let mut unsafe_artifact_json = artifact_json.clone();
            unsafe_artifact_json["indexed_lines"][0]["path"] =
                serde_json::Value::String(unsafe_path.to_string());
            fs::write(
                &artifact_path,
                serde_json::to_vec(&unsafe_artifact_json).unwrap(),
            )
            .unwrap();

            let error = match LocalSearchStore::from_index_artifact("repo_test", &artifact_path) {
                Ok(_) => panic!(
                    "persisted index path with blank component {unsafe_path:?} must fail closed"
                ),
                Err(error) => error,
            };
            assert!(
                error
                    .to_string()
                    .contains("unsafe search index artifact path"),
                "unexpected error for {unsafe_path:?}: {error:#}"
            );
        }

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_rejects_persisted_index_artifact_zero_line_numbers() {
        let (store, root) = create_test_store();
        let artifact_path = root.join(".sourcebot").join("local-sync-index.json");

        store
            .write_index_artifact("repo_test", &artifact_path)
            .unwrap();
        let mut artifact_json: serde_json::Value =
            serde_json::from_slice(&fs::read(&artifact_path).unwrap()).unwrap();
        artifact_json["indexed_lines"][0]["line_number"] = serde_json::Value::from(0);
        fs::write(&artifact_path, serde_json::to_vec(&artifact_json).unwrap()).unwrap();

        let error = match LocalSearchStore::from_index_artifact("repo_test", &artifact_path) {
            Ok(_) => panic!("persisted index zero line numbers must fail closed"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("invalid search index artifact line number"),
            "unexpected error: {error:#}"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_rejects_persisted_index_artifact_line_numbers_beyond_indexed_line_count()
    {
        let (store, root) = create_test_store();
        let artifact_path = root.join(".sourcebot").join("local-sync-index.json");

        store
            .write_index_artifact("repo_test", &artifact_path)
            .unwrap();
        let mut artifact_json: serde_json::Value =
            serde_json::from_slice(&fs::read(&artifact_path).unwrap()).unwrap();
        artifact_json["indexed_lines"][0]["line_number"] = serde_json::Value::from(999);
        fs::write(&artifact_path, serde_json::to_vec(&artifact_json).unwrap()).unwrap();

        let error = match LocalSearchStore::from_index_artifact("repo_test", &artifact_path) {
            Ok(_) => {
                panic!("persisted index line numbers beyond indexed line count must fail closed")
            }
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("search index artifact line number exceeds indexed line count"),
            "unexpected error: {error:#}"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_rejects_persisted_index_artifact_non_contiguous_line_numbers() {
        let (store, root) = create_test_store();
        let artifact_path = root.join(".sourcebot").join("local-sync-index.json");

        store
            .write_index_artifact("repo_test", &artifact_path)
            .unwrap();
        let mut artifact_json: serde_json::Value =
            serde_json::from_slice(&fs::read(&artifact_path).unwrap()).unwrap();
        artifact_json["indexed_lines"][2]["line_number"] = serde_json::Value::from(3);
        fs::write(&artifact_path, serde_json::to_vec(&artifact_json).unwrap()).unwrap();

        let error = match LocalSearchStore::from_index_artifact("repo_test", &artifact_path) {
            Ok(_) => {
                panic!("persisted index artifacts with missing file line entries must fail closed")
            }
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("non-contiguous search index artifact line key"),
            "unexpected error: {error:#}"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_rejects_persisted_index_artifact_duplicate_line_keys() {
        let (store, root) = create_test_store();
        let artifact_path = root.join(".sourcebot").join("local-sync-index.json");

        store
            .write_index_artifact("repo_test", &artifact_path)
            .unwrap();
        let mut artifact_json: serde_json::Value =
            serde_json::from_slice(&fs::read(&artifact_path).unwrap()).unwrap();
        let duplicated_line = artifact_json["indexed_lines"][0].clone();
        artifact_json["indexed_lines"]
            .as_array_mut()
            .unwrap()
            .push(duplicated_line);
        artifact_json["index_status"]["indexed_line_count"] = serde_json::Value::from(4);
        fs::write(&artifact_path, serde_json::to_vec(&artifact_json).unwrap()).unwrap();

        let error = match LocalSearchStore::from_index_artifact("repo_test", &artifact_path) {
            Ok(_) => panic!("persisted index duplicate path/line entries must fail closed"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("duplicate search index artifact line key"),
            "unexpected error: {error:#}"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_rejects_persisted_index_artifact_unsorted_line_keys() {
        let (store, root) = create_test_store();
        let artifact_path = root.join(".sourcebot").join("local-sync-index.json");

        store
            .write_index_artifact("repo_test", &artifact_path)
            .unwrap();
        let mut artifact_json: serde_json::Value =
            serde_json::from_slice(&fs::read(&artifact_path).unwrap()).unwrap();
        artifact_json["indexed_lines"]
            .as_array_mut()
            .unwrap()
            .swap(0, 1);
        fs::write(&artifact_path, serde_json::to_vec(&artifact_json).unwrap()).unwrap();

        let error = match LocalSearchStore::from_index_artifact("repo_test", &artifact_path) {
            Ok(_) => panic!("persisted index out-of-order path/line entries must fail closed"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("unsorted search index artifact line key"),
            "unexpected error: {error:#}"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_rejects_persisted_index_artifact_unknown_schema_fields() {
        let (store, root) = create_test_store();
        let artifact_path = root.join(".sourcebot").join("local-sync-index.json");

        store
            .write_index_artifact("repo_test", &artifact_path)
            .unwrap();
        let artifact_json: serde_json::Value =
            serde_json::from_slice(&fs::read(&artifact_path).unwrap()).unwrap();

        let cases: [(&str, fn(&mut serde_json::Value)); 3] = [
            ("top-level", |artifact_json| {
                artifact_json["unexpected_schema_field"] = serde_json::Value::Bool(true);
            }),
            ("indexed-line", |artifact_json| {
                artifact_json["indexed_lines"][0]["unexpected_schema_field"] =
                    serde_json::Value::Bool(true);
            }),
            ("index-status", |artifact_json| {
                artifact_json["index_status"]["unexpected_schema_field"] =
                    serde_json::Value::Bool(true);
            }),
        ];
        for (label, mutate) in cases {
            let mut unknown_field_artifact_json = artifact_json.clone();
            mutate(&mut unknown_field_artifact_json);
            fs::write(
                &artifact_path,
                serde_json::to_vec(&unknown_field_artifact_json).unwrap(),
            )
            .unwrap();

            let error = match LocalSearchStore::from_index_artifact("repo_test", &artifact_path) {
                Ok(_) => {
                    panic!("persisted index artifact with unknown {label} field must fail closed")
                }
                Err(error) => error,
            };
            assert!(
                error
                    .to_string()
                    .contains("failed to parse local search index artifact"),
                "unexpected error for {label} unknown field: {error:#}"
            );
        }

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_rejects_persisted_index_artifact_repo_id_mismatch() {
        let (store, root) = create_test_store();
        let artifact_path = root.join(".sourcebot").join("local-sync-index.json");

        store
            .write_index_artifact("repo_test", &artifact_path)
            .unwrap();
        let mut artifact_json: serde_json::Value =
            serde_json::from_slice(&fs::read(&artifact_path).unwrap()).unwrap();
        artifact_json["index_status"]["repo_id"] =
            serde_json::Value::String("repo_other".to_string());
        fs::write(&artifact_path, serde_json::to_vec(&artifact_json).unwrap()).unwrap();

        let error = match LocalSearchStore::from_index_artifact("repo_test", &artifact_path) {
            Ok(_) => panic!("persisted index repo_id mismatch must fail closed"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("search index artifact repo_id mismatch"),
            "unexpected error: {error:#}"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_rejects_persisted_index_artifact_status_count_mismatch() {
        let (store, root) = create_test_store();
        let artifact_path = root.join(".sourcebot").join("local-sync-index.json");

        store
            .write_index_artifact("repo_test", &artifact_path)
            .unwrap();
        let mut artifact_json: serde_json::Value =
            serde_json::from_slice(&fs::read(&artifact_path).unwrap()).unwrap();
        artifact_json["index_status"]["indexed_line_count"] = serde_json::Value::from(999);
        fs::write(&artifact_path, serde_json::to_vec(&artifact_json).unwrap()).unwrap();

        let error = match LocalSearchStore::from_index_artifact("repo_test", &artifact_path) {
            Ok(_) => panic!("persisted index status count mismatch must fail closed"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("search index artifact status count mismatch"),
            "unexpected error: {error:#}"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_rejects_persisted_index_artifact_indexed_file_undercount() {
        let (store, root) = create_test_store();
        let artifact_path = root.join(".sourcebot").join("local-sync-index.json");

        store
            .write_index_artifact("repo_test", &artifact_path)
            .unwrap();
        let mut artifact_json: serde_json::Value =
            serde_json::from_slice(&fs::read(&artifact_path).unwrap()).unwrap();
        artifact_json["index_status"]["indexed_file_count"] = serde_json::Value::from(1);
        fs::write(&artifact_path, serde_json::to_vec(&artifact_json).unwrap()).unwrap();

        let error = match LocalSearchStore::from_index_artifact("repo_test", &artifact_path) {
            Ok(_) => panic!("persisted index status file undercount must fail closed"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("search index artifact status file count undercounts stored paths"),
            "unexpected error: {error:#}"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_rejects_persisted_index_artifact_status_state_mismatch() {
        let (store, root) = create_test_store();
        let artifact_path = root.join(".sourcebot").join("local-sync-index.json");

        store
            .write_index_artifact("repo_test", &artifact_path)
            .unwrap();
        let mut artifact_json: serde_json::Value =
            serde_json::from_slice(&fs::read(&artifact_path).unwrap()).unwrap();
        artifact_json["index_status"]["status"] =
            serde_json::Value::String("indexed_empty".to_string());
        fs::write(&artifact_path, serde_json::to_vec(&artifact_json).unwrap()).unwrap();

        let error = match LocalSearchStore::from_index_artifact("repo_test", &artifact_path) {
            Ok(_) => panic!("persisted index status state mismatch must fail closed"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("search index artifact status state mismatch"),
            "unexpected error: {error:#}"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_rejects_persisted_index_artifact_indexed_status_with_no_indexed_files() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "search-indexed-empty-status-artifact",
            "",
            "",
            "target/generated.txt",
        );
        fs::remove_file(fixture.root.join("README.md")).unwrap();
        fs::remove_dir_all(fixture.root.join("src")).unwrap();
        let store = LocalSearchStore::new(HashMap::from([(
            "repo_empty".to_string(),
            fixture.root.clone(),
        )]));
        let artifact_path = fixture
            .root
            .join(".sourcebot")
            .join("local-sync-index.json");

        store
            .write_index_artifact("repo_empty", &artifact_path)
            .unwrap();
        let mut artifact_json: serde_json::Value =
            serde_json::from_slice(&fs::read(&artifact_path).unwrap()).unwrap();
        artifact_json["index_status"]["status"] = serde_json::Value::String("indexed".to_string());
        fs::write(&artifact_path, serde_json::to_vec(&artifact_json).unwrap()).unwrap();

        let error = match LocalSearchStore::from_index_artifact("repo_empty", &artifact_path) {
            Ok(_) => panic!("indexed-status artifact with no indexed files must fail closed"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("search index artifact indexed status has no indexed files"),
            "unexpected error: {error:#}"
        );

        fs::remove_dir_all(fixture.root).unwrap();
    }

    #[test]
    fn local_search_store_rejects_error_status_artifact_that_carries_indexed_content() {
        let (store, root) = create_test_store();
        let artifact_path = root.join(".sourcebot").join("local-sync-index.json");

        store
            .write_index_artifact("repo_test", &artifact_path)
            .unwrap();
        let mut artifact_json: serde_json::Value =
            serde_json::from_slice(&fs::read(&artifact_path).unwrap()).unwrap();
        artifact_json["index_status"]["status"] = serde_json::Value::String("error".to_string());
        artifact_json["index_status"]["error"] =
            serde_json::Value::String("synthetic artifact load failure".to_string());
        fs::write(&artifact_path, serde_json::to_vec(&artifact_json).unwrap()).unwrap();

        let error = match LocalSearchStore::from_index_artifact("repo_test", &artifact_path) {
            Ok(_) => panic!("error-status artifact with indexed content must fail closed"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("search index artifact error status carries indexed content"),
            "unexpected error: {error:#}"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_rejects_error_status_artifact_without_operator_visible_error() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "search-error-status-without-error-artifact",
            "",
            "",
            "target/generated.txt",
        );
        fs::remove_file(fixture.root.join("README.md")).unwrap();
        fs::remove_dir_all(fixture.root.join("src")).unwrap();
        let store = LocalSearchStore::new(HashMap::from([(
            "repo_empty".to_string(),
            fixture.root.clone(),
        )]));
        let artifact_path = fixture
            .root
            .join(".sourcebot")
            .join("local-sync-index.json");

        store
            .write_index_artifact("repo_empty", &artifact_path)
            .unwrap();
        let mut artifact_json: serde_json::Value =
            serde_json::from_slice(&fs::read(&artifact_path).unwrap()).unwrap();
        artifact_json["index_status"]["status"] = serde_json::Value::String("error".to_string());
        artifact_json["index_status"]["error"] = serde_json::Value::Null;
        fs::write(&artifact_path, serde_json::to_vec(&artifact_json).unwrap()).unwrap();

        let error = match LocalSearchStore::from_index_artifact("repo_empty", &artifact_path) {
            Ok(_) => panic!("error-status artifact without an error message must fail closed"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains(
                "search index artifact error status is missing an operator-visible error"
            ),
            "unexpected error: {error:#}"
        );

        fs::remove_dir_all(fixture.root).unwrap();
    }

    #[test]
    fn local_search_store_rejects_success_status_artifact_with_stale_error_message() {
        let (store, root) = create_test_store();
        let artifact_path = root.join(".sourcebot").join("local-sync-index.json");

        store
            .write_index_artifact("repo_test", &artifact_path)
            .unwrap();
        let mut artifact_json: serde_json::Value =
            serde_json::from_slice(&fs::read(&artifact_path).unwrap()).unwrap();
        artifact_json["index_status"]["error"] =
            serde_json::Value::String("stale previous indexing failure".to_string());
        fs::write(&artifact_path, serde_json::to_vec(&artifact_json).unwrap()).unwrap();

        let error = match LocalSearchStore::from_index_artifact("repo_test", &artifact_path) {
            Ok(_) => panic!("success-status artifact with a stale error message must fail closed"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("search index artifact success status carries an error"),
            "unexpected error: {error:#}"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_search_store_rejects_oversized_persisted_index_artifacts_before_parsing() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "search-oversized-artifact",
            "build_router is documented here\n",
            "fn build_router() {}\n",
            "target/generated.txt",
        );
        let artifact_path = fixture
            .root
            .join(".sourcebot")
            .join("local-sync-index.json");
        fs::create_dir_all(artifact_path.parent().unwrap()).unwrap();
        let oversized_bytes = vec![b' '; MAX_INDEX_ARTIFACT_SIZE_BYTES as usize + 1];
        fs::write(&artifact_path, oversized_bytes).unwrap();

        let error = match LocalSearchStore::from_index_artifact("repo_test", &artifact_path) {
            Ok(_) => panic!("oversized persisted index artifact must fail closed"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("search index artifact is too large"),
            "unexpected error: {error:#}"
        );

        fs::remove_dir_all(fixture.root).unwrap();
    }

    #[test]
    fn local_search_store_rejects_non_regular_persisted_index_artifacts_before_reading() {
        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "search-non-regular-artifact",
            "build_router is documented here\n",
            "fn build_router() {}\n",
            "target/generated.txt",
        );
        let artifact_path = fixture
            .root
            .join(".sourcebot")
            .join("local-sync-index.json");
        fs::create_dir_all(&artifact_path).unwrap();

        let error = match LocalSearchStore::from_index_artifact("repo_test", &artifact_path) {
            Ok(_) => panic!("non-regular persisted index artifact must fail closed"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("search index artifact is not a regular file"),
            "unexpected error: {error:#}"
        );

        fs::remove_dir_all(fixture.root).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn local_search_store_rejects_fifo_index_artifacts_without_blocking() {
        use std::{sync::mpsc, time::Duration};

        let fixture = repo_tree_fixture::CanonicalRepoTreeRoot::create(
            "search-fifo-artifact",
            "build_router is documented here\n",
            "fn build_router() {}\n",
            "target/generated.txt",
        );
        let artifact_path = fixture
            .root
            .join(".sourcebot")
            .join("local-sync-index.json");
        fs::create_dir_all(artifact_path.parent().unwrap()).unwrap();
        let status = std::process::Command::new("mkfifo")
            .arg(&artifact_path)
            .status()
            .unwrap();
        assert!(status.success());

        let artifact_path_for_thread = artifact_path.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let error =
                match LocalSearchStore::from_index_artifact("repo_test", &artifact_path_for_thread)
                {
                    Ok(_) => panic!("FIFO persisted index artifact must fail closed"),
                    Err(error) => error,
                };
            tx.send(error.to_string()).unwrap();
        });

        let error = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("FIFO artifact rejection must not block opening the artifact");
        assert!(
            error.contains("search index artifact is not a regular file"),
            "unexpected error: {error}"
        );

        fs::remove_dir_all(fixture.root).unwrap();
    }

    #[test]
    fn extract_symbols_returns_top_level_rust_definitions() {
        let extraction = extract_symbols(
            "src/lib.rs",
            r#"pub fn top_level_function() {
    let _value = 1;
}

pub async fn fetch_widget() {
    let _value = 2;
}

struct Widget {
    name: String,
}

enum Mode {
    Fast,
}

trait Runner {
    fn run(&self);
}

impl Widget {
    fn helper(&self) {}
}

    fn nested_like_indent() {}
"#,
        );

        assert_eq!(
            extraction,
            SymbolExtraction::Supported {
                symbols: vec![
                    SymbolDefinition {
                        path: "src/lib.rs".to_string(),
                        name: "top_level_function".to_string(),
                        kind: SymbolKind::Function,
                        range: SymbolRange {
                            start_line: 1,
                            end_line: 3,
                        },
                    },
                    SymbolDefinition {
                        path: "src/lib.rs".to_string(),
                        name: "fetch_widget".to_string(),
                        kind: SymbolKind::Function,
                        range: SymbolRange {
                            start_line: 5,
                            end_line: 7,
                        },
                    },
                    SymbolDefinition {
                        path: "src/lib.rs".to_string(),
                        name: "Widget".to_string(),
                        kind: SymbolKind::Struct,
                        range: SymbolRange {
                            start_line: 9,
                            end_line: 11,
                        },
                    },
                    SymbolDefinition {
                        path: "src/lib.rs".to_string(),
                        name: "Mode".to_string(),
                        kind: SymbolKind::Enum,
                        range: SymbolRange {
                            start_line: 13,
                            end_line: 15,
                        },
                    },
                    SymbolDefinition {
                        path: "src/lib.rs".to_string(),
                        name: "Runner".to_string(),
                        kind: SymbolKind::Trait,
                        range: SymbolRange {
                            start_line: 17,
                            end_line: 19,
                        },
                    },
                ],
            }
        );
    }

    #[test]
    fn extract_symbols_returns_top_level_typescript_and_javascript_definitions() {
        let extraction = extract_symbols(
            "src/App.tsx",
            r#"import React from "react";

export function App() {
  return <main />;
}

export class WidgetPanel {
  render() {
    return null;
  }
}

export interface WidgetProps {
  name: string;
}

export type WidgetMode = "fast" | "safe";

export const useWidget = () => {
  return "widget";
};

const LOCAL_CONSTANT = "value";

  function nestedIndented() {}
"#,
        );

        assert_eq!(
            extraction,
            SymbolExtraction::Supported {
                symbols: vec![
                    SymbolDefinition {
                        path: "src/App.tsx".to_string(),
                        name: "App".to_string(),
                        kind: SymbolKind::Function,
                        range: SymbolRange {
                            start_line: 3,
                            end_line: 5,
                        },
                    },
                    SymbolDefinition {
                        path: "src/App.tsx".to_string(),
                        name: "WidgetPanel".to_string(),
                        kind: SymbolKind::Class,
                        range: SymbolRange {
                            start_line: 7,
                            end_line: 11,
                        },
                    },
                    SymbolDefinition {
                        path: "src/App.tsx".to_string(),
                        name: "WidgetProps".to_string(),
                        kind: SymbolKind::Interface,
                        range: SymbolRange {
                            start_line: 13,
                            end_line: 15,
                        },
                    },
                    SymbolDefinition {
                        path: "src/App.tsx".to_string(),
                        name: "WidgetMode".to_string(),
                        kind: SymbolKind::TypeAlias,
                        range: SymbolRange {
                            start_line: 17,
                            end_line: 17,
                        },
                    },
                    SymbolDefinition {
                        path: "src/App.tsx".to_string(),
                        name: "useWidget".to_string(),
                        kind: SymbolKind::Function,
                        range: SymbolRange {
                            start_line: 19,
                            end_line: 21,
                        },
                    },
                    SymbolDefinition {
                        path: "src/App.tsx".to_string(),
                        name: "LOCAL_CONSTANT".to_string(),
                        kind: SymbolKind::Constant,
                        range: SymbolRange {
                            start_line: 23,
                            end_line: 23,
                        },
                    },
                ],
            }
        );

        let js_extraction = extract_symbols(
            "src/util.js",
            "export default function createUtil() {\\n  return true;\\n}\\n",
        );
        match js_extraction {
            SymbolExtraction::Supported { symbols } => {
                assert_eq!(symbols.len(), 1);
                assert_eq!(symbols[0].name, "createUtil");
                assert_eq!(symbols[0].kind, SymbolKind::Function);
            }
            SymbolExtraction::Unsupported { .. } => {
                panic!("expected JavaScript symbol extraction to be supported")
            }
        }
    }

    #[test]
    fn extract_symbols_returns_capability_response_for_unsupported_extensions() {
        let extraction = extract_symbols("docs/readme.md", "# Heading\n");

        assert_eq!(
            extraction,
            SymbolExtraction::Unsupported {
                capability: "symbol extraction is not supported for .md files".to_string(),
                symbols: Vec::new(),
            }
        );
    }
}
