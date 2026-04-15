use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::{
    collections::HashMap,
    path::PathBuf,
    process::{Command, Output},
    sync::Arc,
};

pub type DynCommitStore = Arc<dyn CommitStore>;

const SOURCEBOT_REWRITE_REPO_ID: &str = "repo_sourcebot_rewrite";
const SOURCEBOT_REWRITE_ROOT: &str = "/opt/data/projects/sourcebot-rewrite";
const EMPTY_HISTORY_REPO_IDS: &[&str] = &["repo_demo_docs"];
const FIELD_SEPARATOR: char = '\u{1f}';
const RECORD_SEPARATOR: char = '\u{1e}';

pub trait CommitStore: Send + Sync {
    fn list_commits(&self, repo_id: &str, limit: usize) -> Result<Option<CommitListResponse>>;
    fn get_commit(&self, repo_id: &str, commit_id: &str) -> Result<Option<CommitDetailResponse>>;
    fn get_commit_diff(&self, repo_id: &str, commit_id: &str)
        -> Result<Option<CommitDiffResponse>>;
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CommitSummary {
    pub id: String,
    pub short_id: String,
    pub summary: String,
    pub author_name: String,
    pub authored_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CommitDetail {
    pub id: String,
    pub short_id: String,
    pub summary: String,
    pub author_name: String,
    pub authored_at: String,
    pub body: String,
    pub parents: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CommitListResponse {
    pub repo_id: String,
    pub commits: Vec<CommitSummary>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CommitDetailResponse {
    pub repo_id: String,
    pub commit: CommitDetail,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommitDiffChangeType {
    Added,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CommitDiffFile {
    pub path: String,
    pub change_type: CommitDiffChangeType,
    pub old_path: Option<String>,
    pub additions: usize,
    pub deletions: usize,
    pub patch: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CommitDiffResponse {
    pub repo_id: String,
    pub commit_id: String,
    pub files: Vec<CommitDiffFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RawDiffStatus {
    path: String,
    change_type: CommitDiffChangeType,
    old_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RawNumstat {
    path: String,
    old_path: Option<String>,
    additions: usize,
    deletions: usize,
}

#[derive(Clone, Default)]
pub struct LocalCommitStore {
    repo_roots: HashMap<String, PathBuf>,
}

impl LocalCommitStore {
    pub fn new(repo_roots: HashMap<String, PathBuf>) -> Self {
        Self { repo_roots }
    }

    pub fn seeded() -> Self {
        Self::new(HashMap::from([(
            SOURCEBOT_REWRITE_REPO_ID.to_string(),
            PathBuf::from(SOURCEBOT_REWRITE_ROOT),
        )]))
    }

    fn repo_root(&self, repo_id: &str) -> Option<&PathBuf> {
        self.repo_roots.get(repo_id)
    }

    fn supports_empty_history(&self, repo_id: &str) -> bool {
        EMPTY_HISTORY_REPO_IDS.contains(&repo_id)
    }

    fn parse_summary_record(&self, record: &str) -> Result<CommitSummary> {
        let parts = split_record(record, 5)?;
        Ok(CommitSummary {
            id: parts[0].to_string(),
            short_id: parts[1].to_string(),
            summary: parts[2].to_string(),
            author_name: parts[3].to_string(),
            authored_at: parts[4].to_string(),
        })
    }

    fn parse_detail_record(&self, record: &str) -> Result<CommitDetail> {
        let parts = split_record(record, 7)?;
        Ok(CommitDetail {
            id: parts[0].to_string(),
            short_id: parts[1].to_string(),
            summary: parts[2].to_string(),
            author_name: parts[3].to_string(),
            authored_at: parts[4].to_string(),
            body: parts[5].trim_end_matches('\n').to_string(),
            parents: parts[6]
                .split_whitespace()
                .filter(|parent| !parent.is_empty())
                .map(ToString::to_string)
                .collect(),
        })
    }
}

impl CommitStore for LocalCommitStore {
    fn list_commits(&self, repo_id: &str, limit: usize) -> Result<Option<CommitListResponse>> {
        let Some(repo_root) = self.repo_root(repo_id) else {
            if self.supports_empty_history(repo_id) {
                return Ok(Some(CommitListResponse {
                    repo_id: repo_id.to_string(),
                    commits: Vec::new(),
                }));
            }
            return Ok(None);
        };

        let output = run_git(
            repo_root,
            &[
                "log",
                &format!("--max-count={}", limit.max(1)),
                &format!("--format=%H%x1f%h%x1f%s%x1f%an%x1f%aI%x1e"),
            ],
        )?;

        let commits = parse_records(&output)
            .into_iter()
            .map(|record| self.parse_summary_record(record))
            .collect::<Result<Vec<_>>>()?;

        Ok(Some(CommitListResponse {
            repo_id: repo_id.to_string(),
            commits,
        }))
    }

    fn get_commit(&self, repo_id: &str, commit_id: &str) -> Result<Option<CommitDetailResponse>> {
        let Some(repo_root) = self.repo_root(repo_id) else {
            return Ok(None);
        };

        let Some(commit_id) = resolve_single_commit(repo_root, commit_id)? else {
            return Ok(None);
        };

        let output = run_git_allow_not_found(
            repo_root,
            &[
                "show",
                "--no-patch",
                &format!("--format=%H%x1f%h%x1f%s%x1f%an%x1f%aI%x1f%b%x1f%P%x1e"),
                &commit_id,
            ],
        )?;

        let Some(output) = output else {
            return Ok(None);
        };

        let Some(record) = parse_records(&output).into_iter().next() else {
            return Ok(None);
        };

        Ok(Some(CommitDetailResponse {
            repo_id: repo_id.to_string(),
            commit: self.parse_detail_record(record)?,
        }))
    }

    fn get_commit_diff(
        &self,
        repo_id: &str,
        commit_id: &str,
    ) -> Result<Option<CommitDiffResponse>> {
        let Some(repo_root) = self.repo_root(repo_id) else {
            return Ok(None);
        };

        let Some(commit_id) = resolve_single_commit(repo_root, commit_id)? else {
            return Ok(None);
        };

        let status_output = run_git_allow_not_found(
            repo_root,
            &[
                "diff-tree",
                "--root",
                "-r",
                "--find-renames",
                "-M",
                "--name-status",
                "-z",
                &commit_id,
            ],
        )?;
        let Some(status_output) = status_output else {
            return Ok(None);
        };

        let numstat_output = run_git_allow_not_found(
            repo_root,
            &[
                "diff-tree",
                "--root",
                "-r",
                "--find-renames",
                "-M",
                "--numstat",
                "-z",
                &commit_id,
            ],
        )?;
        let Some(numstat_output) = numstat_output else {
            return Ok(None);
        };

        let statuses = parse_diff_name_status(&status_output)?;
        let numstats = parse_diff_numstat(&numstat_output)?;
        if statuses.len() != numstats.len() {
            return Err(anyhow!(
                "mismatched git diff metadata for commit {commit_id}: {} status entries vs {} numstat entries",
                statuses.len(),
                numstats.len()
            ));
        }

        let files = statuses
            .into_iter()
            .zip(numstats)
            .map(|(status, numstat)| {
                if status.path != numstat.path || status.old_path != numstat.old_path {
                    return Err(anyhow!(
                        "mismatched git diff entry for commit {commit_id}: {:?} vs {:?}",
                        status,
                        numstat
                    ));
                }

                let patch = load_patch_for_diff_entry(repo_root, &commit_id, &status)?;
                Ok(CommitDiffFile {
                    path: status.path.clone(),
                    change_type: status.change_type,
                    old_path: status.old_path,
                    additions: numstat.additions,
                    deletions: numstat.deletions,
                    patch,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Some(CommitDiffResponse {
            repo_id: repo_id.to_string(),
            commit_id,
            files,
        }))
    }
}

fn split_record(record: &str, expected_parts: usize) -> Result<Vec<&str>> {
    let parts: Vec<&str> = record.split(FIELD_SEPARATOR).collect();
    if parts.len() != expected_parts {
        return Err(anyhow!(
            "unexpected git output: expected {expected_parts} fields, got {}",
            parts.len()
        ));
    }
    Ok(parts)
}

fn parse_records(output: &str) -> Vec<&str> {
    output
        .split(RECORD_SEPARATOR)
        .map(|record| record.trim_matches('\n'))
        .filter(|record| !record.is_empty())
        .collect()
}

fn parse_diff_name_status(output: &str) -> Result<Vec<RawDiffStatus>> {
    let mut fields = output.split('\0');
    let _commit_id = fields.next();
    let tokens = fields.filter(|field| !field.is_empty()).collect::<Vec<_>>();

    let mut index = 0;
    let mut entries = Vec::new();
    while index < tokens.len() {
        let status = tokens[index];
        index += 1;

        let entry = match status.chars().next() {
            Some('A') => RawDiffStatus {
                path: next_token(&tokens, &mut index, "path")?.to_string(),
                change_type: CommitDiffChangeType::Added,
                old_path: None,
            },
            Some('M') | Some('T') => RawDiffStatus {
                path: next_token(&tokens, &mut index, "path")?.to_string(),
                change_type: CommitDiffChangeType::Modified,
                old_path: None,
            },
            Some('D') => RawDiffStatus {
                path: next_token(&tokens, &mut index, "path")?.to_string(),
                change_type: CommitDiffChangeType::Deleted,
                old_path: None,
            },
            Some('R') => {
                let old_path = next_token(&tokens, &mut index, "old path")?.to_string();
                let new_path = next_token(&tokens, &mut index, "new path")?.to_string();
                RawDiffStatus {
                    path: new_path,
                    change_type: CommitDiffChangeType::Renamed,
                    old_path: Some(old_path),
                }
            }
            other => return Err(anyhow!("unsupported git name-status entry: {:?}", other)),
        };
        entries.push(entry);
    }

    Ok(entries)
}

fn parse_diff_numstat(output: &str) -> Result<Vec<RawNumstat>> {
    let mut fields = output.split('\0');
    let _commit_id = fields.next();
    let tokens = fields.collect::<Vec<_>>();

    let mut index = 0;
    let mut entries = Vec::new();
    while index < tokens.len() {
        let token = tokens[index];
        index += 1;

        if token.is_empty() {
            continue;
        }

        if let Some((additions, deletions, path)) = parse_regular_numstat(token)? {
            entries.push(RawNumstat {
                path: path.to_string(),
                old_path: None,
                additions,
                deletions,
            });
            continue;
        }

        let (additions, deletions) = parse_rename_numstat_header(token)?;
        let old_path = next_token(&tokens, &mut index, "old path")?.to_string();
        let new_path = next_token(&tokens, &mut index, "new path")?.to_string();
        entries.push(RawNumstat {
            path: new_path,
            old_path: Some(old_path),
            additions,
            deletions,
        });
    }

    Ok(entries)
}

fn parse_regular_numstat(token: &str) -> Result<Option<(usize, usize, &str)>> {
    let parts = token.splitn(3, '\t').collect::<Vec<_>>();
    if parts.len() != 3 || parts[2].is_empty() {
        return Ok(None);
    }

    let additions = parse_numstat_count(parts[0])?;
    let deletions = parse_numstat_count(parts[1])?;
    Ok(Some((additions, deletions, parts[2])))
}

fn parse_rename_numstat_header(token: &str) -> Result<(usize, usize)> {
    let parts = token.splitn(3, '\t').collect::<Vec<_>>();
    if parts.len() != 3 || !parts[2].is_empty() {
        return Err(anyhow!("unexpected git numstat rename entry: {token:?}"));
    }

    Ok((
        parse_numstat_count(parts[0])?,
        parse_numstat_count(parts[1])?,
    ))
}

fn parse_numstat_count(value: &str) -> Result<usize> {
    if value == "-" {
        return Ok(0);
    }

    value
        .parse::<usize>()
        .with_context(|| format!("invalid git numstat count: {value}"))
}

fn load_patch_for_diff_entry(
    repo_root: &PathBuf,
    commit_id: &str,
    status: &RawDiffStatus,
) -> Result<Option<String>> {
    let mut args = vec![
        "show".to_string(),
        "--format=".to_string(),
        "--find-renames".to_string(),
        "-M".to_string(),
        "--unified=3".to_string(),
        commit_id.to_string(),
        "--".to_string(),
    ];

    if let Some(old_path) = &status.old_path {
        args.push(old_path.clone());
    }
    args.push(status.path.clone());

    let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let patch = run_git(repo_root, &arg_refs)?;
    Ok(normalize_patch_output(&patch))
}

fn normalize_patch_output(patch: &str) -> Option<String> {
    if patch.trim().is_empty()
        || patch.contains("Binary files ")
        || patch.contains("GIT binary patch")
    {
        None
    } else {
        Some(format!("{patch}"))
    }
}

fn next_token<'a>(tokens: &'a [&str], index: &mut usize, label: &str) -> Result<&'a str> {
    let value = tokens
        .get(*index)
        .copied()
        .ok_or_else(|| anyhow!("missing git diff {label}"))?;
    *index += 1;
    Ok(value)
}

fn run_git(repo_root: &PathBuf, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .with_context(|| format!("failed to run git in {}", repo_root.display()))?;

    git_stdout(repo_root, args, output)
}

fn run_git_allow_not_found(repo_root: &PathBuf, args: &[&str]) -> Result<Option<String>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .with_context(|| format!("failed to run git in {}", repo_root.display()))?;

    if output.status.success() {
        return Ok(Some(
            String::from_utf8(output.stdout).context("git output was not utf-8")?,
        ));
    }

    if git_not_found_output(&output) {
        return Ok(None);
    }

    Err(git_command_error(repo_root, args, &output))
}

fn resolve_single_commit(repo_root: &PathBuf, commit_id: &str) -> Result<Option<String>> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", &format!("{commit_id}^{{commit}}")])
        .current_dir(repo_root)
        .output()
        .with_context(|| format!("failed to run git rev-parse in {}", repo_root.display()))?;

    if !output.status.success() {
        if git_not_found_output(&output) {
            return Ok(None);
        }
        return Err(git_command_error(
            repo_root,
            &["rev-parse", "--verify", "<commit>^{commit}"],
            &output,
        ));
    }

    let resolved = String::from_utf8(output.stdout)
        .context("git output was not utf-8")?
        .trim()
        .to_string();
    if resolved.is_empty() {
        return Ok(None);
    }
    Ok(Some(resolved))
}

fn git_not_found_output(output: &Output) -> bool {
    let stderr = String::from_utf8_lossy(&output.stderr);
    stderr.contains("unknown revision")
        || stderr.contains("bad object")
        || stderr.contains("ambiguous argument")
        || stderr.contains("not a valid object name")
        || stderr.contains("Needed a single revision")
}

fn git_stdout(repo_root: &PathBuf, args: &[&str], output: Output) -> Result<String> {
    if !output.status.success() {
        return Err(git_command_error(repo_root, args, &output));
    }

    String::from_utf8(output.stdout).context("git output was not utf-8")
}

fn git_command_error(repo_root: &PathBuf, args: &[&str], output: &Output) -> anyhow::Error {
    anyhow!(
        "git {:?} failed in {}: {}",
        args,
        repo_root.display(),
        String::from_utf8_lossy(&output.stderr).trim()
    )
}

pub fn build_commit_store() -> DynCommitStore {
    Arc::new(LocalCommitStore::seeded())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::symlink,
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn local_commit_store_lists_real_seeded_repository_commits() {
        let store = LocalCommitStore::seeded();

        let response = store
            .list_commits("repo_sourcebot_rewrite", 2)
            .unwrap()
            .unwrap();

        assert_eq!(response.repo_id, "repo_sourcebot_rewrite");
        assert_eq!(response.commits.len(), 2);
        assert_eq!(response.commits[0].short_id, "fe7f21f");
        assert_eq!(
            response.commits[0].summary,
            "feat: add commit history api and web ui"
        );
    }

    #[test]
    fn local_commit_store_reads_real_commit_detail() {
        let store = LocalCommitStore::seeded();

        let response = store
            .get_commit("repo_sourcebot_rewrite", "556fb45")
            .unwrap()
            .unwrap();

        assert_eq!(response.repo_id, "repo_sourcebot_rewrite");
        assert_eq!(response.commit.short_id, "556fb45");
        assert_eq!(
            response.commit.parents,
            vec!["c22186448cc5b760e83b5a759d105409f1a15e6e".to_string()]
        );
    }

    #[test]
    fn local_commit_store_returns_none_for_unknown_repo_or_commit() {
        let store = LocalCommitStore::seeded();

        assert!(store.list_commits("missing", 20).unwrap().is_none());
        assert_eq!(
            store
                .list_commits("repo_demo_docs", 20)
                .unwrap()
                .unwrap()
                .commits,
            Vec::<CommitSummary>::new()
        );
        assert!(store
            .get_commit("repo_sourcebot_rewrite", "definitely-missing")
            .unwrap()
            .is_none());
        assert!(store
            .get_commit("repo_sourcebot_rewrite", "HEAD~1..HEAD")
            .unwrap()
            .is_none());
        assert!(store
            .get_commit_diff("missing", "fe7f21f")
            .unwrap()
            .is_none());
        assert!(store
            .get_commit_diff("repo_sourcebot_rewrite", "definitely-missing")
            .unwrap()
            .is_none());
        assert!(store
            .get_commit_diff("repo_sourcebot_rewrite", "HEAD~1..HEAD")
            .unwrap()
            .is_none());
    }

    #[test]
    fn parse_diff_name_status_accepts_type_changes_as_modified() {
        let entries = parse_diff_name_status("fe7f21f\0T\0path/to/file\0").unwrap();

        assert_eq!(
            entries,
            vec![RawDiffStatus {
                path: "path/to/file".to_string(),
                change_type: CommitDiffChangeType::Modified,
                old_path: None,
            }]
        );
    }

    #[test]
    fn normalize_patch_output_marks_binary_patches_as_unavailable() {
        assert_eq!(
            normalize_patch_output(
                "diff --git a/assets/logo.png b/assets/logo.png\nBinary files a/assets/logo.png and b/assets/logo.png differ\n",
            ),
            None
        );
    }

    #[test]
    fn local_commit_store_handles_type_changes_as_single_diff_entry() {
        let repo_root = create_temp_git_repo("type-change");
        write_text_file(&repo_root.join("demo"), "hello\n");
        git_in(&repo_root, &["add", "demo"]);
        git_in(&repo_root, &["commit", "-m", "init"]);

        fs::remove_file(repo_root.join("demo")).unwrap();
        symlink("target", repo_root.join("demo")).unwrap();
        git_in(&repo_root, &["add", "-A"]);
        git_in(&repo_root, &["commit", "-m", "typechange"]);

        let commit_id = git_stdout_trimmed(&repo_root, &["rev-parse", "HEAD"]);
        let store = LocalCommitStore::new(HashMap::from([(
            "repo_temp".to_string(),
            repo_root.clone(),
        )]));

        let response = store
            .get_commit_diff("repo_temp", &commit_id)
            .unwrap()
            .unwrap();

        assert_eq!(response.files.len(), 1);
        let file = &response.files[0];
        assert_eq!(file.path, "demo");
        assert_eq!(file.change_type, CommitDiffChangeType::Modified);
        assert_eq!(file.additions, 1);
        assert_eq!(file.deletions, 1);
        assert!(file
            .patch
            .as_deref()
            .unwrap()
            .contains("deleted file mode 100644"));
        assert!(file
            .patch
            .as_deref()
            .unwrap()
            .contains("new file mode 120000"));

        fs::remove_dir_all(repo_root).unwrap();
    }

    #[test]
    fn local_commit_store_reads_real_commit_diff() {
        let store = LocalCommitStore::seeded();

        let response = store
            .get_commit_diff("repo_sourcebot_rewrite", "fe7f21f")
            .unwrap()
            .unwrap();

        assert_eq!(response.repo_id, "repo_sourcebot_rewrite");
        assert_eq!(
            response.commit_id,
            "fe7f21fca594b0dd76988dbaa1ac18bd0c03ce78"
        );
        assert_eq!(response.files.len(), 5);

        let commits_file = response
            .files
            .iter()
            .find(|file| file.path == "crates/api/src/commits.rs")
            .unwrap();
        assert_eq!(commits_file.change_type, CommitDiffChangeType::Added);
        assert_eq!(commits_file.old_path, None);
        assert_eq!(commits_file.additions, 343);
        assert_eq!(commits_file.deletions, 0);
        assert!(commits_file
            .patch
            .as_deref()
            .unwrap()
            .contains("diff --git a/crates/api/src/commits.rs b/crates/api/src/commits.rs"));

        let cargo_lock = response
            .files
            .iter()
            .find(|file| file.path == "Cargo.lock")
            .unwrap();
        assert_eq!(cargo_lock.change_type, CommitDiffChangeType::Modified);
        assert_eq!(cargo_lock.additions, 1);
        assert_eq!(cargo_lock.deletions, 1);
    }

    fn create_temp_git_repo(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("sourcebot-api-{label}-{unique}"));
        fs::create_dir_all(&path).unwrap();
        git_in(&path, &["init"]);
        git_in(&path, &["config", "user.name", "Hermes Test"]);
        git_in(&path, &["config", "user.email", "hermes-test@example.com"]);
        path
    }

    fn write_text_file(path: &Path, content: &str) {
        fs::write(path, content).unwrap();
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
}
