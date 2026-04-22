use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
pub use sourcebot_models::AskCitation;
use sourcebot_models::{
    AskThread, AskThreadSummary, Connection, OrganizationState, Repository, RepositoryDetail,
    RepositorySummary, RepositorySyncJob, RepositorySyncJobStatus, ReviewAgentRun,
    ReviewAgentRunStatus,
};
use std::{
    collections::HashSet,
    path::{Component, Path},
};

pub const PROJECT_NAME: &str = "sourcebot-rewrite";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRepositoryResult {
    pub detail: RepositoryDetail,
    pub created: bool,
}

#[async_trait]
pub trait CatalogStore: Send + Sync {
    async fn list_repositories(&self) -> Result<Vec<RepositorySummary>>;
    async fn get_repository_detail(&self, repo_id: &str) -> Result<Option<RepositoryDetail>>;
    fn supports_local_repository_import(&self) -> bool {
        false
    }
    async fn import_local_repository(
        &self,
        connection: Connection,
        repo_path: &str,
    ) -> Result<ImportRepositoryResult>;
}

#[async_trait]
pub trait BootstrapStore: Send + Sync {
    async fn bootstrap_status(&self) -> Result<sourcebot_models::BootstrapStatus>;
    async fn bootstrap_state(&self) -> Result<Option<sourcebot_models::BootstrapState>>;
    async fn initialize_bootstrap(&self, state: sourcebot_models::BootstrapState) -> Result<()>;
}

#[async_trait]
pub trait LocalSessionStore: Send + Sync {
    async fn local_session(
        &self,
        session_id: &str,
    ) -> Result<Option<sourcebot_models::LocalSession>>;
    async fn persist_local_session_account(
        &self,
        account: sourcebot_models::LocalAccount,
    ) -> Result<()> {
        let _ = account;
        Ok(())
    }
    async fn store_local_session(&self, session: sourcebot_models::LocalSession) -> Result<()>;
    async fn delete_local_session(&self, session_id: &str) -> Result<bool>;
}

#[async_trait]
pub trait OrganizationStore: Send + Sync {
    async fn organization_state(&self) -> Result<sourcebot_models::OrganizationState>;
    async fn store_organization_state(
        &self,
        state: sourcebot_models::OrganizationState,
    ) -> Result<()>;
    async fn store_repository_sync_job(
        &self,
        job: sourcebot_models::RepositorySyncJob,
    ) -> Result<()>;
    async fn claim_next_repository_sync_job(
        &self,
        started_at: &str,
    ) -> Result<Option<RepositorySyncJob>>;
    async fn claim_and_complete_next_repository_sync_job(
        &self,
        started_at: &str,
        execute: fn(RepositorySyncJob) -> RepositorySyncJob,
    ) -> Result<Option<RepositorySyncJob>> {
        let Some(claimed_job) = self.claim_next_repository_sync_job(started_at).await? else {
            return Ok(None);
        };

        let completed_job = execute(claimed_job);
        self.store_repository_sync_job(completed_job.clone())
            .await?;
        Ok(Some(completed_job))
    }
    async fn claim_next_review_agent_run(&self) -> Result<Option<ReviewAgentRun>>;
    async fn complete_review_agent_run(&self, run_id: &str) -> Result<Option<ReviewAgentRun>>;
    async fn fail_review_agent_run(&self, run_id: &str) -> Result<Option<ReviewAgentRun>>;
}

pub fn store_repository_sync_job(state: &mut OrganizationState, job: RepositorySyncJob) {
    if let Some(existing_job) = state
        .repository_sync_jobs
        .iter_mut()
        .find(|existing_job| existing_job.id == job.id)
    {
        *existing_job = job;
    } else {
        state.repository_sync_jobs.push(job);
    }
}

pub fn claim_next_repository_sync_job(
    state: &mut OrganizationState,
    started_at: &str,
) -> Option<RepositorySyncJob> {
    let next_job_index = state
        .repository_sync_jobs
        .iter()
        .enumerate()
        .filter(|(_, job)| job.status == RepositorySyncJobStatus::Queued)
        .min_by(|(left_index, left_job), (right_index, right_job)| {
            left_job
                .queued_at
                .cmp(&right_job.queued_at)
                .then_with(|| left_index.cmp(right_index))
        })
        .map(|(index, _)| index)?;

    let job = state.repository_sync_jobs.get_mut(next_job_index)?;
    job.status = RepositorySyncJobStatus::Running;
    job.started_at = Some(started_at.to_owned());
    job.finished_at = None;
    job.error = None;
    Some(job.clone())
}

pub fn complete_repository_sync_job(
    job: &RepositorySyncJob,
    status: RepositorySyncJobStatus,
    finished_at: &str,
    error: Option<String>,
) -> RepositorySyncJob {
    let mut job = job.clone();
    job.status = status;
    job.finished_at = Some(finished_at.to_owned());
    job.error = error;
    job
}

pub fn claim_next_review_agent_run(state: &mut OrganizationState) -> Option<ReviewAgentRun> {
    let next_run_index = state
        .review_agent_runs
        .iter()
        .enumerate()
        .filter(|(_, run)| run.status == ReviewAgentRunStatus::Queued)
        .min_by(|(left_index, left_run), (right_index, right_run)| {
            left_run
                .created_at
                .cmp(&right_run.created_at)
                .then_with(|| left_index.cmp(right_index))
        })
        .map(|(index, _)| index)?;

    let run = state.review_agent_runs.get_mut(next_run_index)?;
    run.status = ReviewAgentRunStatus::Claimed;
    Some(run.clone())
}

pub fn complete_review_agent_run(
    state: &mut OrganizationState,
    run_id: &str,
) -> Option<ReviewAgentRun> {
    let run = state
        .review_agent_runs
        .iter_mut()
        .find(|run| run.id == run_id && run.status == ReviewAgentRunStatus::Claimed)?;
    run.status = ReviewAgentRunStatus::Completed;
    Some(run.clone())
}

pub fn fail_review_agent_run(
    state: &mut OrganizationState,
    run_id: &str,
) -> Option<ReviewAgentRun> {
    let run = state
        .review_agent_runs
        .iter_mut()
        .find(|run| run.id == run_id && run.status == ReviewAgentRunStatus::Claimed)?;
    run.status = ReviewAgentRunStatus::Failed;
    Some(run.clone())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetrievalToolDefinition {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RetrievalToolContext {
    pub active_repo_id: Option<String>,
    pub repo_scope: Vec<String>,
    pub visible_repo_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryTreeEntryKind {
    File,
    Dir,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositoryTreeEntry {
    pub name: String,
    pub path: String,
    pub kind: RepositoryTreeEntryKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositoryTree {
    pub repo_id: String,
    pub path: String,
    pub entries: Vec<RepositoryTreeEntry>,
}

#[async_trait]
pub trait TreeStore: Send + Sync {
    async fn get_tree(&self, repo_id: &str, path: &str) -> Result<Option<RepositoryTree>>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositoryBlob {
    pub repo_id: String,
    pub path: String,
    pub content: String,
    pub size_bytes: u64,
}

#[async_trait]
pub trait BlobStore: Send + Sync {
    async fn get_blob(&self, repo_id: &str, path: &str) -> Result<Option<RepositoryBlob>>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositoryGlob {
    pub repo_id: String,
    pub pattern: String,
    pub paths: Vec<String>,
}

#[async_trait]
pub trait GlobStore: Send + Sync {
    async fn glob_paths(&self, repo_id: &str, pattern: &str) -> Result<Option<RepositoryGlob>>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositoryGrepMatch {
    pub path: String,
    pub line_number: usize,
    pub line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositoryGrep {
    pub repo_id: String,
    pub query: String,
    pub matches: Vec<RepositoryGrepMatch>,
}

#[async_trait]
pub trait GrepStore: Send + Sync {
    async fn grep(&self, repo_id: &str, query: &str) -> Result<Option<RepositoryGrep>>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "tool", content = "payload", rename_all = "snake_case")]
pub enum RetrievalToolResult {
    ListRepos(ListReposResult),
    ListTree(ListTreeResult),
    ReadFile(ReadFileResult),
    Glob(GlobResult),
    Grep(GrepResult),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListReposResult {
    pub repositories: Vec<RepositorySummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListTreeResult {
    pub repo_id: String,
    pub path: String,
    pub entries: Vec<RepositoryTreeEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadFileResult {
    pub repo_id: String,
    pub path: String,
    pub content: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GlobResult {
    pub repo_id: String,
    pub pattern: String,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GrepMatch {
    pub path: String,
    pub line_number: usize,
    pub line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GrepResult {
    pub repo_id: String,
    pub query: String,
    pub matches: Vec<GrepMatch>,
}

#[async_trait]
pub trait RetrievalTool: Send + Sync {
    fn definition(&self) -> RetrievalToolDefinition;
    async fn run(
        &self,
        catalog: &dyn CatalogStore,
        trees: &dyn TreeStore,
        blobs: &dyn BlobStore,
        globs: &dyn GlobStore,
        greps: &dyn GrepStore,
        context: &RetrievalToolContext,
    ) -> Result<RetrievalToolResult>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ListReposTool;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListTreeTool {
    path: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadFileTool {
    path: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlobTool {
    pattern: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GrepTool {
    query: String,
}

impl ListTreeTool {
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}

impl ReadFileTool {
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}

impl GlobTool {
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
        }
    }
}

impl GrepTool {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
        }
    }
}

#[async_trait]
impl RetrievalTool for ListReposTool {
    fn definition(&self) -> RetrievalToolDefinition {
        RetrievalToolDefinition {
            name: "list_repos".into(),
            description: "List repositories available to the retrieval scope.".into(),
        }
    }

    async fn run(
        &self,
        catalog: &dyn CatalogStore,
        _trees: &dyn TreeStore,
        _blobs: &dyn BlobStore,
        _globs: &dyn GlobStore,
        _greps: &dyn GrepStore,
        context: &RetrievalToolContext,
    ) -> Result<RetrievalToolResult> {
        let repositories = catalog.list_repositories().await?;
        let scoped_repo_ids = scoped_repo_ids(context);

        let repositories = repositories
            .into_iter()
            .filter(|repository| {
                repo_is_visible(context, &repository.id)
                    && scoped_repo_ids
                        .iter()
                        .any(|repo_id| *repo_id == repository.id)
            })
            .collect();

        Ok(RetrievalToolResult::ListRepos(ListReposResult {
            repositories,
        }))
    }
}

#[async_trait]
impl RetrievalTool for ListTreeTool {
    fn definition(&self) -> RetrievalToolDefinition {
        RetrievalToolDefinition {
            name: "list_tree".into(),
            description:
                "List files and directories at a repository path inside the active retrieval scope."
                    .into(),
        }
    }

    async fn run(
        &self,
        _catalog: &dyn CatalogStore,
        trees: &dyn TreeStore,
        _blobs: &dyn BlobStore,
        _globs: &dyn GlobStore,
        _greps: &dyn GrepStore,
        context: &RetrievalToolContext,
    ) -> Result<RetrievalToolResult> {
        let active_repo_id = context
            .active_repo_id
            .as_deref()
            .ok_or_else(|| anyhow!("list_tree requires an active repository"))?;

        if !repo_is_visible(context, active_repo_id) {
            anyhow::bail!(
                "active repository {active_repo_id} is not visible to the retrieval context"
            );
        }

        if !repo_is_in_scope(context, active_repo_id) {
            anyhow::bail!("active repository {active_repo_id} is outside retrieval scope");
        }

        let tree = trees
            .get_tree(active_repo_id, &self.path)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "repository tree not found for repo {active_repo_id} at path {}",
                    self.path
                )
            })?;

        if tree.repo_id != active_repo_id {
            anyhow::bail!(
                "tree store returned repo {} for active repo {active_repo_id}",
                tree.repo_id
            );
        }

        if tree.path != self.path {
            anyhow::bail!(
                "tree store returned path {} for requested path {}",
                tree.path,
                self.path
            );
        }

        Ok(RetrievalToolResult::ListTree(ListTreeResult {
            repo_id: tree.repo_id,
            path: tree.path,
            entries: tree.entries,
        }))
    }
}

#[async_trait]
impl RetrievalTool for ReadFileTool {
    fn definition(&self) -> RetrievalToolDefinition {
        RetrievalToolDefinition {
            name: "read_file".into(),
            description:
                "Read a UTF-8 file at a repository path inside the active retrieval scope.".into(),
        }
    }

    async fn run(
        &self,
        _catalog: &dyn CatalogStore,
        _trees: &dyn TreeStore,
        blobs: &dyn BlobStore,
        _globs: &dyn GlobStore,
        _greps: &dyn GrepStore,
        context: &RetrievalToolContext,
    ) -> Result<RetrievalToolResult> {
        let active_repo_id = context
            .active_repo_id
            .as_deref()
            .ok_or_else(|| anyhow!("read_file requires an active repository"))?;

        if !repo_is_visible(context, active_repo_id) {
            anyhow::bail!(
                "active repository {active_repo_id} is not visible to the retrieval context"
            );
        }

        if !repo_is_in_scope(context, active_repo_id) {
            anyhow::bail!("active repository {active_repo_id} is outside retrieval scope");
        }

        validate_relative_repo_path(&self.path)?;

        let blob = blobs
            .get_blob(active_repo_id, &self.path)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "repository blob not found for repo {active_repo_id} at path {}",
                    self.path
                )
            })?;

        if blob.repo_id != active_repo_id {
            anyhow::bail!(
                "blob store returned repo {} for active repo {active_repo_id}",
                blob.repo_id
            );
        }

        if blob.path != self.path {
            anyhow::bail!(
                "blob store returned path {} for requested path {}",
                blob.path,
                self.path
            );
        }

        Ok(RetrievalToolResult::ReadFile(ReadFileResult {
            repo_id: blob.repo_id,
            path: blob.path,
            content: blob.content,
            size_bytes: blob.size_bytes,
        }))
    }
}

#[async_trait]
impl RetrievalTool for GlobTool {
    fn definition(&self) -> RetrievalToolDefinition {
        RetrievalToolDefinition {
            name: "glob".into(),
            description:
                "List repository file paths matching a glob pattern inside the active retrieval scope.".into(),
        }
    }

    async fn run(
        &self,
        _catalog: &dyn CatalogStore,
        _trees: &dyn TreeStore,
        _blobs: &dyn BlobStore,
        globs: &dyn GlobStore,
        _greps: &dyn GrepStore,
        context: &RetrievalToolContext,
    ) -> Result<RetrievalToolResult> {
        let active_repo_id = context
            .active_repo_id
            .as_deref()
            .ok_or_else(|| anyhow!("glob requires an active repository"))?;

        if !repo_is_visible(context, active_repo_id) {
            anyhow::bail!(
                "active repository {active_repo_id} is not visible to the retrieval context"
            );
        }

        if !repo_is_in_scope(context, active_repo_id) {
            anyhow::bail!("active repository {active_repo_id} is outside retrieval scope");
        }

        validate_relative_repo_path(&self.pattern)?;

        let glob = globs
            .glob_paths(active_repo_id, &self.pattern)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "repository glob not found for repo {active_repo_id} with pattern {}",
                    self.pattern
                )
            })?;

        if glob.repo_id != active_repo_id {
            anyhow::bail!(
                "glob store returned repo {} for active repo {active_repo_id}",
                glob.repo_id
            );
        }

        if glob.pattern != self.pattern {
            anyhow::bail!(
                "glob store returned pattern {} for requested pattern {}",
                glob.pattern,
                self.pattern
            );
        }

        for path in &glob.paths {
            validate_relative_repo_path(path)?;
        }

        Ok(RetrievalToolResult::Glob(GlobResult {
            repo_id: glob.repo_id,
            pattern: glob.pattern,
            paths: glob.paths,
        }))
    }
}

#[async_trait]
impl RetrievalTool for GrepTool {
    fn definition(&self) -> RetrievalToolDefinition {
        RetrievalToolDefinition {
            name: "grep".into(),
            description:
                "Search repository file contents for a literal query inside the active retrieval scope."
                    .into(),
        }
    }

    async fn run(
        &self,
        _catalog: &dyn CatalogStore,
        _trees: &dyn TreeStore,
        _blobs: &dyn BlobStore,
        _globs: &dyn GlobStore,
        greps: &dyn GrepStore,
        context: &RetrievalToolContext,
    ) -> Result<RetrievalToolResult> {
        let active_repo_id = context
            .active_repo_id
            .as_deref()
            .ok_or_else(|| anyhow!("grep requires an active repository"))?;

        if !repo_is_visible(context, active_repo_id) {
            anyhow::bail!(
                "active repository {active_repo_id} is not visible to the retrieval context"
            );
        }

        if !repo_is_in_scope(context, active_repo_id) {
            anyhow::bail!("active repository {active_repo_id} is outside retrieval scope");
        }

        validate_grep_query(&self.query)?;

        let grep = greps
            .grep(active_repo_id, &self.query)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "repository grep not found for repo {active_repo_id} with query {}",
                    self.query
                )
            })?;

        if grep.repo_id != active_repo_id {
            anyhow::bail!(
                "grep store returned repo {} for active repo {active_repo_id}",
                grep.repo_id
            );
        }

        if grep.query != self.query {
            anyhow::bail!(
                "grep store returned query {} for requested query {}",
                grep.query,
                self.query
            );
        }

        let matches = grep
            .matches
            .into_iter()
            .map(|entry| {
                validate_relative_repo_path(&entry.path)?;
                if entry.line_number == 0 {
                    anyhow::bail!(
                        "grep store returned invalid line number for path {}",
                        entry.path
                    );
                }
                if !entry.line.contains(&self.query) {
                    anyhow::bail!(
                        "grep store returned line without requested query for path {}",
                        entry.path
                    );
                }

                Ok(GrepMatch {
                    path: entry.path,
                    line_number: entry.line_number,
                    line: entry.line,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(RetrievalToolResult::Grep(GrepResult {
            repo_id: grep.repo_id,
            query: grep.query,
            matches,
        }))
    }
}

fn scoped_repo_ids(context: &RetrievalToolContext) -> Vec<&str> {
    if !context.repo_scope.is_empty() {
        context.repo_scope.iter().map(String::as_str).collect()
    } else if let Some(active_repo_id) = context.active_repo_id.as_deref() {
        vec![active_repo_id]
    } else {
        context
            .visible_repo_ids
            .iter()
            .map(String::as_str)
            .collect()
    }
}

fn repo_is_visible(context: &RetrievalToolContext, repo_id: &str) -> bool {
    context
        .visible_repo_ids
        .iter()
        .any(|visible_repo_id| visible_repo_id == repo_id)
}

pub fn visible_repo_ids_for_user(state: &OrganizationState, user_id: &str) -> Vec<String> {
    let member_organization_ids = state
        .memberships
        .iter()
        .filter(|membership| membership.user_id == user_id)
        .map(|membership| membership.organization_id.as_str())
        .collect::<HashSet<_>>();

    let mut visible_repo_ids = Vec::new();
    let mut seen_repo_ids = HashSet::new();

    for binding in &state.repo_permissions {
        if member_organization_ids.contains(binding.organization_id.as_str())
            && seen_repo_ids.insert(binding.repository_id.as_str())
        {
            visible_repo_ids.push(binding.repository_id.clone());
        }
    }

    visible_repo_ids
}

fn repo_is_in_scope(context: &RetrievalToolContext, repo_id: &str) -> bool {
    scoped_repo_ids(context)
        .into_iter()
        .any(|scoped_repo_id| scoped_repo_id == repo_id)
}

fn validate_relative_repo_path(path: &str) -> Result<()> {
    for component in Path::new(path).components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                anyhow::bail!("invalid relative path: {path}");
            }
        }
    }

    Ok(())
}

fn validate_grep_query(query: &str) -> Result<()> {
    if query.trim().is_empty() {
        anyhow::bail!("grep query must not be empty");
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskRequest {
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub repo_scope: Vec<String>,
    pub thread_id: Option<String>,
    pub previous_messages: Vec<sourcebot_models::AskMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskResponse {
    pub provider: String,
    pub model: Option<String>,
    pub answer: String,
    pub citations: Vec<AskCitation>,
}

#[async_trait]
pub trait AskThreadStore: Send + Sync {
    async fn create_thread(&self, thread: AskThread) -> Result<()>;
    async fn list_threads_for_user(&self, user_id: &str) -> Result<Vec<AskThreadSummary>>;
    async fn get_thread_for_user(
        &self,
        user_id: &str,
        thread_id: &str,
    ) -> Result<Option<AskThread>>;
    async fn get_thread_messages_for_user(
        &self,
        user_id: &str,
        thread_id: &str,
    ) -> Result<Option<Vec<sourcebot_models::AskMessage>>>;
    async fn get_thread_for_session_for_user(
        &self,
        user_id: &str,
        session_id: &str,
    ) -> Result<Option<AskThread>>;
    async fn update_thread_metadata_for_user(
        &self,
        user_id: &str,
        thread_id: &str,
        title: Option<&str>,
        visibility: Option<sourcebot_models::AskThreadVisibility>,
        updated_at: &str,
    ) -> Result<Option<AskThread>>;
    async fn append_message_for_user(
        &self,
        user_id: &str,
        thread_id: &str,
        message: sourcebot_models::AskMessage,
        updated_at: &str,
    ) -> Result<Option<AskThread>>;
    async fn update_message_for_user(
        &self,
        user_id: &str,
        thread_id: &str,
        message_id: &str,
        content: &str,
        updated_at: &str,
    ) -> Result<Option<AskThread>>;
    async fn replace_message_for_user(
        &self,
        user_id: &str,
        thread_id: &str,
        message_id: &str,
        message: sourcebot_models::AskMessage,
        updated_at: &str,
    ) -> Result<Option<AskThread>>;
    async fn delete_message_for_user(
        &self,
        user_id: &str,
        thread_id: &str,
        message_id: &str,
        updated_at: &str,
    ) -> Result<Option<AskThread>>;
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, request: &AskRequest) -> Result<AskResponse>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmProviderConfig {
    pub provider: String,
    pub model: Option<String>,
    pub api_base: Option<String>,
    pub api_key: Option<String>,
}

impl LlmProviderConfig {
    pub fn disabled() -> Self {
        Self {
            provider: "disabled".into(),
            model: None,
            api_base: None,
            api_key: None,
        }
    }

    pub fn stub(model: Option<String>) -> Self {
        Self {
            provider: "stub".into(),
            model,
            api_base: None,
            api_key: None,
        }
    }
}

pub fn build_llm_provider(config: LlmProviderConfig) -> Box<dyn LlmProvider> {
    match config.provider.as_str() {
        "stub" => Box::new(StubLlmProvider {
            model: config.model,
        }),
        "stub-citations" => Box::new(StubCitationsLlmProvider {
            model: config.model,
        }),
        _ => Box::new(DisabledLlmProvider {
            provider: config.provider,
        }),
    }
}

struct DisabledLlmProvider {
    provider: String,
}

#[async_trait]
impl LlmProvider for DisabledLlmProvider {
    async fn complete(&self, _request: &AskRequest) -> Result<AskResponse> {
        Err(anyhow!(
            "llm provider '{}' is disabled or not configured",
            self.provider
        ))
    }
}

struct StubLlmProvider {
    model: Option<String>,
}

#[async_trait]
impl LlmProvider for StubLlmProvider {
    async fn complete(&self, request: &AskRequest) -> Result<AskResponse> {
        let prior_context = if request.previous_messages.is_empty() {
            String::new()
        } else {
            let last_message = request
                .previous_messages
                .last()
                .map(|message| message.content.as_str())
                .unwrap_or("");
            format!(
                " continuing thread with {} prior messages (latest: '{}')",
                request.previous_messages.len(),
                last_message
            )
        };
        Ok(AskResponse {
            provider: "stub".into(),
            model: self.model.clone(),
            answer: format!(
                "stub response: no real provider configured yet for prompt '{}'{}",
                request.prompt, prior_context
            ),
            citations: Vec::new(),
        })
    }
}

struct StubCitationsLlmProvider {
    model: Option<String>,
}

#[async_trait]
impl LlmProvider for StubCitationsLlmProvider {
    async fn complete(&self, request: &AskRequest) -> Result<AskResponse> {
        let scoped_repo_id = request
            .repo_scope
            .first()
            .cloned()
            .unwrap_or_else(|| "repo_sourcebot_rewrite".into());
        Ok(AskResponse {
            provider: "stub-citations".into(),
            model: self.model.clone(),
            answer: format!(
                "stub citations response: no real provider configured yet for prompt '{}'",
                request.prompt
            ),
            citations: vec![
                AskCitation {
                    repo_id: scoped_repo_id,
                    path: "src/lib.rs".into(),
                    revision: "main".into(),
                    line_start: 10,
                    line_end: 12,
                },
                AskCitation {
                    repo_id: "repo_hidden".into(),
                    path: "secret.txt".into(),
                    revision: "deadbeef".into(),
                    line_start: 1,
                    line_end: 1,
                },
            ],
        })
    }
}

pub fn build_repository_detail(
    repositories: &[Repository],
    connections: &[Connection],
    repo_id: &str,
) -> Result<Option<RepositoryDetail>> {
    let Some(repository) = repositories.iter().find(|repo| repo.id == repo_id).cloned() else {
        return Ok(None);
    };

    let connection = connections
        .iter()
        .find(|conn| conn.id == repository.connection_id)
        .cloned()
        .ok_or_else(|| anyhow!("missing connection for repository {}", repository.id))?;

    Ok(Some(RepositoryDetail {
        repository,
        connection,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sourcebot_models::{
        OrganizationMembership, OrganizationRole, OrganizationState, RepositoryPermissionBinding,
        SyncState,
    };

    struct StaticCatalogStore {
        repositories: Vec<RepositorySummary>,
    }

    struct NullTreeStore;

    struct NullBlobStore;

    struct NullGlobStore;

    struct NullGrepStore;

    struct StaticTreeStore {
        tree: Option<RepositoryTree>,
    }

    struct StaticBlobStore {
        blob: Option<RepositoryBlob>,
    }

    struct StaticGlobStore {
        glob: Option<RepositoryGlob>,
    }

    struct StaticGrepStore {
        grep: Option<RepositoryGrep>,
    }

    #[async_trait]
    impl CatalogStore for StaticCatalogStore {
        async fn list_repositories(&self) -> Result<Vec<RepositorySummary>> {
            Ok(self.repositories.clone())
        }

        async fn get_repository_detail(&self, _repo_id: &str) -> Result<Option<RepositoryDetail>> {
            Ok(None)
        }

        async fn import_local_repository(
            &self,
            _connection: Connection,
            _repo_path: &str,
        ) -> Result<ImportRepositoryResult> {
            Err(anyhow!("static catalog store does not support imports"))
        }
    }

    #[async_trait]
    impl TreeStore for NullTreeStore {
        async fn get_tree(&self, _repo_id: &str, _path: &str) -> Result<Option<RepositoryTree>> {
            Ok(None)
        }
    }

    #[async_trait]
    impl BlobStore for NullBlobStore {
        async fn get_blob(&self, _repo_id: &str, _path: &str) -> Result<Option<RepositoryBlob>> {
            Ok(None)
        }
    }

    #[async_trait]
    impl GlobStore for NullGlobStore {
        async fn glob_paths(
            &self,
            _repo_id: &str,
            _pattern: &str,
        ) -> Result<Option<RepositoryGlob>> {
            Ok(None)
        }
    }

    #[async_trait]
    impl GrepStore for NullGrepStore {
        async fn grep(&self, _repo_id: &str, _query: &str) -> Result<Option<RepositoryGrep>> {
            Ok(None)
        }
    }

    #[async_trait]
    impl TreeStore for StaticTreeStore {
        async fn get_tree(&self, _repo_id: &str, _path: &str) -> Result<Option<RepositoryTree>> {
            Ok(self.tree.clone())
        }
    }

    #[async_trait]
    impl BlobStore for StaticBlobStore {
        async fn get_blob(&self, _repo_id: &str, _path: &str) -> Result<Option<RepositoryBlob>> {
            Ok(self.blob.clone())
        }
    }

    #[async_trait]
    impl GlobStore for StaticGlobStore {
        async fn glob_paths(
            &self,
            _repo_id: &str,
            _pattern: &str,
        ) -> Result<Option<RepositoryGlob>> {
            Ok(self.glob.clone())
        }
    }

    #[async_trait]
    impl GrepStore for StaticGrepStore {
        async fn grep(&self, _repo_id: &str, _query: &str) -> Result<Option<RepositoryGrep>> {
            Ok(self.grep.clone())
        }
    }

    #[test]
    fn list_repos_tool_definition_is_machine_readable() {
        let tool = ListReposTool;

        assert_eq!(
            tool.definition(),
            RetrievalToolDefinition {
                name: "list_repos".into(),
                description: "List repositories available to the retrieval scope.".into(),
            }
        );
    }

    #[test]
    fn list_tree_tool_definition_is_machine_readable() {
        let tool = ListTreeTool::new("src");

        assert_eq!(
            tool.definition(),
            RetrievalToolDefinition {
                name: "list_tree".into(),
                description: "List files and directories at a repository path inside the active retrieval scope.".into(),
            }
        );
    }

    #[test]
    fn read_file_tool_definition_is_machine_readable() {
        let tool = ReadFileTool::new("src/lib.rs");

        assert_eq!(
            tool.definition(),
            RetrievalToolDefinition {
                name: "read_file".into(),
                description:
                    "Read a UTF-8 file at a repository path inside the active retrieval scope."
                        .into(),
            }
        );
    }

    #[test]
    fn glob_tool_definition_is_machine_readable() {
        let tool = GlobTool::new("src/**/*.rs");

        assert_eq!(
            tool.definition(),
            RetrievalToolDefinition {
                name: "glob".into(),
                description:
                    "List repository file paths matching a glob pattern inside the active retrieval scope."
                        .into(),
            }
        );
    }

    #[test]
    fn grep_tool_definition_is_machine_readable() {
        let tool = GrepTool::new("needle");

        assert_eq!(
            tool.definition(),
            RetrievalToolDefinition {
                name: "grep".into(),
                description:
                    "Search repository file contents for a literal query inside the active retrieval scope."
                        .into(),
            }
        );
    }

    #[test]
    fn visible_repo_ids_for_user_collects_bound_repositories_for_member_organizations() {
        let state = OrganizationState {
            memberships: vec![
                OrganizationMembership {
                    organization_id: "org_acme".into(),
                    user_id: "user_1".into(),
                    role: OrganizationRole::Admin,
                    joined_at: "2026-04-21T00:00:00Z".into(),
                },
                OrganizationMembership {
                    organization_id: "org_beta".into(),
                    user_id: "user_1".into(),
                    role: OrganizationRole::Viewer,
                    joined_at: "2026-04-21T00:01:00Z".into(),
                },
                OrganizationMembership {
                    organization_id: "org_other".into(),
                    user_id: "user_2".into(),
                    role: OrganizationRole::Admin,
                    joined_at: "2026-04-21T00:02:00Z".into(),
                },
            ],
            repo_permissions: vec![
                RepositoryPermissionBinding {
                    organization_id: "org_beta".into(),
                    repository_id: "repo_docs".into(),
                    synced_at: "2026-04-21T00:03:00Z".into(),
                },
                RepositoryPermissionBinding {
                    organization_id: "org_acme".into(),
                    repository_id: "repo_sourcebot_rewrite".into(),
                    synced_at: "2026-04-21T00:04:00Z".into(),
                },
                RepositoryPermissionBinding {
                    organization_id: "org_other".into(),
                    repository_id: "repo_secret".into(),
                    synced_at: "2026-04-21T00:05:00Z".into(),
                },
            ],
            ..OrganizationState::default()
        };

        assert_eq!(
            visible_repo_ids_for_user(&state, "user_1"),
            vec![
                "repo_docs".to_string(),
                "repo_sourcebot_rewrite".to_string()
            ]
        );
    }

    #[test]
    fn visible_repo_ids_for_user_deduplicates_repositories_and_ignores_users_without_memberships() {
        let state = OrganizationState {
            memberships: vec![
                OrganizationMembership {
                    organization_id: "org_acme".into(),
                    user_id: "user_1".into(),
                    role: OrganizationRole::Admin,
                    joined_at: "2026-04-21T00:00:00Z".into(),
                },
                OrganizationMembership {
                    organization_id: "org_beta".into(),
                    user_id: "user_1".into(),
                    role: OrganizationRole::Viewer,
                    joined_at: "2026-04-21T00:01:00Z".into(),
                },
                OrganizationMembership {
                    organization_id: "org_acme".into(),
                    user_id: "user_1".into(),
                    role: OrganizationRole::Viewer,
                    joined_at: "2026-04-21T00:02:00Z".into(),
                },
            ],
            repo_permissions: vec![
                RepositoryPermissionBinding {
                    organization_id: "org_acme".into(),
                    repository_id: "repo_sourcebot_rewrite".into(),
                    synced_at: "2026-04-21T00:03:00Z".into(),
                },
                RepositoryPermissionBinding {
                    organization_id: "org_beta".into(),
                    repository_id: "repo_sourcebot_rewrite".into(),
                    synced_at: "2026-04-21T00:04:00Z".into(),
                },
                RepositoryPermissionBinding {
                    organization_id: "org_acme".into(),
                    repository_id: "repo_sourcebot_rewrite".into(),
                    synced_at: "2026-04-21T00:05:00Z".into(),
                },
            ],
            ..OrganizationState::default()
        };

        assert_eq!(
            visible_repo_ids_for_user(&state, "user_1"),
            vec!["repo_sourcebot_rewrite".to_string()]
        );
        assert!(visible_repo_ids_for_user(&state, "user_missing").is_empty());
    }

    #[tokio::test]
    async fn list_repos_tool_returns_only_visible_repositories_in_scope() {
        let tool = ListReposTool;
        let store = StaticCatalogStore {
            repositories: vec![
                RepositorySummary {
                    id: "repo_sourcebot_rewrite".into(),
                    name: "sourcebot-rewrite".into(),
                    default_branch: "main".into(),
                    sync_state: SyncState::Ready,
                },
                RepositorySummary {
                    id: "repo_demo_docs".into(),
                    name: "demo-docs".into(),
                    default_branch: "main".into(),
                    sync_state: SyncState::Pending,
                },
                RepositorySummary {
                    id: "repo_secret".into(),
                    name: "secret".into(),
                    default_branch: "main".into(),
                    sync_state: SyncState::Ready,
                },
            ],
        };

        let result = tool
            .run(
                &store,
                &NullTreeStore,
                &NullBlobStore,
                &NullGlobStore,
                &NullGrepStore,
                &RetrievalToolContext {
                    active_repo_id: Some("repo_sourcebot_rewrite".into()),
                    repo_scope: vec!["repo_sourcebot_rewrite".into(), "repo_demo_docs".into()],
                    visible_repo_ids: vec!["repo_sourcebot_rewrite".into(), "repo_secret".into()],
                },
            )
            .await
            .expect("list_repos should succeed");

        assert_eq!(
            result,
            RetrievalToolResult::ListRepos(ListReposResult {
                repositories: vec![RepositorySummary {
                    id: "repo_sourcebot_rewrite".into(),
                    name: "sourcebot-rewrite".into(),
                    default_branch: "main".into(),
                    sync_state: SyncState::Ready,
                }],
            })
        );
    }

    #[tokio::test]
    async fn list_repos_tool_falls_back_to_active_repo_when_scope_is_empty() {
        let tool = ListReposTool;
        let store = StaticCatalogStore {
            repositories: vec![
                RepositorySummary {
                    id: "repo_sourcebot_rewrite".into(),
                    name: "sourcebot-rewrite".into(),
                    default_branch: "main".into(),
                    sync_state: SyncState::Ready,
                },
                RepositorySummary {
                    id: "repo_demo_docs".into(),
                    name: "demo-docs".into(),
                    default_branch: "main".into(),
                    sync_state: SyncState::Pending,
                },
            ],
        };

        let result = tool
            .run(
                &store,
                &NullTreeStore,
                &NullBlobStore,
                &NullGlobStore,
                &NullGrepStore,
                &RetrievalToolContext {
                    active_repo_id: Some("repo_demo_docs".into()),
                    repo_scope: Vec::new(),
                    visible_repo_ids: vec![
                        "repo_sourcebot_rewrite".into(),
                        "repo_demo_docs".into(),
                    ],
                },
            )
            .await
            .expect("list_repos should use the active repo fallback");

        assert_eq!(
            result,
            RetrievalToolResult::ListRepos(ListReposResult {
                repositories: vec![RepositorySummary {
                    id: "repo_demo_docs".into(),
                    name: "demo-docs".into(),
                    default_branch: "main".into(),
                    sync_state: SyncState::Pending,
                }],
            })
        );
    }

    #[tokio::test]
    async fn list_tree_tool_returns_machine_readable_tree_for_active_repo() {
        let tool = ListTreeTool::new("src");
        let catalog = StaticCatalogStore {
            repositories: Vec::new(),
        };
        let trees = StaticTreeStore {
            tree: Some(RepositoryTree {
                repo_id: "repo_sourcebot_rewrite".into(),
                path: "src".into(),
                entries: vec![RepositoryTreeEntry {
                    name: "main.rs".into(),
                    path: "src/main.rs".into(),
                    kind: RepositoryTreeEntryKind::File,
                }],
            }),
        };

        let result = tool
            .run(
                &catalog,
                &trees,
                &NullBlobStore,
                &NullGlobStore,
                &NullGrepStore,
                &RetrievalToolContext {
                    active_repo_id: Some("repo_sourcebot_rewrite".into()),
                    repo_scope: vec!["repo_sourcebot_rewrite".into()],
                    visible_repo_ids: vec!["repo_sourcebot_rewrite".into(), "repo_secret".into()],
                },
            )
            .await
            .expect("list_tree should succeed for the active repository");

        assert_eq!(
            result,
            RetrievalToolResult::ListTree(ListTreeResult {
                repo_id: "repo_sourcebot_rewrite".into(),
                path: "src".into(),
                entries: vec![RepositoryTreeEntry {
                    name: "main.rs".into(),
                    path: "src/main.rs".into(),
                    kind: RepositoryTreeEntryKind::File,
                }],
            })
        );
    }

    #[tokio::test]
    async fn list_tree_tool_rejects_tree_store_metadata_outside_requested_scope() {
        let tool = ListTreeTool::new("src");
        let catalog = StaticCatalogStore {
            repositories: Vec::new(),
        };
        let trees = StaticTreeStore {
            tree: Some(RepositoryTree {
                repo_id: "repo_secret".into(),
                path: "other".into(),
                entries: vec![RepositoryTreeEntry {
                    name: "leak.txt".into(),
                    path: "other/leak.txt".into(),
                    kind: RepositoryTreeEntryKind::File,
                }],
            }),
        };

        let err = tool
            .run(
                &catalog,
                &trees,
                &NullBlobStore,
                &NullGlobStore,
                &NullGrepStore,
                &RetrievalToolContext {
                    active_repo_id: Some("repo_sourcebot_rewrite".into()),
                    repo_scope: vec!["repo_sourcebot_rewrite".into()],
                    visible_repo_ids: vec!["repo_sourcebot_rewrite".into()],
                },
            )
            .await
            .expect_err("list_tree should reject mismatched tree metadata");

        assert!(err.to_string().contains("tree store returned repo"));
    }

    #[tokio::test]
    async fn list_tree_tool_rejects_active_repo_outside_scope() {
        let tool = ListTreeTool::default();
        let catalog = StaticCatalogStore {
            repositories: Vec::new(),
        };

        let err = tool
            .run(
                &catalog,
                &NullTreeStore,
                &NullBlobStore,
                &NullGlobStore,
                &NullGrepStore,
                &RetrievalToolContext {
                    active_repo_id: Some("repo_secret".into()),
                    repo_scope: vec!["repo_sourcebot_rewrite".into()],
                    visible_repo_ids: vec!["repo_sourcebot_rewrite".into(), "repo_secret".into()],
                },
            )
            .await
            .expect_err("list_tree should fail when the active repo is outside scope");

        assert!(err.to_string().contains("outside retrieval scope"));
    }

    #[tokio::test]
    async fn read_file_tool_rejects_parent_directory_components_in_requested_path() {
        let tool = ReadFileTool::new("../secrets.txt");
        let catalog = StaticCatalogStore {
            repositories: Vec::new(),
        };

        let err = tool
            .run(
                &catalog,
                &NullTreeStore,
                &NullBlobStore,
                &NullGlobStore,
                &NullGrepStore,
                &RetrievalToolContext {
                    active_repo_id: Some("repo_sourcebot_rewrite".into()),
                    repo_scope: vec!["repo_sourcebot_rewrite".into()],
                    visible_repo_ids: vec!["repo_sourcebot_rewrite".into()],
                },
            )
            .await
            .expect_err("read_file should reject invalid relative paths");

        assert!(err.to_string().contains("invalid relative path"));
    }

    #[tokio::test]
    async fn read_file_tool_returns_machine_readable_blob_for_active_repo() {
        let tool = ReadFileTool::new("src/lib.rs");
        let catalog = StaticCatalogStore {
            repositories: Vec::new(),
        };
        let blobs = StaticBlobStore {
            blob: Some(RepositoryBlob {
                repo_id: "repo_sourcebot_rewrite".into(),
                path: "src/lib.rs".into(),
                content: "pub fn demo() {}\n".into(),
                size_bytes: 17,
            }),
        };

        let result = tool
            .run(
                &catalog,
                &NullTreeStore,
                &blobs,
                &NullGlobStore,
                &NullGrepStore,
                &RetrievalToolContext {
                    active_repo_id: Some("repo_sourcebot_rewrite".into()),
                    repo_scope: vec!["repo_sourcebot_rewrite".into()],
                    visible_repo_ids: vec!["repo_sourcebot_rewrite".into()],
                },
            )
            .await
            .expect("read_file should succeed for the active repository");

        assert_eq!(
            result,
            RetrievalToolResult::ReadFile(ReadFileResult {
                repo_id: "repo_sourcebot_rewrite".into(),
                path: "src/lib.rs".into(),
                content: "pub fn demo() {}\n".into(),
                size_bytes: 17,
            })
        );
    }

    #[tokio::test]
    async fn read_file_tool_rejects_blob_store_metadata_outside_requested_scope() {
        let tool = ReadFileTool::new("src/lib.rs");
        let catalog = StaticCatalogStore {
            repositories: Vec::new(),
        };
        let blobs = StaticBlobStore {
            blob: Some(RepositoryBlob {
                repo_id: "repo_secret".into(),
                path: "secrets.txt".into(),
                content: "do not leak\n".into(),
                size_bytes: 12,
            }),
        };

        let err = tool
            .run(
                &catalog,
                &NullTreeStore,
                &blobs,
                &NullGlobStore,
                &NullGrepStore,
                &RetrievalToolContext {
                    active_repo_id: Some("repo_sourcebot_rewrite".into()),
                    repo_scope: vec!["repo_sourcebot_rewrite".into()],
                    visible_repo_ids: vec!["repo_sourcebot_rewrite".into()],
                },
            )
            .await
            .expect_err("read_file should reject mismatched blob metadata");

        assert!(err.to_string().contains("blob store returned repo"));
    }

    #[tokio::test]
    async fn glob_tool_returns_machine_readable_matches_for_active_repo() {
        let tool = GlobTool::new("src/**/*.rs");
        let catalog = StaticCatalogStore {
            repositories: Vec::new(),
        };
        let globs = StaticGlobStore {
            glob: Some(RepositoryGlob {
                repo_id: "repo_sourcebot_rewrite".into(),
                pattern: "src/**/*.rs".into(),
                paths: vec!["src/lib.rs".into(), "src/main.rs".into()],
            }),
        };

        let result = tool
            .run(
                &catalog,
                &NullTreeStore,
                &NullBlobStore,
                &globs,
                &NullGrepStore,
                &RetrievalToolContext {
                    active_repo_id: Some("repo_sourcebot_rewrite".into()),
                    repo_scope: vec!["repo_sourcebot_rewrite".into()],
                    visible_repo_ids: vec!["repo_sourcebot_rewrite".into()],
                },
            )
            .await
            .expect("glob should succeed for the active repository");

        assert_eq!(
            result,
            RetrievalToolResult::Glob(GlobResult {
                repo_id: "repo_sourcebot_rewrite".into(),
                pattern: "src/**/*.rs".into(),
                paths: vec!["src/lib.rs".into(), "src/main.rs".into()],
            })
        );
    }

    #[tokio::test]
    async fn glob_tool_rejects_parent_directory_components_in_requested_pattern() {
        let tool = GlobTool::new("../*.rs");
        let catalog = StaticCatalogStore {
            repositories: Vec::new(),
        };

        let err = tool
            .run(
                &catalog,
                &NullTreeStore,
                &NullBlobStore,
                &StaticGlobStore { glob: None },
                &NullGrepStore,
                &RetrievalToolContext {
                    active_repo_id: Some("repo_sourcebot_rewrite".into()),
                    repo_scope: vec!["repo_sourcebot_rewrite".into()],
                    visible_repo_ids: vec!["repo_sourcebot_rewrite".into()],
                },
            )
            .await
            .expect_err("glob should reject invalid relative patterns");

        assert!(err.to_string().contains("invalid relative path"));
    }

    #[tokio::test]
    async fn glob_tool_rejects_glob_store_metadata_outside_requested_scope() {
        let tool = GlobTool::new("src/**/*.rs");
        let catalog = StaticCatalogStore {
            repositories: Vec::new(),
        };
        let globs = StaticGlobStore {
            glob: Some(RepositoryGlob {
                repo_id: "repo_secret".into(),
                pattern: "other/**/*.rs".into(),
                paths: vec!["other/leak.rs".into()],
            }),
        };

        let err = tool
            .run(
                &catalog,
                &NullTreeStore,
                &NullBlobStore,
                &globs,
                &NullGrepStore,
                &RetrievalToolContext {
                    active_repo_id: Some("repo_sourcebot_rewrite".into()),
                    repo_scope: vec!["repo_sourcebot_rewrite".into()],
                    visible_repo_ids: vec!["repo_sourcebot_rewrite".into()],
                },
            )
            .await
            .expect_err("glob should reject mismatched store metadata");

        assert!(err.to_string().contains("glob store returned repo"));
    }

    #[tokio::test]
    async fn grep_tool_returns_machine_readable_matches_for_active_repo() {
        let tool = GrepTool::new("needle");
        let catalog = StaticCatalogStore {
            repositories: Vec::new(),
        };
        let greps = StaticGrepStore {
            grep: Some(RepositoryGrep {
                repo_id: "repo_sourcebot_rewrite".into(),
                query: "needle".into(),
                matches: vec![
                    RepositoryGrepMatch {
                        path: "src/lib.rs".into(),
                        line_number: 3,
                        line: "const NEEDLE: &str = \"needle\";".into(),
                    },
                    RepositoryGrepMatch {
                        path: "src/main.rs".into(),
                        line_number: 8,
                        line: "println!(\"needle\");".into(),
                    },
                ],
            }),
        };

        let result = tool
            .run(
                &catalog,
                &NullTreeStore,
                &NullBlobStore,
                &NullGlobStore,
                &greps,
                &RetrievalToolContext {
                    active_repo_id: Some("repo_sourcebot_rewrite".into()),
                    repo_scope: vec!["repo_sourcebot_rewrite".into()],
                    visible_repo_ids: vec!["repo_sourcebot_rewrite".into()],
                },
            )
            .await
            .expect("grep should succeed for the active repository");

        assert_eq!(
            result,
            RetrievalToolResult::Grep(GrepResult {
                repo_id: "repo_sourcebot_rewrite".into(),
                query: "needle".into(),
                matches: vec![
                    GrepMatch {
                        path: "src/lib.rs".into(),
                        line_number: 3,
                        line: "const NEEDLE: &str = \"needle\";".into(),
                    },
                    GrepMatch {
                        path: "src/main.rs".into(),
                        line_number: 8,
                        line: "println!(\"needle\");".into(),
                    },
                ],
            })
        );
    }

    #[tokio::test]
    async fn grep_tool_rejects_empty_query() {
        let tool = GrepTool::new("   ");
        let catalog = StaticCatalogStore {
            repositories: Vec::new(),
        };

        let err = tool
            .run(
                &catalog,
                &NullTreeStore,
                &NullBlobStore,
                &NullGlobStore,
                &StaticGrepStore { grep: None },
                &RetrievalToolContext {
                    active_repo_id: Some("repo_sourcebot_rewrite".into()),
                    repo_scope: vec!["repo_sourcebot_rewrite".into()],
                    visible_repo_ids: vec!["repo_sourcebot_rewrite".into()],
                },
            )
            .await
            .expect_err("grep should reject empty queries");

        assert!(err.to_string().contains("query must not be empty"));
    }

    #[tokio::test]
    async fn grep_tool_rejects_grep_store_metadata_outside_requested_scope() {
        let tool = GrepTool::new("needle");
        let catalog = StaticCatalogStore {
            repositories: Vec::new(),
        };
        let greps = StaticGrepStore {
            grep: Some(RepositoryGrep {
                repo_id: "repo_secret".into(),
                query: "other".into(),
                matches: vec![RepositoryGrepMatch {
                    path: "../leak.txt".into(),
                    line_number: 0,
                    line: "totally unrelated".into(),
                }],
            }),
        };

        let err = tool
            .run(
                &catalog,
                &NullTreeStore,
                &NullBlobStore,
                &NullGlobStore,
                &greps,
                &RetrievalToolContext {
                    active_repo_id: Some("repo_sourcebot_rewrite".into()),
                    repo_scope: vec!["repo_sourcebot_rewrite".into()],
                    visible_repo_ids: vec!["repo_sourcebot_rewrite".into()],
                },
            )
            .await
            .expect_err("grep should reject mismatched grep metadata");

        assert!(err.to_string().contains("grep store returned repo"));
    }

    #[tokio::test]
    async fn disabled_provider_returns_actionable_error() {
        let provider = build_llm_provider(LlmProviderConfig::disabled());
        let response = provider
            .complete(&AskRequest {
                prompt: "where is healthz implemented?".into(),
                system_prompt: None,
                repo_scope: vec!["repo_sourcebot_rewrite".into()],
                thread_id: None,
                previous_messages: Vec::new(),
            })
            .await;

        let err = response.expect_err("disabled provider should fail closed");
        assert!(err.to_string().contains("disabled"));
    }

    #[tokio::test]
    async fn stub_provider_returns_answer_and_no_citations() {
        let provider = build_llm_provider(LlmProviderConfig::stub(Some("stub-model".into())));
        let response = provider
            .complete(&AskRequest {
                prompt: "where is healthz implemented?".into(),
                system_prompt: Some("answer with citations".into()),
                repo_scope: vec!["repo_sourcebot_rewrite".into()],
                thread_id: None,
                previous_messages: Vec::new(),
            })
            .await
            .expect("stub provider should succeed");

        assert_eq!(response.provider, "stub");
        assert_eq!(response.model.as_deref(), Some("stub-model"));
        assert!(response.answer.contains("stub response"));
        assert!(response.citations.is_empty());
    }
}
