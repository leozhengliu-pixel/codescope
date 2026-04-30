use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::Read,
    path::{Component, Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

pub type DynCommitStore = Arc<dyn CommitStore>;

const SOURCEBOT_REWRITE_REPO_ID: &str = "repo_sourcebot_rewrite";
const SOURCEBOT_REWRITE_ROOT: &str = "/opt/data/projects/sourcebot-rewrite";
const EMPTY_HISTORY_REPO_IDS: &[&str] = &["repo_demo_docs"];
const COMMIT_DIFF_PATCH_MAX_BYTES: usize = 64 * 1024;
const COMMIT_DIFF_PATCH_TRUNCATED_MARKER: &str = "[Sourcebot diff truncated: patch exceeds 64 KiB]";
const MAX_COMMIT_DIFF_FILES: usize = 100;
const MAX_COMMIT_PAGE_LIMIT: usize = 100;

pub trait CommitStore: Send + Sync {
    fn list_commits(
        &self,
        repo_id: &str,
        limit: usize,
        offset: usize,
        revision: Option<&str>,
    ) -> Result<Option<CommitListResponse>>;
    fn list_refs(&self, repo_id: &str) -> Result<Option<RefListResponse>>;
    fn get_commit(&self, repo_id: &str, commit_id: &str) -> Result<Option<CommitDetailResponse>>;
    fn get_commit_diff(&self, repo_id: &str, commit_id: &str)
        -> Result<Option<CommitDiffResponse>>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitSummary {
    pub id: String,
    pub short_id: String,
    pub summary: String,
    pub author_name: String,
    pub authored_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitDetail {
    pub id: String,
    pub short_id: String,
    pub summary: String,
    pub author_name: String,
    pub authored_at: String,
    pub body: String,
    pub parents: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitPageInfo {
    pub limit: usize,
    pub offset: usize,
    pub has_next_page: bool,
    pub next_offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitListResponse {
    pub repo_id: String,
    pub commits: Vec<CommitSummary>,
    pub page_info: CommitPageInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RefKind {
    Branch,
    Tag,
}

impl RefKind {
    fn sort_key(&self) -> u8 {
        match self {
            RefKind::Branch => 0,
            RefKind::Tag => 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RefSummary {
    pub name: String,
    pub target: String,
    pub kind: RefKind,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RefListResponse {
    pub repo_id: String,
    pub refs: Vec<RefSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitDetailResponse {
    pub repo_id: String,
    pub commit: CommitDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommitDiffChangeType {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitDiffFile {
    pub path: String,
    pub change_type: CommitDiffChangeType,
    pub old_path: Option<String>,
    pub additions: usize,
    pub deletions: usize,
    pub patch: Option<String>,
    pub patch_truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedPatch {
    patch: Option<String>,
    truncated: bool,
}

#[derive(Clone, Default)]
pub struct LocalCommitStore {
    repo_roots: HashMap<String, PathBuf>,
    snapshot_revisions: HashMap<String, String>,
}

impl LocalCommitStore {
    pub fn new(repo_roots: HashMap<String, PathBuf>) -> Self {
        Self {
            repo_roots,
            snapshot_revisions: HashMap::new(),
        }
    }

    pub fn with_snapshot_revision(
        repo_id: String,
        repo_root: PathBuf,
        snapshot_revision: String,
    ) -> Self {
        Self {
            repo_roots: HashMap::from([(repo_id.clone(), repo_root)]),
            snapshot_revisions: HashMap::from([(repo_id, snapshot_revision)]),
        }
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

    fn snapshot_revision(&self, repo_id: &str) -> Option<&str> {
        self.snapshot_revisions.get(repo_id).map(String::as_str)
    }

    fn commit_is_visible_in_snapshot(
        &self,
        repo_id: &str,
        repo_root: &PathBuf,
        commit_id: &str,
    ) -> Result<bool> {
        let Some(snapshot_revision) = self.snapshot_revision(repo_id) else {
            return Ok(true);
        };
        let Some(snapshot_revision) = resolve_single_commit(repo_root, snapshot_revision)? else {
            return Ok(false);
        };
        commit_is_ancestor(repo_root, commit_id, &snapshot_revision)
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
    fn list_commits(
        &self,
        repo_id: &str,
        limit: usize,
        offset: usize,
        revision: Option<&str>,
    ) -> Result<Option<CommitListResponse>> {
        let page_limit = limit.clamp(1, MAX_COMMIT_PAGE_LIMIT);
        let Some(repo_root) = self.repo_root(repo_id) else {
            if self.supports_empty_history(repo_id) {
                return Ok(Some(CommitListResponse {
                    repo_id: repo_id.to_string(),
                    commits: Vec::new(),
                    page_info: CommitPageInfo {
                        limit: page_limit,
                        offset,
                        has_next_page: false,
                        next_offset: None,
                    },
                }));
            }
            return Ok(None);
        };

        let revision = revision.or_else(|| self.snapshot_revision(repo_id));
        let resolved_revision = match revision {
            Some(revision) => match resolve_single_commit(repo_root, revision)? {
                Some(resolved) => Some(resolved),
                None => return Ok(None),
            },
            None => None,
        };
        if let Some(revision) = resolved_revision.as_deref() {
            if !self.commit_is_visible_in_snapshot(repo_id, repo_root, revision)? {
                return Ok(None);
            }
        }

        let mut args = vec![
            "log".to_string(),
            format!("--max-count={}", page_limit.saturating_add(1)),
            format!("--skip={offset}"),
            "--format=%H%x00%h%x00%s%x00%an%x00%aI%x00".to_string(),
        ];
        if let Some(revision) = resolved_revision.as_deref() {
            args.push(revision.to_string());
        }
        let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
        let output = run_git(repo_root, &arg_refs)?;

        let mut commits = parse_nul_field_records(&output, 5)?
            .into_iter()
            .map(|record| self.parse_summary_record(&record))
            .collect::<Result<Vec<_>>>()?;
        let has_next_page = commits.len() > page_limit;
        commits.truncate(page_limit);
        let next_offset = has_next_page.then_some(offset + commits.len());

        Ok(Some(CommitListResponse {
            repo_id: repo_id.to_string(),
            commits,
            page_info: CommitPageInfo {
                limit: page_limit,
                offset,
                has_next_page,
                next_offset,
            },
        }))
    }

    fn list_refs(&self, repo_id: &str) -> Result<Option<RefListResponse>> {
        let Some(repo_root) = self.repo_root(repo_id) else {
            return Ok(None);
        };

        let default_branch = default_branch_ref_name(repo_root)?;
        let output = run_git(
            repo_root,
            &[
                "for-each-ref",
                "--format=%(refname)%09%(objectname)%09%(objecttype)",
                "refs/heads",
                "refs/remotes",
                "refs/tags",
            ],
        )?;
        let mut refs = Vec::new();
        for record in output.lines().filter(|record| !record.is_empty()) {
            let parts = record.split('\t').collect::<Vec<_>>();
            if parts.len() != 3 {
                anyhow::bail!(
                    "unexpected git ref output: expected 3 fields, got {}",
                    parts.len()
                );
            }
            let refname = parts[0];
            let (kind, name) = if let Some(name) = refname.strip_prefix("refs/heads/") {
                (RefKind::Branch, name)
            } else if let Some(name) = refname.strip_prefix("refs/remotes/") {
                if name.ends_with("/HEAD") {
                    continue;
                }
                (RefKind::Branch, name)
            } else if let Some(name) = refname.strip_prefix("refs/tags/") {
                (RefKind::Tag, name)
            } else {
                continue;
            };
            let Some(target) = resolve_single_commit(repo_root, refname)? else {
                continue;
            };
            if !self.commit_is_visible_in_snapshot(repo_id, repo_root, &target)? {
                continue;
            }
            refs.push(RefSummary {
                is_default: kind == RefKind::Branch && default_branch.as_deref() == Some(name),
                name: name.to_string(),
                target,
                kind,
            });
        }
        refs.sort_by(|left, right| {
            left.kind
                .sort_key()
                .cmp(&right.kind.sort_key())
                .then_with(|| (!left.is_default).cmp(&(!right.is_default)))
                .then_with(|| left.name.cmp(&right.name))
                .then_with(|| left.target.cmp(&right.target))
        });

        Ok(Some(RefListResponse {
            repo_id: repo_id.to_string(),
            refs,
        }))
    }

    fn get_commit(&self, repo_id: &str, commit_id: &str) -> Result<Option<CommitDetailResponse>> {
        let Some(repo_root) = self.repo_root(repo_id) else {
            return Ok(None);
        };

        let Some(commit_id) = resolve_single_commit(repo_root, commit_id)? else {
            return Ok(None);
        };
        if !self.commit_is_visible_in_snapshot(repo_id, repo_root, &commit_id)? {
            return Ok(None);
        }

        let output = run_git_allow_not_found(
            repo_root,
            &[
                "show",
                "--no-patch",
                "--format=%H%x00%h%x00%s%x00%an%x00%aI%x00%b%x00%P%x00",
                &commit_id,
            ],
        )?;

        let Some(output) = output else {
            return Ok(None);
        };

        let Some(record) = parse_nul_field_records(&output, 7)?.into_iter().next() else {
            return Ok(None);
        };

        Ok(Some(CommitDetailResponse {
            repo_id: repo_id.to_string(),
            commit: self.parse_detail_record(&record)?,
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
        if !self.commit_is_visible_in_snapshot(repo_id, repo_root, &commit_id)? {
            return Ok(None);
        }

        let status_output = run_git_allow_not_found(
            repo_root,
            &[
                "diff-tree",
                "--root",
                "-r",
                "--find-renames",
                "--find-copies",
                "--find-copies-harder",
                "-M",
                "-C",
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
                "--find-copies",
                "--find-copies-harder",
                "-M",
                "-C",
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
        ensure_diff_file_count_bounded(commit_id.as_str(), statuses.len())?;
        ensure_diff_file_count_bounded(commit_id.as_str(), numstats.len())?;
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

                let normalized_patch = load_patch_for_diff_entry(repo_root, &commit_id, &status)?;
                Ok(CommitDiffFile {
                    path: status.path.clone(),
                    change_type: status.change_type,
                    old_path: status.old_path,
                    additions: numstat.additions,
                    deletions: numstat.deletions,
                    patch: normalized_patch.patch,
                    patch_truncated: normalized_patch.truncated,
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
    let parts: Vec<&str> = record.split('\0').collect();
    if parts.len() != expected_parts {
        return Err(anyhow!(
            "unexpected git output: expected {expected_parts} fields, got {}",
            parts.len()
        ));
    }
    Ok(parts)
}

fn parse_nul_field_records(output: &str, fields_per_record: usize) -> Result<Vec<String>> {
    if fields_per_record == 0 {
        return Err(anyhow!("git output record width must be non-zero"));
    }

    let mut tokens = output.trim_matches('\n').split('\0').collect::<Vec<_>>();
    if tokens.last().is_some_and(|token| token.is_empty()) {
        tokens.pop();
    }
    if tokens.is_empty() {
        return Ok(Vec::new());
    }
    if tokens.len() % fields_per_record != 0 {
        return Err(anyhow!(
            "unexpected git output: expected groups of {fields_per_record} NUL-delimited fields, got {} fields",
            tokens.len()
        ));
    }

    Ok(tokens
        .chunks(fields_per_record)
        .map(|chunk| {
            let mut record = chunk.join("\0");
            if let Some(stripped) = record.strip_prefix('\n') {
                record = stripped.to_string();
            }
            record
        })
        .collect())
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
                path: next_diff_path(&tokens, &mut index, "path")?.to_string(),
                change_type: CommitDiffChangeType::Added,
                old_path: None,
            },
            Some('M') | Some('T') => RawDiffStatus {
                path: next_diff_path(&tokens, &mut index, "path")?.to_string(),
                change_type: CommitDiffChangeType::Modified,
                old_path: None,
            },
            Some('D') => RawDiffStatus {
                path: next_diff_path(&tokens, &mut index, "path")?.to_string(),
                change_type: CommitDiffChangeType::Deleted,
                old_path: None,
            },
            Some('R') => {
                let old_path = next_diff_path(&tokens, &mut index, "old path")?.to_string();
                let new_path = next_diff_path(&tokens, &mut index, "new path")?.to_string();
                RawDiffStatus {
                    path: new_path,
                    change_type: CommitDiffChangeType::Renamed,
                    old_path: Some(old_path),
                }
            }
            Some('C') => {
                let old_path = next_diff_path(&tokens, &mut index, "old path")?.to_string();
                let new_path = next_diff_path(&tokens, &mut index, "new path")?.to_string();
                RawDiffStatus {
                    path: new_path,
                    change_type: CommitDiffChangeType::Copied,
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
                path: validate_diff_path(path, "path")?.to_string(),
                old_path: None,
                additions,
                deletions,
            });
            continue;
        }

        let (additions, deletions) = parse_rename_numstat_header(token)?;
        let old_path = next_diff_path(&tokens, &mut index, "old path")?.to_string();
        let new_path = next_diff_path(&tokens, &mut index, "new path")?.to_string();
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
) -> Result<NormalizedPatch> {
    let mut args = vec![
        "show".to_string(),
        "--format=".to_string(),
        "--find-renames".to_string(),
        "--find-copies".to_string(),
        "--find-copies-harder".to_string(),
        "-M".to_string(),
        "-C".to_string(),
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

fn normalize_patch_output(patch: &str) -> NormalizedPatch {
    if patch.trim().is_empty()
        || patch.contains("Binary files ")
        || patch.contains("GIT binary patch")
    {
        return NormalizedPatch {
            patch: None,
            truncated: false,
        };
    }

    if patch.len() <= COMMIT_DIFF_PATCH_MAX_BYTES {
        return NormalizedPatch {
            patch: Some(patch.to_string()),
            truncated: false,
        };
    }

    let marker = format!("\n{COMMIT_DIFF_PATCH_TRUNCATED_MARKER}\n");
    let prefix_limit = COMMIT_DIFF_PATCH_MAX_BYTES.saturating_sub(marker.len());
    let prefix = truncate_str_to_byte_boundary(patch, prefix_limit);
    NormalizedPatch {
        patch: Some(format!("{prefix}{marker}")),
        truncated: true,
    }
}

fn truncate_str_to_byte_boundary(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }

    let mut boundary = max_bytes;
    while !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    &value[..boundary]
}

fn ensure_diff_file_count_bounded(commit_id: &str, file_count: usize) -> Result<()> {
    if file_count > MAX_COMMIT_DIFF_FILES {
        return Err(anyhow!(
            "commit diff changed file limit exceeded for commit {commit_id}: {file_count} files > {MAX_COMMIT_DIFF_FILES} limit"
        ));
    }

    Ok(())
}

fn next_token<'a>(tokens: &'a [&str], index: &mut usize, label: &str) -> Result<&'a str> {
    let value = tokens
        .get(*index)
        .copied()
        .ok_or_else(|| anyhow!("missing git diff {label}"))?;
    *index += 1;
    Ok(value)
}

fn next_diff_path<'a>(tokens: &'a [&str], index: &mut usize, label: &str) -> Result<&'a str> {
    let value = next_token(tokens, index, label)?;
    validate_diff_path(value, label)
}

fn validate_diff_path<'a>(path: &'a str, label: &str) -> Result<&'a str> {
    if path.is_empty() {
        return Err(anyhow!("unsafe git diff {label}: empty path"));
    }
    if path.contains('\\') {
        return Err(anyhow!("unsafe git diff {label}: backslash path {path:?}"));
    }
    if path.chars().any(char::is_control) {
        return Err(anyhow!(
            "unsafe git diff {label}: control-character path {path:?}"
        ));
    }

    let parsed = Path::new(path);
    if parsed.is_absolute() {
        return Err(anyhow!("unsafe git diff {label}: absolute path {path:?}"));
    }
    if parsed.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(anyhow!("unsafe git diff {label}: parent path {path:?}"));
    }

    Ok(path)
}

fn run_git(repo_root: &PathBuf, args: &[&str]) -> Result<String> {
    let output = bounded_git_output(repo_root, args)?;

    git_stdout(repo_root, args, output)
}

fn run_git_allow_not_found(repo_root: &PathBuf, args: &[&str]) -> Result<Option<String>> {
    let output = bounded_git_output(repo_root, args)?;

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

fn default_branch_ref_name(repo_root: &PathBuf) -> Result<Option<String>> {
    if let Some(branch) = symbolic_ref_short(repo_root, "HEAD")? {
        return Ok(Some(branch));
    }

    symbolic_ref_short(repo_root, "refs/remotes/origin/HEAD")
}

fn symbolic_ref_short(repo_root: &PathBuf, refname: &str) -> Result<Option<String>> {
    let output = bounded_git_output(repo_root, &["symbolic-ref", "--quiet", "--short", refname])?;
    if output.status.success() {
        let branch = String::from_utf8(output.stdout)
            .context("git output was not utf-8")?
            .trim()
            .to_string();
        return Ok((!branch.is_empty()).then_some(branch));
    }
    match output.status.code() {
        Some(1) => Ok(None),
        _ => Err(git_command_error(
            repo_root,
            &["symbolic-ref", "--quiet", "--short", "<ref>"],
            &output,
        )),
    }
}

fn resolve_single_commit(repo_root: &PathBuf, commit_id: &str) -> Result<Option<String>> {
    if !is_safe_revision_selector(commit_id) {
        return Ok(None);
    }

    let verify_arg = format!("{commit_id}^{{commit}}");
    let args = [
        "rev-parse",
        "--verify",
        "--end-of-options",
        verify_arg.as_str(),
    ];
    let output = bounded_git_output(repo_root, &args)?;

    if !output.status.success() {
        if git_not_found_output(&output) {
            return Ok(None);
        }
        return Err(git_command_error(
            repo_root,
            &[
                "rev-parse",
                "--verify",
                "--end-of-options",
                "<commit>^{commit}",
            ],
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

fn is_safe_revision_selector(revision: &str) -> bool {
    !revision.is_empty()
        && !revision.starts_with('-')
        && !revision.contains("..")
        && !revision.contains("@{")
        && !revision
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, '^' | '~' | ':' | '?' | '*' | '[' | '\\'))
}

fn commit_is_ancestor(repo_root: &PathBuf, commit_id: &str, ancestor_of: &str) -> Result<bool> {
    let output = bounded_git_output(
        repo_root,
        &["merge-base", "--is-ancestor", commit_id, ancestor_of],
    )?;
    if output.status.success() {
        return Ok(true);
    }
    match output.status.code() {
        Some(1) => Ok(false),
        _ => Err(git_command_error(
            repo_root,
            &["merge-base", "--is-ancestor", "<commit>", "<snapshot>"],
            &output,
        )),
    }
}

fn git_not_found_output(output: &Output) -> bool {
    let stderr = String::from_utf8_lossy(&output.stderr);
    stderr.contains("unknown revision")
        || stderr.contains("bad object")
        || stderr.contains("ambiguous argument")
        || stderr.contains("not a valid object name")
        || stderr.contains("Needed a single revision")
}

fn bounded_git_output(repo_root: &PathBuf, args: &[&str]) -> Result<Output> {
    const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
    const GIT_STDOUT_CAPTURE_MAX_BYTES: usize = 1024 * 1024;
    const GIT_STDERR_CAPTURE_MAX_BYTES: usize = 64 * 1024;

    let mut child = Command::new("git")
        .args(args)
        .env("GIT_LITERAL_PATHSPECS", "1")
        .current_dir(repo_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to start git in {}", repo_root.display()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("failed to capture git stdout in {}", repo_root.display()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("failed to capture git stderr in {}", repo_root.display()))?;
    let stdout_reader =
        thread::spawn(move || read_stream_bounded(stdout, GIT_STDOUT_CAPTURE_MAX_BYTES));
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
                .map_err(|_| anyhow!("failed to join git stdout reader"))??;
            let stderr = stderr_reader
                .join()
                .map_err(|_| anyhow!("failed to join git stderr reader"))??;
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
            return Err(anyhow!(
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
    loop {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            return Ok(captured);
        }

        let remaining = max_bytes.saturating_sub(captured.len());
        if read > remaining {
            if remaining > 0 {
                captured.extend_from_slice(&buffer[..remaining]);
            }
            return Err(anyhow!(
                "git output exceeded {max_bytes} byte capture limit"
            ));
        }
        captured.extend_from_slice(&buffer[..read]);
    }
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
            .list_commits("repo_sourcebot_rewrite", 2, 0, None)
            .unwrap()
            .unwrap();

        assert_eq!(response.repo_id, "repo_sourcebot_rewrite");
        assert_eq!(response.commits.len(), 2);

        let expected_head_id = git_stdout_trimmed(
            &PathBuf::from(SOURCEBOT_REWRITE_ROOT),
            &["rev-parse", "HEAD"],
        );
        let expected_head_short_id = git_stdout_trimmed(
            &PathBuf::from(SOURCEBOT_REWRITE_ROOT),
            &["rev-parse", "--short=7", "HEAD"],
        );
        let expected_head_summary = git_stdout_trimmed(
            &PathBuf::from(SOURCEBOT_REWRITE_ROOT),
            &["log", "-1", "--pretty=%s", "HEAD"],
        );

        assert_eq!(response.commits[0].id, expected_head_id);
        assert_eq!(response.commits[0].short_id, expected_head_short_id);
        assert_eq!(response.commits[0].summary, expected_head_summary);
    }

    #[test]
    fn local_commit_store_reads_real_commit_detail() {
        let store = LocalCommitStore::seeded();
        let commit_id = store
            .list_commits("repo_sourcebot_rewrite", 2, 0, None)
            .unwrap()
            .unwrap()
            .commits
            .into_iter()
            .nth(1)
            .expect("seeded repository should expose at least two commits")
            .short_id;

        let response = store
            .get_commit("repo_sourcebot_rewrite", &commit_id)
            .unwrap()
            .unwrap();

        assert_eq!(response.repo_id, "repo_sourcebot_rewrite");
        assert_eq!(response.commit.short_id, commit_id);
        assert_eq!(response.commit.author_name, "Hermes Agent");
        assert_eq!(response.commit.id.len(), 40);
        assert!(response.commit.authored_at.ends_with('Z'));
    }

    #[test]
    fn local_commit_store_preserves_commit_messages_with_internal_field_separators() {
        let repo_root = create_temp_git_repo("commit-message-separators");
        write_text_file(&repo_root.join("demo.txt"), "separator-safe\n");
        git_in(&repo_root, &["add", "demo.txt"]);
        git_in(
            &repo_root,
            &[
                "commit",
                "-m",
                "subject with \u{1f} separator",
                "-m",
                "body keeps \u{1e} record marker",
            ],
        );

        let store = LocalCommitStore::new(HashMap::from([(
            "repo_test".to_string(),
            repo_root.clone(),
        )]));

        let list = store
            .list_commits("repo_test", 1, 0, None)
            .expect("commit list should parse separator-bearing subject")
            .unwrap();
        assert_eq!(list.commits[0].summary, "subject with \u{1f} separator");

        let detail = store
            .get_commit("repo_test", &list.commits[0].id)
            .expect("commit detail should parse separator-bearing body")
            .unwrap();
        assert_eq!(detail.commit.summary, "subject with \u{1f} separator");
        assert_eq!(detail.commit.body, "body keeps \u{1e} record marker");

        fs::remove_dir_all(repo_root).unwrap();
    }

    #[test]
    fn local_commit_store_lists_branch_and_tag_refs() {
        let repo_root = create_temp_git_repo("ref-list");
        write_text_file(&repo_root.join("demo.txt"), "base\n");
        git_in(&repo_root, &["add", "demo.txt"]);
        git_in(&repo_root, &["commit", "-m", "base"]);
        let base_revision = git_stdout_trimmed(&repo_root, &["rev-parse", "HEAD"]);
        git_in(&repo_root, &["tag", "v1.0.0"]);
        git_in(&repo_root, &["checkout", "-b", "feature/ref-list"]);
        write_text_file(&repo_root.join("demo.txt"), "feature\n");
        git_in(&repo_root, &["commit", "-am", "feature"]);
        let feature_revision = git_stdout_trimmed(&repo_root, &["rev-parse", "HEAD"]);

        let store = LocalCommitStore::new(HashMap::from([(
            "repo_temp".to_string(),
            repo_root.clone(),
        )]));

        let refs = store.list_refs("repo_temp").unwrap().unwrap();

        assert_eq!(refs.repo_id, "repo_temp");
        assert!(refs.refs.iter().any(|reference| {
            reference.name == "feature/ref-list"
                && reference.kind == RefKind::Branch
                && reference.target == feature_revision
                && reference.is_default
        }));
        assert!(refs.refs.iter().any(|reference| {
            reference.name == "v1.0.0"
                && reference.kind == RefKind::Tag
                && reference.target == base_revision
                && !reference.is_default
        }));

        fs::remove_dir_all(repo_root).unwrap();
    }

    #[test]
    fn local_commit_store_lists_remote_tracking_branch_refs() {
        let repo_root = create_temp_git_repo("remote-ref-list");
        write_text_file(&repo_root.join("demo.txt"), "base\n");
        git_in(&repo_root, &["add", "demo.txt"]);
        git_in(&repo_root, &["commit", "-m", "base"]);
        let main_revision = git_stdout_trimmed(&repo_root, &["rev-parse", "HEAD"]);
        git_in(
            &repo_root,
            &["update-ref", "refs/remotes/origin/main", "HEAD"],
        );

        git_in(&repo_root, &["checkout", "-b", "feature/remote-ref-list"]);
        write_text_file(&repo_root.join("demo.txt"), "feature\n");
        git_in(&repo_root, &["commit", "-am", "feature"]);
        let feature_revision = git_stdout_trimmed(&repo_root, &["rev-parse", "HEAD"]);
        git_in(
            &repo_root,
            &[
                "update-ref",
                "refs/remotes/origin/feature/remote-ref-list",
                "HEAD",
            ],
        );
        git_in(
            &repo_root,
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/main",
            ],
        );

        let store = LocalCommitStore::new(HashMap::from([(
            "repo_temp".to_string(),
            repo_root.clone(),
        )]));

        let refs = store.list_refs("repo_temp").unwrap().unwrap();

        assert!(refs.refs.iter().any(|reference| {
            reference.name == "origin/main"
                && reference.kind == RefKind::Branch
                && reference.target == main_revision
        }));
        assert!(refs.refs.iter().any(|reference| {
            reference.name == "origin/feature/remote-ref-list"
                && reference.kind == RefKind::Branch
                && reference.target == feature_revision
        }));
        assert!(
            refs.refs
                .iter()
                .all(|reference| reference.name != "origin/HEAD"),
            "symbolic remote HEAD should not be exposed as a selectable branch"
        );

        fs::remove_dir_all(repo_root).unwrap();
    }

    #[test]
    fn local_commit_store_marks_remote_head_default_when_detached() {
        let repo_root = create_temp_git_repo("detached-remote-default-ref");
        write_text_file(&repo_root.join("demo.txt"), "base\n");
        git_in(&repo_root, &["add", "demo.txt"]);
        git_in(&repo_root, &["commit", "-m", "base"]);
        let main_revision = git_stdout_trimmed(&repo_root, &["rev-parse", "HEAD"]);
        git_in(
            &repo_root,
            &["update-ref", "refs/remotes/origin/main", "HEAD"],
        );
        git_in(
            &repo_root,
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/main",
            ],
        );
        git_in(&repo_root, &["checkout", "--detach", "HEAD"]);

        let store = LocalCommitStore::new(HashMap::from([(
            "repo_temp".to_string(),
            repo_root.clone(),
        )]));

        let refs = store.list_refs("repo_temp").unwrap().unwrap();

        assert!(refs.refs.iter().any(|reference| {
            reference.name == "origin/main"
                && reference.kind == RefKind::Branch
                && reference.target == main_revision
                && reference.is_default
        }));
        assert!(
            refs.refs
                .iter()
                .all(|reference| reference.name != "origin/HEAD"),
            "symbolic remote HEAD should remain hidden when used as the default marker"
        );

        fs::remove_dir_all(repo_root).unwrap();
    }

    #[test]
    fn local_commit_store_lists_commits_from_requested_revision() {
        let repo_root = create_temp_git_repo("revision-list");
        write_text_file(&repo_root.join("demo.txt"), "base\n");
        git_in(&repo_root, &["add", "demo.txt"]);
        git_in(&repo_root, &["commit", "-m", "base"]);

        git_in(&repo_root, &["checkout", "-b", "feature/revision-list"]);
        write_text_file(&repo_root.join("demo.txt"), "feature\n");
        git_in(&repo_root, &["commit", "-am", "feature"]);

        git_in(&repo_root, &["checkout", "master"]);
        let store = LocalCommitStore::new(HashMap::from([(
            "repo_temp".to_string(),
            repo_root.clone(),
        )]));

        let feature_commits = store
            .list_commits("repo_temp", 5, 0, Some("feature/revision-list"))
            .unwrap()
            .unwrap();
        let master_commits = store
            .list_commits("repo_temp", 5, 0, Some("master"))
            .unwrap()
            .unwrap();

        assert_eq!(feature_commits.commits[0].summary, "feature");
        assert_eq!(master_commits.commits[0].summary, "base");

        fs::remove_dir_all(repo_root).unwrap();
    }

    #[test]
    fn local_commit_store_rejects_revision_lists_outside_snapshot_scope() {
        let repo_root = create_temp_git_repo("snapshot-revision-list-scope");
        write_text_file(&repo_root.join("scoped.txt"), "base\n");
        git_in(&repo_root, &["add", "scoped.txt"]);
        git_in(&repo_root, &["commit", "-m", "base"]);
        let snapshot_revision = git_stdout_trimmed(&repo_root, &["rev-parse", "HEAD"]);

        write_text_file(&repo_root.join("scoped.txt"), "hidden\n");
        git_in(&repo_root, &["commit", "-am", "hidden"]);
        let hidden_revision = git_stdout_trimmed(&repo_root, &["rev-parse", "HEAD"]);
        let store = LocalCommitStore::with_snapshot_revision(
            "repo_temp".to_string(),
            repo_root.clone(),
            snapshot_revision.clone(),
        );

        assert!(store
            .list_commits("repo_temp", 5, 0, Some(&hidden_revision))
            .unwrap()
            .is_none());
        let scoped = store
            .list_commits("repo_temp", 5, 0, Some(&snapshot_revision))
            .unwrap()
            .unwrap();
        assert_eq!(scoped.commits[0].summary, "base");

        fs::remove_dir_all(repo_root).unwrap();
    }

    #[test]
    fn local_commit_store_returns_commit_pagination_metadata() {
        let repo_root = create_temp_git_repo("commit-pagination");
        for summary in ["first", "second", "third"] {
            fs::write(repo_root.join("history.txt"), format!("{summary}\n")).unwrap();
            git_in(&repo_root, &["add", "history.txt"]);
            git_in(&repo_root, &["commit", "-m", summary]);
        }
        let store = LocalCommitStore::new(HashMap::from([(
            "repo_temp".to_string(),
            repo_root.clone(),
        )]));

        let first_page = store
            .list_commits("repo_temp", 2, 0, None)
            .unwrap()
            .unwrap();
        assert_eq!(
            first_page
                .commits
                .iter()
                .map(|commit| commit.summary.as_str())
                .collect::<Vec<_>>(),
            vec!["third", "second"]
        );
        assert_eq!(first_page.page_info.limit, 2);
        assert_eq!(first_page.page_info.offset, 0);
        assert!(first_page.page_info.has_next_page);
        assert_eq!(first_page.page_info.next_offset, Some(2));

        let second_page = store
            .list_commits(
                "repo_temp",
                2,
                first_page.page_info.next_offset.unwrap(),
                None,
            )
            .unwrap()
            .unwrap();
        assert_eq!(second_page.commits[0].summary, "first");
        assert!(!second_page.page_info.has_next_page);
        assert_eq!(second_page.page_info.next_offset, None);

        fs::remove_dir_all(repo_root).unwrap();
    }

    #[test]
    fn local_commit_store_clamps_oversized_commit_page_limits() {
        let repo_root = create_temp_git_repo("commit-limit-clamp");
        for summary in ["first", "second", "third"] {
            fs::write(repo_root.join("history.txt"), format!("{summary}\n")).unwrap();
            git_in(&repo_root, &["add", "history.txt"]);
            git_in(&repo_root, &["commit", "-m", summary]);
        }
        let store = LocalCommitStore::new(HashMap::from([(
            "repo_temp".to_string(),
            repo_root.clone(),
        )]));

        let response = store
            .list_commits("repo_temp", 1_000, 0, None)
            .unwrap()
            .unwrap();

        assert_eq!(response.page_info.limit, MAX_COMMIT_PAGE_LIMIT);
        assert_eq!(response.commits.len(), 3);
        assert!(!response.page_info.has_next_page);

        fs::remove_dir_all(repo_root).unwrap();
    }

    #[test]
    fn local_commit_store_returns_none_for_unknown_repo_or_commit() {
        let store = LocalCommitStore::seeded();

        assert!(store
            .list_commits("missing", 20, 0, None)
            .unwrap()
            .is_none());
        assert_eq!(
            store
                .list_commits("repo_demo_docs", 20, 0, None)
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
            .get_commit("repo_sourcebot_rewrite", "HEAD~1")
            .unwrap()
            .is_none());
        assert!(store
            .get_commit("repo_sourcebot_rewrite", "HEAD^1")
            .unwrap()
            .is_none());
        assert!(store
            .get_commit("repo_sourcebot_rewrite", "--path-format=absolute")
            .unwrap()
            .is_none());
        assert!(store
            .get_commit("repo_sourcebot_rewrite", "")
            .unwrap()
            .is_none());
        assert!(store
            .get_commit("repo_sourcebot_rewrite", "HEAD\0--path-format=absolute")
            .unwrap()
            .is_none());
        assert!(store
            .get_commit_diff("repo_sourcebot_rewrite", "")
            .unwrap()
            .is_none());
        assert!(store
            .get_commit_diff("repo_sourcebot_rewrite", "HEAD\0--path-format=absolute")
            .unwrap()
            .is_none());
        assert!(store
            .list_commits(
                "repo_sourcebot_rewrite",
                20,
                0,
                Some("--path-format=absolute"),
            )
            .unwrap()
            .is_none());
        assert!(store
            .list_commits("repo_sourcebot_rewrite", 20, 0, Some(""))
            .unwrap()
            .is_none());
        assert!(store
            .list_commits(
                "repo_sourcebot_rewrite",
                20,
                0,
                Some("HEAD\0--path-format=absolute"),
            )
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
    fn parse_diff_name_status_accepts_copies() {
        let entries =
            parse_diff_name_status("fe7f21f\0C100\0src/original.rs\0src/copied.rs\0").unwrap();

        assert_eq!(
            entries,
            vec![RawDiffStatus {
                path: "src/copied.rs".to_string(),
                change_type: CommitDiffChangeType::Copied,
                old_path: Some("src/original.rs".to_string()),
            }]
        );
    }

    #[test]
    fn diff_metadata_parsers_reject_unsafe_paths() {
        for path in [
            "",
            "/absolute",
            "../parent",
            "nested/../parent",
            "windows\\path",
            "line\nbreak",
        ] {
            assert!(
                parse_diff_name_status(&format!("fe7f21f\0M\0{path}\0")).is_err(),
                "name-status parser should reject unsafe path {path:?}"
            );
            assert!(
                parse_diff_numstat(&format!("fe7f21f\01\t2\t{path}\0")).is_err(),
                "numstat parser should reject unsafe path {path:?}"
            );
        }

        for old_path in [
            "",
            "/absolute",
            "../parent",
            "nested/../parent",
            "windows\\path",
            "line\nbreak",
        ] {
            assert!(
                parse_diff_name_status(&format!("fe7f21f\0R100\0{old_path}\0safe/new.txt\0"))
                    .is_err(),
                "name-status parser should reject unsafe old path {old_path:?}"
            );
            assert!(
                parse_diff_numstat(&format!("fe7f21f\01\t2\t\0{old_path}\0safe/new.txt\0"))
                    .is_err(),
                "numstat parser should reject unsafe old path {old_path:?}"
            );
        }
    }

    #[test]
    fn normalize_patch_output_marks_binary_patches_as_unavailable() {
        let normalized = normalize_patch_output(
            "diff --git a/assets/logo.png b/assets/logo.png\nBinary files a/assets/logo.png and b/assets/logo.png differ\n",
        );

        assert_eq!(normalized.patch, None);
        assert!(!normalized.truncated);
    }

    #[test]
    fn read_stream_bounded_rejects_oversized_git_output() {
        let oversized_output = vec![b'x'; 17];

        let error = read_stream_bounded(std::io::Cursor::new(oversized_output), 16)
            .expect_err("oversized git output must fail closed instead of silently truncating");

        assert!(
            error.to_string().contains("exceeded 16 byte capture limit"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn normalize_patch_output_caps_large_textual_patches_with_marker() {
        let patch = format!(
            "diff --git a/large.txt b/large.txt\n{}",
            "+oversized line\n".repeat((COMMIT_DIFF_PATCH_MAX_BYTES / 16) + 128)
        );

        let normalized = normalize_patch_output(&patch);
        let normalized_patch = normalized
            .patch
            .expect("text patch should remain available");

        assert!(normalized.truncated);
        assert!(normalized_patch.len() <= COMMIT_DIFF_PATCH_MAX_BYTES);
        assert!(normalized_patch.contains(COMMIT_DIFF_PATCH_TRUNCATED_MARKER));
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
        assert!(!file.patch_truncated);

        fs::remove_dir_all(repo_root).unwrap();
    }

    #[test]
    fn local_commit_store_reports_copied_files() {
        let repo_root = create_temp_git_repo("copy-diff");
        write_text_file(&repo_root.join("original.txt"), "same content\n");
        git_in(&repo_root, &["add", "original.txt"]);
        git_in(&repo_root, &["commit", "-m", "init"]);

        fs::copy(repo_root.join("original.txt"), repo_root.join("copied.txt")).unwrap();
        git_in(&repo_root, &["add", "copied.txt"]);
        git_in(&repo_root, &["commit", "-m", "copy file"]);

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
        assert_eq!(file.path, "copied.txt");
        assert_eq!(file.old_path.as_deref(), Some("original.txt"));
        assert_eq!(file.change_type, CommitDiffChangeType::Copied);
        assert_eq!(file.additions, 0);
        assert_eq!(file.deletions, 0);
        let patch = file
            .patch
            .as_deref()
            .expect("copy metadata patch should be present");
        assert!(patch.contains("similarity index 100%"));
        assert!(patch.contains("copy from original.txt"));
        assert!(patch.contains("copy to copied.txt"));
        assert!(!file.patch_truncated);

        fs::remove_dir_all(repo_root).unwrap();
    }

    #[test]
    fn local_commit_store_loads_literal_pathspec_patches() {
        let repo_root = create_temp_git_repo("literal-pathspec-diff");
        write_text_file(&repo_root.join("literal*.txt"), "base star\n");
        write_text_file(&repo_root.join("literalA.txt"), "base a\n");
        git_in(&repo_root, &["add", "literal*.txt", "literalA.txt"]);
        git_in(&repo_root, &["commit", "-m", "init"]);

        write_text_file(&repo_root.join("literal*.txt"), "base star\nchanged star\n");
        write_text_file(&repo_root.join("literalA.txt"), "base a\nchanged a\n");
        git_in(&repo_root, &["add", "literal*.txt", "literalA.txt"]);
        git_in(&repo_root, &["commit", "-m", "change wildcard names"]);

        let commit_id = git_stdout_trimmed(&repo_root, &["rev-parse", "HEAD"]);
        let store = LocalCommitStore::new(HashMap::from([(
            "repo_temp".to_string(),
            repo_root.clone(),
        )]));

        let response = store
            .get_commit_diff("repo_temp", &commit_id)
            .unwrap()
            .unwrap();
        let wildcard_file = response
            .files
            .iter()
            .find(|file| file.path == "literal*.txt")
            .expect("diff should include wildcard-named file");
        let patch = wildcard_file
            .patch
            .as_deref()
            .expect("wildcard-named file should have an isolated text patch");

        assert!(patch.contains("diff --git a/literal*.txt b/literal*.txt"));
        assert!(patch.contains("+changed star"));
        assert!(
            !patch.contains("literalA.txt") && !patch.contains("+changed a"),
            "literal path patch must not include pathspec-glob matches: {patch}"
        );

        fs::remove_dir_all(repo_root).unwrap();
    }

    #[test]
    fn local_commit_store_caps_large_text_patch_payloads() {
        let repo_root = create_temp_git_repo("large-diff");
        write_text_file(&repo_root.join("large.txt"), "base\n");
        git_in(&repo_root, &["add", "large.txt"]);
        git_in(&repo_root, &["commit", "-m", "init"]);

        write_text_file(
            &repo_root.join("large.txt"),
            &format!("base\n{}\n", "x".repeat(COMMIT_DIFF_PATCH_MAX_BYTES + 1024)),
        );
        git_in(&repo_root, &["add", "large.txt"]);
        git_in(&repo_root, &["commit", "-m", "large diff"]);

        let commit_id = git_stdout_trimmed(&repo_root, &["rev-parse", "HEAD"]);
        let store = LocalCommitStore::new(HashMap::from([(
            "repo_temp".to_string(),
            repo_root.clone(),
        )]));

        let response = store
            .get_commit_diff("repo_temp", &commit_id)
            .unwrap()
            .unwrap();
        let file = response
            .files
            .iter()
            .find(|file| file.path == "large.txt")
            .expect("large diff should include changed file");
        let patch = file.patch.as_deref().expect("text patch should be present");

        assert!(file.patch_truncated);
        assert!(patch.len() <= COMMIT_DIFF_PATCH_MAX_BYTES);
        assert!(patch.contains(COMMIT_DIFF_PATCH_TRUNCATED_MARKER));

        fs::remove_dir_all(repo_root).unwrap();
    }

    #[test]
    fn local_commit_store_rejects_diff_metadata_over_file_limit() {
        let repo_root = create_temp_git_repo("many-diff-files");
        write_text_file(&repo_root.join("seed.txt"), "seed\n");
        git_in(&repo_root, &["add", "seed.txt"]);
        git_in(&repo_root, &["commit", "-m", "init"]);

        for index in 0..101 {
            write_text_file(&repo_root.join(format!("file-{index:03}.txt")), "added\n");
        }
        git_in(&repo_root, &["add", "."]);
        git_in(&repo_root, &["commit", "-m", "many files"]);

        let commit_id = git_stdout_trimmed(&repo_root, &["rev-parse", "HEAD"]);
        let store = LocalCommitStore::new(HashMap::from([(
            "repo_temp".to_string(),
            repo_root.clone(),
        )]));

        let error = store
            .get_commit_diff("repo_temp", &commit_id)
            .expect_err("oversized diff metadata must fail closed before per-file patch loading");

        assert!(
            error.to_string().contains("changed file limit"),
            "unexpected error: {error:#}"
        );

        fs::remove_dir_all(repo_root).unwrap();
    }

    #[test]
    fn local_commit_store_reads_real_commit_diff() {
        let store = LocalCommitStore::seeded();
        let commit_id = store
            .list_commits("repo_sourcebot_rewrite", 1, 0, None)
            .unwrap()
            .unwrap()
            .commits
            .into_iter()
            .next()
            .expect("seeded repository should expose at least one commit")
            .short_id;

        let response = store
            .get_commit_diff("repo_sourcebot_rewrite", &commit_id)
            .unwrap()
            .unwrap();

        assert_eq!(response.repo_id, "repo_sourcebot_rewrite");
        assert_eq!(response.commit_id.len(), 40);
        assert!(!response.files.is_empty());
        assert!(response.files.iter().all(|file| !file.path.is_empty()));
        assert!(response
            .files
            .iter()
            .all(|file| file.additions + file.deletions > 0));
        assert!(response.files.iter().any(|file| file.patch.is_some()));

        let changed_file = response
            .files
            .iter()
            .find(|file| file.change_type == CommitDiffChangeType::Modified)
            .or_else(|| response.files.first())
            .expect("seeded repository diff should expose at least one changed file");
        assert!(!changed_file.path.is_empty());
        assert!(changed_file.additions + changed_file.deletions > 0);
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
