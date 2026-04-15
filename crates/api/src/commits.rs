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

    #[test]
    fn local_commit_store_lists_real_seeded_repository_commits() {
        let store = LocalCommitStore::seeded();

        let response = store
            .list_commits("repo_sourcebot_rewrite", 2)
            .unwrap()
            .unwrap();

        assert_eq!(response.repo_id, "repo_sourcebot_rewrite");
        assert_eq!(response.commits.len(), 2);
        assert_eq!(response.commits[0].short_id, "556fb45");
        assert_eq!(
            response.commits[0].summary,
            "feat: add minimal search api and web ui"
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
    }
}
