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
    fn repository_index_status(&self, repo_id: &str) -> Result<Option<RepositoryIndexStatus>>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryIndexState {
    Indexed,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
pub struct SearchResponse {
    pub query: String,
    pub repo_id: Option<String>,
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct IndexedLine {
    path: String,
    line_number: usize,
    line: String,
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
                            status: RepositoryIndexState::Indexed,
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
        Ok((
            lines,
            indexed_file_count,
            indexed_line_count,
            skipped_file_count,
        ))
    }

    fn search_repo(&self, repo_id: &str, query: &str) -> Vec<SearchResult> {
        self.indexed_lines
            .get(repo_id)
            .into_iter()
            .flatten()
            .filter(|indexed_line| indexed_line.line.contains(query))
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

            if should_skip_file(&path) {
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

            let relative_path = path
                .strip_prefix(root)
                .with_context(|| format!("failed to strip prefix for {}", path.display()))?
                .to_string_lossy()
                .replace('\\', "/");

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

impl SearchStore for LocalSearchStore {
    fn search(&self, query: &str, repo_id: Option<&str>) -> Result<SearchResponse> {
        let query = query.trim();
        let requested_repo_id = repo_id.map(str::trim).filter(|value| !value.is_empty());

        let repos: Vec<&String> = match requested_repo_id {
            Some(repo_id) => self
                .repo_roots
                .get_key_value(repo_id)
                .map(|(repo_id, _)| repo_id)
                .into_iter()
                .collect(),
            None => self.repo_roots.keys().collect(),
        };

        let mut results = Vec::new();
        for repo_id in repos {
            results.extend(self.search_repo(repo_id, query));
        }

        Ok(SearchResponse {
            query: query.to_string(),
            repo_id: requested_repo_id.map(ToOwned::to_owned),
            results,
        })
    }

    fn repository_index_status(&self, repo_id: &str) -> Result<Option<RepositoryIndexStatus>> {
        Ok(self.index_statuses.get(repo_id).cloned())
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

pub fn extract_symbols(path: &str, content: &str) -> SymbolExtraction {
    let extension = Path::new(path).extension().and_then(|value| value.to_str());

    match extension {
        Some("rs") => SymbolExtraction::Supported {
            symbols: extract_rust_symbols(path, content),
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
                end_line: find_rust_symbol_end_line(&lines, index),
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

fn find_rust_symbol_end_line(lines: &[&str], start_index: usize) -> usize {
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
