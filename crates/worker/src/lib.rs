use anyhow::Result;
use sourcebot_core::{complete_repository_sync_job, OrganizationStore};
use sourcebot_models::{
    ConnectionConfig, OrganizationState, RepositorySyncJob, RepositorySyncJobStatus, ReviewAgentRun,
};
use std::{
    collections::HashMap,
    ffi::OsStr,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

const REPOSITORY_SYNC_STUB_FAILURE_ERROR: &str =
    "repository sync stub execution configured to fail";
const LOCAL_REPOSITORY_SYNC_PREFLIGHT_FAILURE_PREFIX: &str =
    "local repository sync preflight failed";
const LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX: &str =
    "local repository sync execution failed";
const GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX: &str =
    "generic Git repository sync execution failed";
const LOCAL_REPOSITORY_SYNC_PREFLIGHT_TIMEOUT: Duration = Duration::from_secs(10);
const LOCAL_REPOSITORY_SYNC_GIT_OUTPUT_LIMIT_BYTES: usize = 16 * 1024 * 1024;
const GENERIC_GIT_LS_REMOTE_OUTPUT_LIMIT_BYTES: usize = 1024 * 1024;
const GENERIC_GIT_FAILURE_DETAIL_LIMIT_BYTES: usize = 4096;
const LOCAL_REPOSITORY_SYNC_FAILURE_DETAIL_LIMIT_BYTES: usize = 4096;
const REPOSITORY_SYNC_RUNNING_JOB_LEASE_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const REPOSITORY_SYNC_STALE_RUNNING_RECOVERY_PREFIX: &str =
    "repository sync job exceeded worker lease and was marked failed before the next claim";
const REPOSITORY_SYNC_MALFORMED_RUNNING_LEASE_PREFIX: &str =
    "repository sync job had malformed running lease timestamp and was marked failed before the next claim";

pub use sourcebot_core::claim_next_review_agent_run;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerTickOutcome {
    ReviewAgentRun(ReviewAgentRun),
    RepositorySyncJob(RepositorySyncJob),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StubReviewAgentRunExecutionOutcome {
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StubRepositorySyncJobExecutionOutcome {
    Succeeded,
    Failed,
}

pub async fn run_worker_tick(
    store: &dyn OrganizationStore,
    review_agent_stub_outcome: StubReviewAgentRunExecutionOutcome,
    repository_sync_stub_outcome: StubRepositorySyncJobExecutionOutcome,
) -> Result<Option<WorkerTickOutcome>> {
    if let Some(run) = run_review_agent_tick(store, review_agent_stub_outcome).await? {
        return Ok(Some(WorkerTickOutcome::ReviewAgentRun(run)));
    }

    Ok(
        run_repository_sync_claim_tick(store, repository_sync_stub_outcome)
            .await?
            .map(WorkerTickOutcome::RepositorySyncJob),
    )
}

pub async fn run_review_agent_tick(
    store: &dyn OrganizationStore,
    stub_outcome: StubReviewAgentRunExecutionOutcome,
) -> Result<Option<ReviewAgentRun>> {
    let Some(claimed_run) = claim_next_review_agent_run_from_store(store).await? else {
        return Ok(None);
    };

    let stub_outcome = execute_claimed_review_agent_run_stub(&claimed_run, stub_outcome);
    persist_stub_review_agent_run_execution_outcome(store, &claimed_run.id, stub_outcome).await
}

pub async fn run_repository_sync_claim_tick(
    store: &dyn OrganizationStore,
    stub_outcome: StubRepositorySyncJobExecutionOutcome,
) -> Result<Option<RepositorySyncJob>> {
    let now = current_timestamp();
    recover_stale_running_repository_sync_jobs(store, &now).await?;
    requeue_old_repository_sync_lease_failures(store, &now).await?;
    let started_at = current_timestamp();
    let execute = match stub_outcome {
        StubRepositorySyncJobExecutionOutcome::Succeeded => {
            execute_claimed_repository_sync_job_succeeded_stub
        }
        StubRepositorySyncJobExecutionOutcome::Failed => {
            execute_claimed_repository_sync_job_failed_stub
        }
    };

    store
        .claim_and_complete_next_repository_sync_job(&started_at, execute)
        .await
}

pub fn execute_claimed_repository_sync_job_stub(job: RepositorySyncJob) -> RepositorySyncJob {
    execute_claimed_repository_sync_job_stub_at(
        &OrganizationState::default(),
        job,
        StubRepositorySyncJobExecutionOutcome::Succeeded,
        &current_timestamp(),
    )
}

pub fn execute_claimed_repository_sync_job_succeeded_stub(
    state: &OrganizationState,
    job: RepositorySyncJob,
) -> RepositorySyncJob {
    execute_claimed_repository_sync_job_stub_at(
        state,
        job,
        StubRepositorySyncJobExecutionOutcome::Succeeded,
        &current_timestamp(),
    )
}

pub fn execute_claimed_repository_sync_job_failed_stub(
    state: &OrganizationState,
    job: RepositorySyncJob,
) -> RepositorySyncJob {
    execute_claimed_repository_sync_job_stub_at(
        state,
        job,
        StubRepositorySyncJobExecutionOutcome::Failed,
        &current_timestamp(),
    )
}

pub fn execute_claimed_repository_sync_job_stub_at(
    state: &OrganizationState,
    job: RepositorySyncJob,
    stub_outcome: StubRepositorySyncJobExecutionOutcome,
    finished_at: &str,
) -> RepositorySyncJob {
    if let Some(generic_git_result) =
        complete_generic_git_repository_sync_job_if_applicable(state, &job, finished_at)
    {
        return generic_git_result;
    }

    if let Some(local_result) =
        complete_local_repository_sync_job_if_applicable(state, &job, finished_at)
    {
        return local_result;
    }

    match stub_outcome {
        StubRepositorySyncJobExecutionOutcome::Succeeded => complete_repository_sync_job(
            &job,
            RepositorySyncJobStatus::Succeeded,
            finished_at,
            None,
        ),
        StubRepositorySyncJobExecutionOutcome::Failed => complete_repository_sync_job(
            &job,
            RepositorySyncJobStatus::Failed,
            finished_at,
            Some(REPOSITORY_SYNC_STUB_FAILURE_ERROR.to_string()),
        ),
    }
}

fn complete_generic_git_repository_sync_job_if_applicable(
    state: &OrganizationState,
    job: &RepositorySyncJob,
    finished_at: &str,
) -> Option<RepositorySyncJob> {
    complete_generic_git_repository_sync_job_with_git_command_if_applicable(
        state,
        job,
        finished_at,
        OsStr::new("git"),
        LOCAL_REPOSITORY_SYNC_PREFLIGHT_TIMEOUT,
    )
}

fn complete_generic_git_repository_sync_job_with_git_command_if_applicable(
    state: &OrganizationState,
    job: &RepositorySyncJob,
    finished_at: &str,
    git_command: &OsStr,
    timeout: Duration,
) -> Option<RepositorySyncJob> {
    let connection = state
        .connections
        .iter()
        .find(|connection| connection.id == job.connection_id)?;
    let Some(ConnectionConfig::GenericGit { base_url }) = &connection.config else {
        return None;
    };

    match run_generic_git_repository_sync_execution(git_command, base_url, timeout) {
        Ok(execution) => {
            let mut completed_job = complete_repository_sync_job(
                job,
                RepositorySyncJobStatus::Succeeded,
                finished_at,
                None,
            );
            completed_job.synced_revision = Some(execution.revision);
            completed_job.synced_branch = Some(execution.branch);
            completed_job.synced_content_file_count = None;
            Some(completed_job)
        }
        Err(error) => Some(complete_repository_sync_job(
            job,
            RepositorySyncJobStatus::Failed,
            finished_at,
            Some(error),
        )),
    }
}

#[derive(Clone)]
struct GenericGitRepositorySyncExecution {
    revision: String,
    branch: String,
}

fn run_generic_git_repository_sync_execution(
    git_command: &OsStr,
    base_url: &str,
    timeout: Duration,
) -> Result<GenericGitRepositorySyncExecution, String> {
    validate_generic_git_base_url(base_url).map_err(|error| {
        format!("{GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: {error}")
    })?;

    let head_output = match run_git_ls_remote_head_symref(git_command, base_url, timeout) {
        Ok(Some(output)) => output,
        Ok(None) => {
            return Err(format!(
                "{GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git ls-remote --symref HEAD timed out after {}ms",
                timeout.as_millis()
            ))
        }
        Err(error) => {
            return Err(format!(
                "{GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: {error}"
            ))
        }
    };

    if generic_git_ls_remote_output_exceeded_limit(&head_output) {
        return Err(format!(
            "{GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git ls-remote --symref HEAD output exceeded {GENERIC_GIT_LS_REMOTE_OUTPUT_LIMIT_BYTES} bytes"
        ));
    }
    if !head_output.status.success() {
        return Err(generic_git_failure_detail(
            "git ls-remote --symref HEAD",
            &head_output,
        ));
    }
    let head_revision = parse_git_ls_remote_head_revision(&head_output.stdout)?;
    let head_symref_execution = match parse_git_ls_remote_head_symref(&head_output.stdout) {
        Ok(execution) => execution,
        Err(error) => return Err(error),
    };

    let output = match run_git_ls_remote_heads(git_command, base_url, timeout) {
        Ok(Some(output)) => output,
        Ok(None) => {
            return Err(format!(
                "{GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git ls-remote --heads timed out after {}ms",
                timeout.as_millis()
            ))
        }
        Err(error) => {
            return Err(format!(
                "{GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: {error}"
            ))
        }
    };

    if generic_git_ls_remote_output_exceeded_limit(&output) {
        return Err(format!(
            "{GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git ls-remote --heads output exceeded {GENERIC_GIT_LS_REMOTE_OUTPUT_LIMIT_BYTES} bytes"
        ));
    }
    if !output.status.success() {
        return Err(generic_git_failure_detail("git ls-remote --heads", &output));
    }

    let advertised_head = parse_git_ls_remote_heads(&output.stdout, head_revision.as_deref())?.ok_or_else(|| {
        format!(
            "{GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git ls-remote --heads advertised no valid branch refs"
        )
    })?;
    if let Some(head_symref_execution) = head_symref_execution {
        if !generic_git_heads_advertise_execution(&output.stdout, &head_symref_execution) {
            return Err(format!(
                "{GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git ls-remote --symref HEAD branch was not advertised by --heads"
            ));
        }
        return Ok(head_symref_execution);
    }

    Ok(advertised_head)
}

fn generic_git_heads_advertise_execution(
    stdout: &[u8],
    expected: &GenericGitRepositorySyncExecution,
) -> bool {
    let expected_ref = format!("refs/heads/{}", expected.branch);
    String::from_utf8_lossy(stdout).lines().any(|line| {
        let Some((revision, reference)) = line.split_once('\t') else {
            return false;
        };
        revision == expected.revision && reference == expected_ref
    })
}

fn parse_git_ls_remote_heads(
    stdout: &[u8],
    preferred_revision: Option<&str>,
) -> Result<Option<GenericGitRepositorySyncExecution>, String> {
    let mut first_valid_head = None;
    let mut preferred_head = None;
    let mut advertised_branches = HashMap::new();
    for line in String::from_utf8_lossy(stdout).lines() {
        if line.is_empty() {
            continue;
        }
        let Some((revision, reference)) = line.split_once('\t') else {
            return Err(format!(
                "{GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git ls-remote --heads advertised malformed ref line"
            ));
        };
        if reference.contains('\t') {
            return Err(format!(
                "{GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git ls-remote --heads advertised malformed ref line"
            ));
        }
        let Some(branch) = reference.strip_prefix("refs/heads/") else {
            return Err(format!(
                "{GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git ls-remote --heads advertised malformed ref line"
            ));
        };
        if !is_valid_git_object_id(revision) || !is_valid_git_branch_name(branch) {
            return Err(format!(
                "{GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git ls-remote --heads advertised malformed ref line"
            ));
        }

        if advertised_branches
            .insert(branch.to_owned(), revision.to_owned())
            .is_some()
        {
            return Err(format!(
                "{GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git ls-remote --heads advertised duplicate branch ref"
            ));
        }

        let execution = GenericGitRepositorySyncExecution {
            revision: revision.to_owned(),
            branch: branch.to_owned(),
        };
        if preferred_revision == Some(revision) {
            preferred_head.get_or_insert(execution.clone());
        }
        first_valid_head.get_or_insert(execution);
    }
    Ok(preferred_head.or(first_valid_head))
}

fn parse_git_ls_remote_head_revision(stdout: &[u8]) -> Result<Option<String>, String> {
    let mut revision = None;
    for line in String::from_utf8_lossy(stdout).lines() {
        if line.starts_with("ref: ") {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 2 || fields[1] != "HEAD" {
            continue;
        }
        if fields.len() != 2 || !is_valid_git_object_id(fields[0]) {
            return Err(generic_git_malformed_head_revision_failure());
        }
        if revision.is_some() {
            return Err(format!(
                "{GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git ls-remote --symref HEAD advertised ambiguous HEAD revision"
            ));
        }
        revision = Some(fields[0].to_owned());
    }

    Ok(revision)
}

fn parse_git_ls_remote_head_symref(
    stdout: &[u8],
) -> Result<Option<GenericGitRepositorySyncExecution>, String> {
    let mut branch = None;
    let mut revision = None;

    for line in String::from_utf8_lossy(stdout).lines() {
        if let Some(symref) = line.strip_prefix("ref: ") {
            let fields: Vec<&str> = symref.split('\t').collect();
            let [reference, target] = fields.as_slice() else {
                return Err(generic_git_malformed_head_symref_failure());
            };
            if *target != "HEAD" {
                return Err(generic_git_malformed_head_symref_failure());
            }
            let Some(branch_name) = reference.strip_prefix("refs/heads/") else {
                return Err(generic_git_malformed_head_symref_failure());
            };
            if !is_valid_git_branch_name(branch_name) || branch.is_some() {
                return Err(generic_git_malformed_head_symref_failure());
            }
            branch = Some(branch_name.to_owned());
            continue;
        }

        let Some((object_id, reference)) = line.split_once('\t') else {
            return Err(generic_git_malformed_head_metadata_failure());
        };
        if reference != "HEAD" {
            return Err(generic_git_malformed_head_metadata_failure());
        }
        if !is_valid_git_object_id(object_id) {
            return Err(generic_git_malformed_head_revision_failure());
        }
        if revision.is_some() {
            return Err(format!(
                "{GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git ls-remote --symref HEAD advertised ambiguous HEAD revision"
            ));
        }
        revision = Some(object_id.to_owned());
    }

    match (branch, revision) {
        (Some(branch), Some(revision)) => {
            Ok(Some(GenericGitRepositorySyncExecution { revision, branch }))
        }
        (Some(_), None) => Err(generic_git_incomplete_head_symref_failure()),
        (None, _) => Ok(None),
    }
}

fn generic_git_incomplete_head_symref_failure() -> String {
    format!(
        "{GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git ls-remote --symref HEAD advertised incomplete HEAD symref metadata"
    )
}

fn generic_git_malformed_head_revision_failure() -> String {
    format!(
        "{GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git ls-remote --symref HEAD advertised malformed HEAD revision"
    )
}

fn generic_git_malformed_head_symref_failure() -> String {
    format!(
        "{GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git ls-remote --symref HEAD advertised malformed HEAD symref"
    )
}

fn generic_git_malformed_head_metadata_failure() -> String {
    format!(
        "{GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git ls-remote --symref HEAD advertised malformed HEAD metadata"
    )
}

fn generic_git_ls_remote_output_exceeded_limit(output: &Output) -> bool {
    output.stdout.len() > GENERIC_GIT_LS_REMOTE_OUTPUT_LIMIT_BYTES
        || output.stderr.len() > GENERIC_GIT_LS_REMOTE_OUTPUT_LIMIT_BYTES
        || output.stdout.len().saturating_add(output.stderr.len())
            > GENERIC_GIT_LS_REMOTE_OUTPUT_LIMIT_BYTES
}

fn is_valid_git_object_id(revision: &str) -> bool {
    matches!(revision.len(), 40 | 64) && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_valid_git_branch_name(branch: &str) -> bool {
    !branch.is_empty()
        && branch != "@"
        && !branch.starts_with('-')
        && !branch.ends_with('.')
        && !branch.ends_with('/')
        && !branch.contains("..")
        && !branch.contains("@{")
        && !branch.contains("//")
        && branch.split('/').all(|component| {
            !component.is_empty() && !component.starts_with('.') && !component.ends_with(".lock")
        })
        && !branch.bytes().any(|byte| {
            byte <= 0x20
                || byte == 0x7f
                || matches!(byte, b'~' | b'^' | b':' | b'?' | b'*' | b'[' | b'\\')
        })
}

fn complete_local_repository_sync_job_if_applicable(
    state: &OrganizationState,
    job: &RepositorySyncJob,
    finished_at: &str,
) -> Option<RepositorySyncJob> {
    complete_local_repository_sync_job_with_git_command_if_applicable(
        state,
        job,
        finished_at,
        OsStr::new("git"),
        LOCAL_REPOSITORY_SYNC_PREFLIGHT_TIMEOUT,
    )
}

fn complete_local_repository_sync_job_with_git_command_if_applicable(
    state: &OrganizationState,
    job: &RepositorySyncJob,
    finished_at: &str,
    git_command: &OsStr,
    preflight_timeout: Duration,
) -> Option<RepositorySyncJob> {
    complete_local_repository_sync_job_with_git_command_and_output_limit_if_applicable(
        state,
        job,
        finished_at,
        git_command,
        preflight_timeout,
        LOCAL_REPOSITORY_SYNC_GIT_OUTPUT_LIMIT_BYTES,
    )
}

fn complete_local_repository_sync_job_with_git_command_and_output_limit_if_applicable(
    state: &OrganizationState,
    job: &RepositorySyncJob,
    finished_at: &str,
    git_command: &OsStr,
    preflight_timeout: Duration,
    output_limit_bytes: usize,
) -> Option<RepositorySyncJob> {
    let connection = state
        .connections
        .iter()
        .find(|connection| connection.id == job.connection_id)?;
    let Some(ConnectionConfig::Local { repo_path }) = &connection.config else {
        return None;
    };

    if let Err(error) =
        validate_non_blank_repository_sync_config_value("local repository repo_path", repo_path)
    {
        return Some(fail_local_repository_sync_job(job, finished_at, error));
    }

    let preflight = run_git_working_tree_preflight(
        git_command,
        repo_path,
        preflight_timeout,
        output_limit_bytes,
    );

    match preflight {
        Ok(Some(output)) if local_git_output_exceeded_limit(&output, output_limit_bytes) => {
            Some(fail_local_repository_sync_job(
                job,
                finished_at,
                format!("git preflight output exceeded {output_limit_bytes} bytes"),
            ))
        }
        Ok(Some(output))
            if output.status.success() && git_preflight_stdout_is_true(&output.stdout) =>
        {
            let execution = match run_local_repository_sync_execution(
                git_command,
                repo_path,
                job,
                preflight_timeout,
                output_limit_bytes,
            ) {
                Ok(execution) => execution,
                Err(execution_failure) => {
                    return Some(fail_local_repository_sync_job(
                        job,
                        finished_at,
                        execution_failure,
                    ));
                }
            };

            let mut completed_job = complete_repository_sync_job(
                job,
                RepositorySyncJobStatus::Succeeded,
                finished_at,
                None,
            );
            completed_job.synced_revision = Some(execution.revision);
            completed_job.synced_branch = Some(execution.branch);
            completed_job.synced_content_file_count = Some(execution.content_file_count);
            Some(completed_job)
        }
        Ok(Some(output)) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            let detail = if !stderr.is_empty() {
                stderr
            } else if output.status.success() && !stdout.eq_ignore_ascii_case("true") {
                format!("git preflight reported non-working-tree output {stdout:?}")
            } else {
                format!("git preflight exited with status {}", output.status)
            };
            Some(fail_local_repository_sync_job(job, finished_at, detail))
        }
        Ok(None) => Some(fail_local_repository_sync_job(
            job,
            finished_at,
            format!(
                "git preflight timed out after {}ms",
                preflight_timeout.as_millis()
            ),
        )),
        Err(error) => Some(fail_local_repository_sync_job(
            job,
            finished_at,
            error.to_string(),
        )),
    }
}

struct LocalRepositorySyncExecution {
    revision: String,
    branch: String,
    content_file_count: i64,
}

fn run_local_repository_sync_execution(
    git_command: &OsStr,
    repo_path: &str,
    job: &RepositorySyncJob,
    timeout: Duration,
    output_limit_bytes: usize,
) -> Result<LocalRepositorySyncExecution, String> {
    let head = match run_git_command_with_output_limit(
        git_command,
        repo_path,
        &["rev-parse", "HEAD"],
        timeout,
        output_limit_bytes,
    ) {
        Ok(Some(output)) if local_git_output_exceeded_limit(&output, output_limit_bytes) => {
            return Err(local_git_output_limit_failure_detail(
                "git rev-parse HEAD",
                output_limit_bytes,
            ));
        }
        Ok(Some(output)) if output.status.success() && !output.stdout.is_empty() => {
            let revision = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if !is_valid_git_object_id(&revision) {
                return Err(format!(
                    "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git rev-parse HEAD returned malformed revision"
                ));
            }
            revision
        }
        Ok(Some(output)) => return Err(git_failure_detail("git rev-parse HEAD", &output)),
        Ok(None) => {
            return Err(format!(
                "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git rev-parse HEAD timed out after {}ms",
                timeout.as_millis()
            ))
        }
        Err(error) => return Err(format!("{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: {error}")),
    };

    let content_paths = match run_git_command_with_output_limit(
        git_command,
        repo_path,
        &["ls-tree", "-rz", "--name-only", "HEAD"],
        timeout,
        output_limit_bytes,
    ) {
        Ok(Some(output)) if local_git_output_exceeded_limit(&output, output_limit_bytes) => {
            return Err(local_git_output_limit_failure_detail(
                "git ls-tree -rz --name-only HEAD",
                output_limit_bytes,
            ));
        }
        Ok(Some(output)) if output.status.success() && !output.stdout.is_empty() => {
            parse_nul_delimited_git_paths(&output.stdout)
        }
        Ok(Some(output)) if output.status.success() => {
            return Err(format!(
                "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git ls-tree -rz --name-only HEAD found no tracked content"
            ))
        }
        Ok(Some(output)) => return Err(git_failure_detail("git ls-tree -rz --name-only HEAD", &output)),
        Ok(None) => {
            return Err(format!(
                "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git ls-tree -rz --name-only HEAD timed out after {}ms",
                timeout.as_millis()
            ))
        }
        Err(error) => return Err(format!("{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: {error}")),
    };

    let branch = match run_git_command_with_output_limit(
        git_command,
        repo_path,
        &["symbolic-ref", "--short", "HEAD"],
        timeout,
        output_limit_bytes,
    ) {
        Ok(Some(output)) if local_git_output_exceeded_limit(&output, output_limit_bytes) => {
            return Err(local_git_output_limit_failure_detail(
                "git symbolic-ref --short HEAD",
                output_limit_bytes,
            ));
        }
        Ok(Some(output)) if output.status.success() && !output.stdout.is_empty() => {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if !is_valid_git_branch_name(&branch) {
                return Err(format!(
                    "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git symbolic-ref --short HEAD returned malformed branch name"
                ));
            }
            branch
        }
        Ok(Some(output)) => {
            resolve_detached_head_branch_label(git_command, repo_path, timeout, output_limit_bytes)
                .map_err(|_| git_failure_detail("git symbolic-ref --short HEAD", &output))?
        }
        Ok(None) => {
            return Err(format!(
                "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git symbolic-ref --short HEAD timed out after {}ms",
                timeout.as_millis()
            ));
        }
        Err(error) => {
            return Err(format!(
                "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: {error}"
            ))
        }
    };

    write_local_repository_sync_snapshot(
        git_command,
        repo_path,
        job,
        &content_paths,
        timeout,
        output_limit_bytes,
    )?;
    write_local_repository_sync_manifest(repo_path, job, &head, &branch, &content_paths)?;
    write_local_repository_sync_search_index(repo_path, job)?;
    tracing::info!(
        repo_path = %repo_path,
        head = %head,
        current_branch = %branch,
        content_file_count = content_paths.len(),
        "completed bounded local repository sync Git content snapshot"
    );
    Ok(LocalRepositorySyncExecution {
        revision: head,
        branch,
        content_file_count: content_paths.len() as i64,
    })
}

fn resolve_detached_head_branch_label(
    git_command: &OsStr,
    repo_path: &str,
    timeout: Duration,
    output_limit_bytes: usize,
) -> Result<String, String> {
    match run_git_command_with_output_limit(
        git_command,
        repo_path,
        &["rev-parse", "--abbrev-ref", "HEAD"],
        timeout,
        output_limit_bytes,
    ) {
        Ok(Some(output)) if local_git_output_exceeded_limit(&output, output_limit_bytes) => {
            Err(local_git_output_limit_failure_detail(
                "git rev-parse --abbrev-ref HEAD",
                output_limit_bytes,
            ))
        }
        Ok(Some(output)) if output.status.success() => {
            let label = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if label == "HEAD" {
                Ok(label)
            } else {
                Err(git_failure_detail("git rev-parse --abbrev-ref HEAD", &output))
            }
        }
        Ok(Some(output)) => Err(git_failure_detail("git rev-parse --abbrev-ref HEAD", &output)),
        Ok(None) => Err(format!(
            "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git rev-parse --abbrev-ref HEAD timed out after {}ms",
            timeout.as_millis()
        )),
        Err(error) => Err(format!("{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: {error}")),
    }
}

fn parse_nul_delimited_git_paths(output: &[u8]) -> Vec<String> {
    output
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).into_owned())
        .collect()
}

fn write_local_repository_sync_snapshot(
    git_command: &OsStr,
    repo_path: &str,
    job: &RepositorySyncJob,
    content_paths: &[String],
    timeout: Duration,
    output_limit_bytes: usize,
) -> Result<(), String> {
    let manifest_path = local_repository_sync_manifest_path(repo_path, job);
    let job_dir = manifest_path.parent().ok_or_else(|| {
        format!(
            "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: local sync manifest path has no parent"
        )
    })?;
    let snapshot_path = job_dir.join("snapshot");
    let tmp_path = job_dir.join("snapshot.tmp");
    if tmp_path.exists() {
        fs::remove_dir_all(&tmp_path).map_err(|error| {
            format!(
                "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: failed to remove stale local sync snapshot {}: {error}",
                tmp_path.display()
            )
        })?;
    }
    fs::create_dir_all(&tmp_path).map_err(|error| {
        format!(
            "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: failed to create local sync snapshot directory {}: {error}",
            tmp_path.display()
        )
    })?;

    for content_path in content_paths {
        let relative_path = match safe_tracked_content_relative_path(content_path) {
            Ok(path) => path,
            Err(error) => {
                let _ = fs::remove_dir_all(&tmp_path);
                return Err(error);
            }
        };
        let output = match run_git_command_with_output_limit(
            git_command,
            repo_path,
            &["show", &format!("HEAD:{content_path}")],
            timeout,
            output_limit_bytes,
        ) {
            Ok(Some(output)) if local_git_output_exceeded_limit(&output, output_limit_bytes) => {
                let _ = fs::remove_dir_all(&tmp_path);
                return Err(local_git_output_limit_failure_detail(
                    "git show HEAD:<tracked-path>",
                    output_limit_bytes,
                ));
            }
            Ok(Some(output)) if output.status.success() => output,
            Ok(Some(output)) => {
                let _ = fs::remove_dir_all(&tmp_path);
                return Err(git_failure_detail("git show HEAD:<tracked-path>", &output));
            }
            Ok(None) => {
                let _ = fs::remove_dir_all(&tmp_path);
                return Err(format!(
                    "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git show HEAD:{content_path} timed out after {}ms",
                    timeout.as_millis()
                ));
            }
            Err(error) => {
                let _ = fs::remove_dir_all(&tmp_path);
                return Err(format!(
                    "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: {error}"
                ));
            }
        };
        let destination = tmp_path.join(relative_path);
        if let Some(parent) = destination.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                let _ = fs::remove_dir_all(&tmp_path);
                return Err(format!(
                    "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: failed to create local sync snapshot parent {}: {error}",
                    parent.display()
                ));
            }
        }
        if let Err(error) = fs::write(&destination, output.stdout) {
            let _ = fs::remove_dir_all(&tmp_path);
            return Err(format!(
                "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: failed to write local sync snapshot file {}: {error}",
                destination.display()
            ));
        }
    }

    if snapshot_path.exists() {
        fs::remove_dir_all(&snapshot_path).map_err(|error| {
            format!(
                "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: failed to remove previous local sync snapshot {}: {error}",
                snapshot_path.display()
            )
        })?;
    }
    if let Err(error) = fs::rename(&tmp_path, &snapshot_path) {
        let _ = fs::remove_dir_all(&tmp_path);
        return Err(format!(
            "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: failed to finalize local sync snapshot {}: {error}",
            snapshot_path.display()
        ));
    }

    Ok(())
}

fn safe_tracked_content_relative_path(content_path: &str) -> Result<PathBuf, String> {
    let path = Path::new(content_path);
    if path
        .components()
        .all(|component| matches!(component, std::path::Component::Normal(_)))
        && !content_path
            .bytes()
            .any(|byte| byte != b'\t' && (byte <= 0x1f || byte == 0x7f))
    {
        Ok(path.to_path_buf())
    } else {
        Err(format!(
            "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: tracked content path cannot be snapshotted safely: {content_path:?}"
        ))
    }
}

fn write_local_repository_sync_search_index(
    repo_path: &str,
    job: &RepositorySyncJob,
) -> Result<(), String> {
    let manifest_path = local_repository_sync_manifest_path(repo_path, job);
    let job_dir = manifest_path.parent().ok_or_else(|| {
        format!(
            "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: local sync manifest path has no parent"
        )
    })?;
    let snapshot_path = job_dir.join("snapshot");
    let artifact_path = job_dir.join("search-index.json");
    let search_store = sourcebot_search::LocalSearchStore::new(HashMap::from([(
        job.repository_id.clone(),
        snapshot_path,
    )]));
    let index_status = sourcebot_search::SearchStore::repository_index_status(
        &search_store,
        &job.repository_id,
    )
    .map_err(|error| {
        format!(
            "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: failed to inspect local search index status: {error}"
        )
    })?
    .ok_or_else(|| {
        format!(
            "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: missing local search index status for repository {}",
            job.repository_id
        )
    })?;
    if !matches!(
        index_status.status,
        sourcebot_search::RepositoryIndexState::Indexed
            | sourcebot_search::RepositoryIndexState::IndexedEmpty
    ) {
        return Err(format!(
            "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: local search index artifact could not be built for repository {}: {}",
            job.repository_id,
            index_status
                .error
                .unwrap_or_else(|| "index status was not indexed".to_owned())
        ));
    }
    search_store
        .write_index_artifact(&job.repository_id, &artifact_path)
        .map_err(|error| {
            format!(
                "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: failed to persist local search index artifact {}: {error}",
                artifact_path.display()
            )
        })
}

fn write_local_repository_sync_manifest(
    repo_path: &str,
    job: &RepositorySyncJob,
    revision: &str,
    branch: &str,
    content_paths: &[String],
) -> Result<(), String> {
    let manifest_path = local_repository_sync_manifest_path(repo_path, job);
    let manifest_dir = manifest_path.parent().ok_or_else(|| {
        format!(
            "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: local sync manifest path has no parent"
        )
    })?;

    fs::create_dir_all(manifest_dir).map_err(|error| {
        format!(
            "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: failed to create local sync manifest directory {}: {error}",
            manifest_dir.display()
        )
    })?;

    let tmp_path = manifest_path.with_extension("txt.tmp");
    let manifest = local_repository_sync_manifest_content(revision, branch, content_paths);
    if let Err(error) = fs::write(&tmp_path, manifest) {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!(
            "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: failed to write local sync manifest {}: {error}",
            tmp_path.display()
        ));
    }
    if let Err(error) = fs::rename(&tmp_path, &manifest_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!(
            "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: failed to finalize local sync manifest {}: {error}",
            manifest_path.display()
        ));
    }

    Ok(())
}

async fn recover_stale_running_repository_sync_jobs(
    store: &dyn OrganizationStore,
    now: &str,
) -> Result<()> {
    let mut state = store.organization_state().await?;
    if fail_stale_running_repository_sync_jobs(&mut state, now) {
        store.store_organization_state(state).await?;
    }
    Ok(())
}

async fn requeue_old_repository_sync_lease_failures(
    store: &dyn OrganizationStore,
    now: &str,
) -> Result<()> {
    let mut state = store.organization_state().await?;
    if requeue_old_repository_sync_lease_failures_in_state(&mut state, now) {
        store.store_organization_state(state).await?;
    }
    Ok(())
}

fn requeue_old_repository_sync_lease_failures_in_state(
    state: &mut OrganizationState,
    now: &str,
) -> bool {
    let Ok(now_time) = OffsetDateTime::parse(now, &Rfc3339) else {
        return false;
    };
    let mut retry_jobs = Vec::new();
    for job in &state.repository_sync_jobs {
        if job.status != RepositorySyncJobStatus::Failed
            || job.id.contains("_auto_retry_")
            || !is_retryable_repository_sync_lease_failure(job)
        {
            continue;
        }
        let Some(finished_at) = job.finished_at.as_deref() else {
            continue;
        };
        let Ok(finished_at) = OffsetDateTime::parse(finished_at, &Rfc3339) else {
            continue;
        };
        if now_time - finished_at < REPOSITORY_SYNC_RUNNING_JOB_LEASE_TIMEOUT {
            continue;
        }
        let retry_id = format!("{}_auto_retry_1", job.id);
        let has_replacement_for_target = |existing: &RepositorySyncJob| {
            existing.id == retry_id
                || (existing.organization_id == job.organization_id
                    && existing.repository_id == job.repository_id
                    && existing.connection_id == job.connection_id
                    && (existing.id.contains("_auto_retry_")
                        || matches!(
                            existing.status,
                            RepositorySyncJobStatus::Queued | RepositorySyncJobStatus::Running
                        )))
        };
        if state
            .repository_sync_jobs
            .iter()
            .any(has_replacement_for_target)
            || retry_jobs.iter().any(has_replacement_for_target)
        {
            continue;
        }
        let mut retry = job.clone();
        retry.id = retry_id;
        retry.status = RepositorySyncJobStatus::Queued;
        retry.queued_at = now.to_string();
        retry.started_at = None;
        retry.finished_at = None;
        retry.error = None;
        retry.synced_revision = None;
        retry.synced_branch = None;
        retry.synced_content_file_count = None;
        retry_jobs.push(retry);
    }
    if retry_jobs.is_empty() {
        return false;
    }
    state.repository_sync_jobs.extend(retry_jobs);
    true
}

fn is_retryable_repository_sync_lease_failure(job: &RepositorySyncJob) -> bool {
    let error = job.error.as_deref().unwrap_or_default();
    error.starts_with(REPOSITORY_SYNC_STALE_RUNNING_RECOVERY_PREFIX)
        || error.starts_with(REPOSITORY_SYNC_MALFORMED_RUNNING_LEASE_PREFIX)
}

fn fail_stale_running_repository_sync_jobs(state: &mut OrganizationState, now: &str) -> bool {
    let Ok(now) = OffsetDateTime::parse(now, &Rfc3339) else {
        return false;
    };
    let mut changed = false;
    for job in &mut state.repository_sync_jobs {
        if job.status != RepositorySyncJobStatus::Running {
            continue;
        }
        let Some(started_at) = job.started_at.as_deref() else {
            mark_malformed_running_repository_sync_job_failed(job, &now, "missing started_at");
            changed = true;
            continue;
        };
        let Ok(started_at) = OffsetDateTime::parse(started_at, &Rfc3339) else {
            mark_malformed_running_repository_sync_job_failed(job, &now, "invalid started_at");
            changed = true;
            continue;
        };
        let age = now - started_at;
        if age < REPOSITORY_SYNC_RUNNING_JOB_LEASE_TIMEOUT {
            continue;
        }

        let finished_at = now
            .format(&Rfc3339)
            .expect("current UTC timestamp should format as RFC3339");
        job.status = RepositorySyncJobStatus::Failed;
        job.finished_at = Some(finished_at.clone());
        job.error = Some(format!(
            "{REPOSITORY_SYNC_STALE_RUNNING_RECOVERY_PREFIX} at {finished_at}"
        ));
        changed = true;
    }
    changed
}

fn mark_malformed_running_repository_sync_job_failed(
    job: &mut RepositorySyncJob,
    now: &OffsetDateTime,
    detail: &str,
) {
    let finished_at = now
        .format(&Rfc3339)
        .expect("current UTC timestamp should format as RFC3339");
    job.status = RepositorySyncJobStatus::Failed;
    job.finished_at = Some(finished_at.clone());
    job.error = Some(format!(
        "{REPOSITORY_SYNC_MALFORMED_RUNNING_LEASE_PREFIX}: {detail} at {finished_at}"
    ));
}

fn local_repository_sync_manifest_path(repo_path: &str, job: &RepositorySyncJob) -> PathBuf {
    Path::new(repo_path)
        .join(".sourcebot")
        .join("local-sync")
        .join(safe_manifest_path_component(&job.organization_id))
        .join(safe_manifest_path_component(&job.repository_id))
        .join(safe_manifest_path_component(&job.id))
        .join("manifest.txt")
}

fn safe_manifest_path_component(component: &str) -> String {
    let sanitized = component
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' => character,
            _ => '_',
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "_".to_owned()
    } else {
        sanitized
    }
}

fn local_repository_sync_manifest_content(
    revision: &str,
    branch: &str,
    content_paths: &[String],
) -> String {
    let mut manifest = format!(
        "revision={revision}\nbranch={branch}\ntracked_content_file_count={}\ntracked_content_paths:\n",
        content_paths.len()
    );
    for path in content_paths {
        manifest.push_str(path);
        manifest.push('\n');
    }
    manifest
}

fn git_failure_detail(command: &str, output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let detail = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("{command} exited with status {}", output.status)
    };
    let detail = bounded_local_repository_sync_failure_detail(&detail);
    format!("{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: {detail}")
}

fn bounded_local_repository_sync_failure_detail(detail: &str) -> String {
    bounded_failure_detail(detail, LOCAL_REPOSITORY_SYNC_FAILURE_DETAIL_LIMIT_BYTES)
}

fn local_git_output_exceeded_limit(output: &Output, output_limit_bytes: usize) -> bool {
    output.stdout.len() > output_limit_bytes
        || output.stderr.len() > output_limit_bytes
        || output.stdout.len().saturating_add(output.stderr.len()) > output_limit_bytes
}

fn local_git_output_limit_failure_detail(command: &str, output_limit_bytes: usize) -> String {
    format!(
        "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: {command} output exceeded {output_limit_bytes} bytes"
    )
}

fn generic_git_failure_detail(command: &str, output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let detail = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("{command} exited with status {}", output.status)
    };
    let detail = bounded_generic_git_failure_detail(&detail);
    format!("{GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: {detail}")
}

fn bounded_generic_git_failure_detail(detail: &str) -> String {
    bounded_failure_detail(detail, GENERIC_GIT_FAILURE_DETAIL_LIMIT_BYTES)
}

fn bounded_failure_detail(detail: &str, limit_bytes: usize) -> String {
    if detail.len() <= limit_bytes {
        return detail.to_owned();
    }

    let mut end = limit_bytes;
    while !detail.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...[truncated]", &detail[..end])
}

fn run_git_ls_remote_heads(
    git_command: &OsStr,
    base_url: &str,
    timeout: Duration,
) -> std::io::Result<Option<Output>> {
    let mut command = Command::new(git_command);
    configure_non_interactive_git_command(&mut command);
    let child = command
        .args(["ls-remote", "--heads", "--", base_url])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    wait_for_child_output_with_timeout_and_output_limit(
        child,
        timeout,
        GENERIC_GIT_LS_REMOTE_OUTPUT_LIMIT_BYTES,
    )
}

fn run_git_ls_remote_head_symref(
    git_command: &OsStr,
    base_url: &str,
    timeout: Duration,
) -> std::io::Result<Option<Output>> {
    let mut command = Command::new(git_command);
    configure_non_interactive_git_command(&mut command);
    let child = command
        .args(["ls-remote", "--symref", "--", base_url, "HEAD"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    wait_for_child_output_with_timeout_and_output_limit(
        child,
        timeout,
        GENERIC_GIT_LS_REMOTE_OUTPUT_LIMIT_BYTES,
    )
}

fn run_git_working_tree_preflight(
    git_command: &OsStr,
    repo_path: &str,
    timeout: Duration,
    output_limit_bytes: usize,
) -> std::io::Result<Option<Output>> {
    run_git_command_with_output_limit(
        git_command,
        repo_path,
        &["rev-parse", "--is-inside-work-tree"],
        timeout,
        output_limit_bytes,
    )
}

fn run_git_command_with_output_limit(
    git_command: &OsStr,
    repo_path: &str,
    args: &[&str],
    timeout: Duration,
    output_limit_bytes: usize,
) -> std::io::Result<Option<Output>> {
    let mut command = Command::new(git_command);
    configure_non_interactive_git_command(&mut command);
    let child = command
        .arg("-C")
        .arg(repo_path)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    wait_for_child_output_with_timeout_and_output_limit(child, timeout, output_limit_bytes)
}

fn configure_non_interactive_git_command(command: &mut Command) {
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .env("GIT_ASKPASS", "/bin/false")
        .env("SSH_ASKPASS", "/bin/false")
        .env("SSH_ASKPASS_REQUIRE", "never")
        .env(
            "GIT_SSH_COMMAND",
            "ssh -oBatchMode=yes -oNumberOfPasswordPrompts=0",
        );
}

fn wait_for_child_output_with_timeout_and_output_limit(
    mut child: std::process::Child,
    timeout: Duration,
    output_limit_bytes: usize,
) -> std::io::Result<Option<Output>> {
    let output_limit_exceeded = Arc::new(AtomicBool::new(false));
    let combined_output_bytes = Arc::new(AtomicUsize::new(0));
    let stdout_reader = child.stdout.take().map(|reader| {
        spawn_bounded_pipe_reader(
            reader,
            output_limit_bytes,
            Arc::clone(&output_limit_exceeded),
            Arc::clone(&combined_output_bytes),
        )
    });
    let stderr_reader = child.stderr.take().map(|reader| {
        spawn_bounded_pipe_reader(
            reader,
            output_limit_bytes,
            Arc::clone(&output_limit_exceeded),
            Arc::clone(&combined_output_bytes),
        )
    });
    let started_at = Instant::now();

    loop {
        if let Some(status) = child.try_wait()? {
            let stdout = join_pipe_reader(stdout_reader)?;
            let stderr = join_pipe_reader(stderr_reader)?;
            return Ok(Some(Output {
                status,
                stdout,
                stderr,
            }));
        }
        if output_limit_exceeded.load(Ordering::Relaxed) {
            let _ = child.kill();
            let status = child.wait()?;
            let stdout = join_pipe_reader(stdout_reader)?;
            let stderr = join_pipe_reader(stderr_reader)?;
            return Ok(Some(Output {
                status,
                stdout,
                stderr,
            }));
        }
        if started_at.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            let _ = join_pipe_reader(stdout_reader);
            let _ = join_pipe_reader(stderr_reader);
            return Ok(None);
        }
        let remaining = timeout.saturating_sub(started_at.elapsed());
        thread::sleep(remaining.min(Duration::from_millis(10)));
    }
}

fn spawn_bounded_pipe_reader<R>(
    mut reader: R,
    limit: usize,
    output_limit_exceeded: Arc<AtomicBool>,
    combined_output_bytes: Arc<AtomicUsize>,
) -> thread::JoinHandle<std::io::Result<Vec<u8>>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let retained_limit = limit.saturating_add(1);
        let mut output = Vec::with_capacity(retained_limit.min(8192));
        let mut buffer = [0_u8; 8192];
        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            let combined_after_read = combined_output_bytes
                .fetch_add(bytes_read, Ordering::Relaxed)
                .saturating_add(bytes_read);
            if combined_after_read > limit {
                output_limit_exceeded.store(true, Ordering::Relaxed);
            }
            let remaining_retained = retained_limit.saturating_sub(output.len());
            if remaining_retained > 0 {
                output.extend_from_slice(&buffer[..bytes_read.min(remaining_retained)]);
                if output.len() > limit {
                    output_limit_exceeded.store(true, Ordering::Relaxed);
                }
            } else {
                output_limit_exceeded.store(true, Ordering::Relaxed);
            }
        }
        Ok(output)
    })
}

fn join_pipe_reader(
    handle: Option<thread::JoinHandle<std::io::Result<Vec<u8>>>>,
) -> std::io::Result<Vec<u8>> {
    match handle {
        Some(handle) => handle
            .join()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "child pipe reader panicked"))?,
        None => Ok(Vec::new()),
    }
}

fn git_preflight_stdout_is_true(stdout: &[u8]) -> bool {
    String::from_utf8_lossy(stdout)
        .trim()
        .eq_ignore_ascii_case("true")
}

fn fail_local_repository_sync_job(
    job: &RepositorySyncJob,
    finished_at: &str,
    detail: String,
) -> RepositorySyncJob {
    let error = if detail.starts_with(LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX) {
        detail
    } else {
        format!("{LOCAL_REPOSITORY_SYNC_PREFLIGHT_FAILURE_PREFIX}: {detail}")
    };

    complete_repository_sync_job(
        job,
        RepositorySyncJobStatus::Failed,
        finished_at,
        Some(error),
    )
}

fn validate_non_blank_repository_sync_config_value(
    field_name: &str,
    value: &str,
) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{field_name} is empty"))
    } else {
        Ok(())
    }
}

fn validate_generic_git_base_url(base_url: &str) -> Result<(), String> {
    validate_non_blank_repository_sync_config_value("Generic Git base_url", base_url)?;

    if base_url.chars().any(char::is_control) {
        return Err("Generic Git base_url must not include control characters".to_owned());
    }

    let trimmed = base_url.trim();
    if base_url != trimmed {
        return Err("Generic Git base_url must not include surrounding whitespace".to_owned());
    }
    if trimmed.starts_with('-') {
        return Err("Generic Git base_url must not start with '-'".to_owned());
    }
    let Some((scheme, rest)) = trimmed.split_once("://") else {
        if generic_git_url_has_unsupported_transport_prefix(trimmed) {
            return Err("Generic Git base_url scheme is not supported".to_owned());
        }
        if generic_git_scp_like_url_contains_userinfo(trimmed) {
            return Err("Generic Git base_url must not include embedded credentials".to_owned());
        }
        return Ok(());
    };
    if !generic_git_url_scheme_is_supported(scheme) {
        return Err("Generic Git base_url scheme is not supported".to_owned());
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.is_empty() && !scheme.eq_ignore_ascii_case("file") {
        return Err("Generic Git base_url URL authority must not be empty".to_owned());
    }
    if authority.contains('@') || percent_decode_ascii(authority).contains('@') {
        Err("Generic Git base_url must not include embedded credentials".to_owned())
    } else {
        Ok(())
    }
}

fn generic_git_url_scheme_is_supported(scheme: &str) -> bool {
    matches!(
        scheme.to_ascii_lowercase().as_str(),
        "http" | "https" | "ssh" | "git" | "file"
    )
}

fn generic_git_url_has_unsupported_transport_prefix(value: &str) -> bool {
    let Some((prefix, _)) = value.split_once("::") else {
        return false;
    };
    !prefix.is_empty()
        && prefix
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'.' | b'-'))
}

fn generic_git_scp_like_url_contains_userinfo(value: &str) -> bool {
    let first_path_separator = value.find('/').unwrap_or(value.len());
    let prefix = &value[..first_path_separator];
    let Some(colon_index) = prefix.find(':') else {
        return false;
    };
    prefix[..colon_index].contains('@')
        || percent_decode_ascii(&prefix[..colon_index]).contains('@')
}

fn percent_decode_ascii(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = String::with_capacity(value.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) = (
                hex_digit_value(bytes[index + 1]),
                hex_digit_value(bytes[index + 2]),
            ) {
                decoded.push((high << 4 | low) as char);
                index += 3;
                continue;
            }
        }

        decoded.push(bytes[index] as char);
        index += 1;
    }

    decoded
}

fn hex_digit_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub fn execute_claimed_review_agent_run_stub(
    _run: &ReviewAgentRun,
    stub_outcome: StubReviewAgentRunExecutionOutcome,
) -> StubReviewAgentRunExecutionOutcome {
    stub_outcome
}

pub async fn claim_next_review_agent_run_from_store(
    store: &dyn OrganizationStore,
) -> Result<Option<ReviewAgentRun>> {
    store.claim_next_review_agent_run().await
}

pub async fn complete_review_agent_run_in_store(
    store: &dyn OrganizationStore,
    run_id: &str,
) -> Result<Option<ReviewAgentRun>> {
    store.complete_review_agent_run(run_id).await
}

pub async fn persist_stub_review_agent_run_execution_outcome(
    store: &dyn OrganizationStore,
    run_id: &str,
    outcome: StubReviewAgentRunExecutionOutcome,
) -> Result<Option<ReviewAgentRun>> {
    match outcome {
        StubReviewAgentRunExecutionOutcome::Completed => {
            complete_review_agent_run_in_store(store, run_id).await
        }
        StubReviewAgentRunExecutionOutcome::Failed => {
            fail_review_agent_run_in_store(store, run_id).await
        }
    }
}

pub async fn fail_review_agent_run_in_store(
    store: &dyn OrganizationStore,
    run_id: &str,
) -> Result<Option<ReviewAgentRun>> {
    store.fail_review_agent_run(run_id).await
}

pub async fn claim_next_repository_sync_job_from_store(
    store: &dyn OrganizationStore,
) -> Result<Option<RepositorySyncJob>> {
    claim_next_repository_sync_job_from_store_at(store, &current_timestamp()).await
}

pub async fn claim_next_repository_sync_job_from_store_at(
    store: &dyn OrganizationStore,
    started_at: &str,
) -> Result<Option<RepositorySyncJob>> {
    store.claim_next_repository_sync_job(started_at).await
}

fn current_timestamp() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("current UTC time should format as RFC3339")
}

#[cfg(test)]
mod tests {
    use super::{
        claim_next_repository_sync_job_from_store_at, claim_next_review_agent_run,
        claim_next_review_agent_run_from_store,
        complete_local_repository_sync_job_with_git_command_and_output_limit_if_applicable,
        complete_local_repository_sync_job_with_git_command_if_applicable,
        execute_claimed_repository_sync_job_stub_at, execute_claimed_review_agent_run_stub,
        git_failure_detail, local_repository_sync_manifest_path,
        persist_stub_review_agent_run_execution_outcome, run_generic_git_repository_sync_execution,
        run_repository_sync_claim_tick, run_review_agent_tick, run_worker_tick,
        safe_manifest_path_component, safe_tracked_content_relative_path,
        wait_for_child_output_with_timeout_and_output_limit, StubRepositorySyncJobExecutionOutcome,
        StubReviewAgentRunExecutionOutcome, WorkerTickOutcome,
    };
    use crate::{
        GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX,
        REPOSITORY_SYNC_MALFORMED_RUNNING_LEASE_PREFIX,
        REPOSITORY_SYNC_STALE_RUNNING_RECOVERY_PREFIX, REPOSITORY_SYNC_STUB_FAILURE_ERROR,
    };
    use anyhow::Result;
    use async_trait::async_trait;
    use sourcebot_api::auth::FileOrganizationStore;
    use sourcebot_core::OrganizationStore;
    use sourcebot_models::{
        Connection, ConnectionConfig, ConnectionKind, OrganizationState, RepositorySyncJob,
        RepositorySyncJobStatus, ReviewAgentRun, ReviewAgentRunStatus,
    };
    use sourcebot_search::SearchStore;
    use std::{
        ffi::OsStr,
        fs,
        os::unix::fs::PermissionsExt,
        process::{Command, Stdio},
        sync::Mutex,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };
    use time::{format_description::well_known::Rfc3339, OffsetDateTime};

    #[derive(Debug)]
    struct InMemoryOrganizationStore {
        state: Mutex<OrganizationState>,
    }

    #[derive(Debug)]
    struct FailingStoreOrganizationWriteStore {
        state: Mutex<OrganizationState>,
    }

    impl InMemoryOrganizationStore {
        fn new(state: OrganizationState) -> Self {
            Self {
                state: Mutex::new(state),
            }
        }
    }

    impl FailingStoreOrganizationWriteStore {
        fn new(state: OrganizationState) -> Self {
            Self {
                state: Mutex::new(state),
            }
        }
    }

    #[async_trait]
    impl OrganizationStore for InMemoryOrganizationStore {
        async fn organization_state(&self) -> Result<OrganizationState> {
            Ok(self.state.lock().unwrap().clone())
        }

        async fn store_organization_state(&self, state: OrganizationState) -> Result<()> {
            *self.state.lock().unwrap() = state;
            Ok(())
        }

        async fn store_repository_sync_job(&self, job: RepositorySyncJob) -> Result<()> {
            let mut state = self.state.lock().unwrap();
            sourcebot_core::store_repository_sync_job(&mut state, job);
            Ok(())
        }

        async fn claim_next_repository_sync_job(
            &self,
            started_at: &str,
        ) -> Result<Option<RepositorySyncJob>> {
            let mut state = self.state.lock().unwrap();
            Ok(sourcebot_core::claim_next_repository_sync_job(
                &mut state, started_at,
            ))
        }

        async fn claim_and_complete_next_repository_sync_job(
            &self,
            started_at: &str,
            execute: for<'state> fn(
                &'state OrganizationState,
                RepositorySyncJob,
            ) -> RepositorySyncJob,
        ) -> Result<Option<RepositorySyncJob>> {
            let mut state = self.state.lock().unwrap();
            let Some(claimed_job) =
                sourcebot_core::claim_next_repository_sync_job(&mut state, started_at)
            else {
                return Ok(None);
            };

            let completed_job = execute(&state, claimed_job);
            sourcebot_core::store_repository_sync_job(&mut state, completed_job.clone());
            Ok(Some(completed_job))
        }

        async fn claim_next_review_agent_run(&self) -> Result<Option<ReviewAgentRun>> {
            let mut state = self.state.lock().unwrap();
            Ok(claim_next_review_agent_run(&mut state))
        }

        async fn complete_review_agent_run(&self, run_id: &str) -> Result<Option<ReviewAgentRun>> {
            let mut state = self.state.lock().unwrap();
            Ok(sourcebot_core::complete_review_agent_run(
                &mut state, run_id,
            ))
        }

        async fn fail_review_agent_run(&self, run_id: &str) -> Result<Option<ReviewAgentRun>> {
            let mut state = self.state.lock().unwrap();
            Ok(sourcebot_core::fail_review_agent_run(&mut state, run_id))
        }
    }

    #[async_trait]
    impl OrganizationStore for FailingStoreOrganizationWriteStore {
        async fn organization_state(&self) -> Result<OrganizationState> {
            Ok(self.state.lock().unwrap().clone())
        }

        async fn store_organization_state(&self, _state: OrganizationState) -> Result<()> {
            Err(anyhow::anyhow!(
                "synthetic store_organization_state failure"
            ))
        }

        async fn store_repository_sync_job(&self, _job: RepositorySyncJob) -> Result<()> {
            panic!("run_repository_sync_claim_tick_at should not use store_repository_sync_job")
        }

        async fn claim_next_repository_sync_job(
            &self,
            _started_at: &str,
        ) -> Result<Option<RepositorySyncJob>> {
            panic!(
                "run_repository_sync_claim_tick should not use claim_next_repository_sync_job directly"
            )
        }

        async fn claim_and_complete_next_repository_sync_job(
            &self,
            _started_at: &str,
            _execute: for<'state> fn(
                &'state OrganizationState,
                RepositorySyncJob,
            ) -> RepositorySyncJob,
        ) -> Result<Option<RepositorySyncJob>> {
            Err(anyhow::anyhow!(
                "synthetic claim_and_complete_next_repository_sync_job failure"
            ))
        }

        async fn claim_next_review_agent_run(&self) -> Result<Option<ReviewAgentRun>> {
            Ok(None)
        }

        async fn complete_review_agent_run(&self, _run_id: &str) -> Result<Option<ReviewAgentRun>> {
            Ok(None)
        }

        async fn fail_review_agent_run(&self, _run_id: &str) -> Result<Option<ReviewAgentRun>> {
            Ok(None)
        }
    }

    fn unique_test_path(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("sourcebot-worker-{name}-{nanos}.json"))
    }

    fn review_agent_run(
        id: &str,
        status: ReviewAgentRunStatus,
        created_at: &str,
    ) -> ReviewAgentRun {
        ReviewAgentRun {
            id: id.into(),
            organization_id: "org_acme".into(),
            webhook_id: format!("webhook_{id}"),
            delivery_attempt_id: format!("delivery_{id}"),
            connection_id: "conn_github".into(),
            repository_id: "repo_sourcebot_rewrite".into(),
            review_id: format!("review_{id}"),
            status,
            created_at: created_at.into(),
        }
    }

    fn repository_sync_job(
        id: &str,
        status: RepositorySyncJobStatus,
        queued_at: &str,
    ) -> RepositorySyncJob {
        RepositorySyncJob {
            id: id.into(),
            organization_id: "org_acme".into(),
            repository_id: format!("repo_{id}"),
            connection_id: "conn_github".into(),
            status,
            queued_at: queued_at.into(),
            started_at: None,
            finished_at: None,
            error: None,
            synced_revision: None,
            synced_branch: None,
            synced_content_file_count: None,
        }
    }

    fn local_repository_sync_job(
        id: &str,
        status: RepositorySyncJobStatus,
        queued_at: &str,
    ) -> RepositorySyncJob {
        RepositorySyncJob {
            connection_id: "conn_local".into(),
            ..repository_sync_job(id, status, queued_at)
        }
    }

    fn local_connection(repo_path: impl Into<String>) -> Connection {
        Connection {
            id: "conn_local".into(),
            name: "Local fixture".into(),
            kind: ConnectionKind::Local,
            config: Some(ConnectionConfig::Local {
                repo_path: repo_path.into(),
            }),
        }
    }

    fn generic_git_repository_sync_job(
        id: &str,
        status: RepositorySyncJobStatus,
        queued_at: &str,
    ) -> RepositorySyncJob {
        RepositorySyncJob {
            connection_id: "conn_generic_git".into(),
            ..repository_sync_job(id, status, queued_at)
        }
    }

    fn generic_git_connection(base_url: impl Into<String>) -> Connection {
        Connection {
            id: "conn_generic_git".into(),
            name: "Generic Git fixture".into(),
            kind: ConnectionKind::GenericGit,
            config: Some(ConnectionConfig::GenericGit {
                base_url: base_url.into(),
            }),
        }
    }

    #[test]
    fn local_repository_sync_git_failure_detail_bounds_remote_stderr() {
        let output = Command::new("sh")
            .arg("-c")
            .arg("python3 -c 'import sys; sys.stderr.write(\"x\" * 9000)'; exit 1")
            .output()
            .expect("synthetic failing command should run");

        let detail = git_failure_detail("git rev-parse HEAD", &output);

        assert!(
            detail.starts_with("local repository sync execution failed: "),
            "local git failures should retain operator-visible prefix: {detail}"
        );
        assert!(
            detail.ends_with("...[truncated]"),
            "oversized local git failure detail should be explicitly truncated: {detail}"
        );
        assert!(
            detail.len() <= 4200,
            "local git failure detail should be bounded, got {} bytes",
            detail.len()
        );
    }

    #[test]
    fn local_sync_manifest_path_components_cannot_escape_manifest_root() {
        assert_eq!(safe_manifest_path_component("../org/acme"), "___org_acme");
        assert_eq!(safe_manifest_path_component("."), "_");
        assert_eq!(safe_manifest_path_component(".."), "__");
        assert_eq!(safe_manifest_path_component("org_acme-1"), "org_acme-1");
    }

    #[test]
    fn local_sync_tracked_content_paths_with_manifest_delimiters_fail_closed() {
        let error = safe_tracked_content_relative_path("safe\nname.txt")
            .expect_err("tracked paths with newlines must not be snapshotted or persisted");

        assert_eq!(
            error,
            "local repository sync execution failed: tracked content path cannot be snapshotted safely: \"safe\\nname.txt\""
        );
    }

    #[test]
    fn local_sync_tracked_content_paths_preserve_valid_tabs() {
        assert_eq!(
            safe_tracked_content_relative_path("dir/safe\tname.txt").expect(
                "tracked paths with tabs are valid Git paths and safe relative filesystem paths"
            ),
            std::path::PathBuf::from("dir/safe\tname.txt")
        );
    }

    #[test]
    fn claim_next_review_agent_run_claims_oldest_queued_run() {
        let mut state = OrganizationState {
            review_agent_runs: vec![
                review_agent_run(
                    "run_queued_newer",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:06Z",
                ),
                review_agent_run(
                    "run_claimed",
                    ReviewAgentRunStatus::Claimed,
                    "2026-04-25T00:10:04Z",
                ),
                review_agent_run(
                    "run_queued_oldest",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:05Z",
                ),
            ],
            ..OrganizationState::default()
        };

        let claimed_run =
            claim_next_review_agent_run(&mut state).expect("queued run to be claimed");

        assert_eq!(claimed_run.id, "run_queued_oldest");
        assert_eq!(claimed_run.status, ReviewAgentRunStatus::Claimed);
        assert_eq!(
            state.review_agent_runs[0].status,
            ReviewAgentRunStatus::Queued
        );
        assert_eq!(
            state.review_agent_runs[1].status,
            ReviewAgentRunStatus::Claimed
        );
        assert_eq!(
            state.review_agent_runs[2].status,
            ReviewAgentRunStatus::Claimed
        );
    }

    #[test]
    fn claim_next_review_agent_run_prefers_earlier_index_when_queued_timestamps_match() {
        let mut state = OrganizationState {
            review_agent_runs: vec![
                review_agent_run(
                    "run_first",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:05Z",
                ),
                review_agent_run(
                    "run_second",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:05Z",
                ),
            ],
            ..OrganizationState::default()
        };

        let claimed_run =
            claim_next_review_agent_run(&mut state).expect("queued run to be claimed");

        assert_eq!(claimed_run.id, "run_first");
        assert_eq!(
            state.review_agent_runs[0].status,
            ReviewAgentRunStatus::Claimed
        );
        assert_eq!(
            state.review_agent_runs[1].status,
            ReviewAgentRunStatus::Queued
        );
    }

    #[test]
    fn claim_next_review_agent_run_returns_none_when_no_queued_runs_exist() {
        let mut state = OrganizationState {
            review_agent_runs: vec![review_agent_run(
                "run_claimed",
                ReviewAgentRunStatus::Claimed,
                "2026-04-25T00:10:05Z",
            )],
            ..OrganizationState::default()
        };

        assert_eq!(claim_next_review_agent_run(&mut state), None);
    }

    #[test]
    fn execute_claimed_review_agent_run_stub_returns_the_requested_outcome() {
        let claimed_run = review_agent_run(
            "run_claimed",
            ReviewAgentRunStatus::Claimed,
            "2026-04-25T00:10:05Z",
        );

        let stub_outcome = execute_claimed_review_agent_run_stub(
            &claimed_run,
            StubReviewAgentRunExecutionOutcome::Failed,
        );

        assert_eq!(stub_outcome, StubReviewAgentRunExecutionOutcome::Failed);
    }

    #[tokio::test]
    async fn run_review_agent_tick_records_a_completed_run_in_the_file_store_after_stub_execution()
    {
        let path = unique_test_path("worker-tick-file-store");
        let store = FileOrganizationStore::new(&path);
        store
            .store_organization_state(OrganizationState {
                review_agent_runs: vec![
                    review_agent_run(
                        "run_queued_newer",
                        ReviewAgentRunStatus::Queued,
                        "2026-04-25T00:10:06Z",
                    ),
                    review_agent_run(
                        "run_queued_oldest",
                        ReviewAgentRunStatus::Queued,
                        "2026-04-25T00:10:05Z",
                    ),
                ],
                ..OrganizationState::default()
            })
            .await
            .unwrap();

        let completed_run =
            run_review_agent_tick(&store, StubReviewAgentRunExecutionOutcome::Completed)
                .await
                .unwrap()
                .expect("queued run to be completed");

        assert_eq!(completed_run.id, "run_queued_oldest");
        assert_eq!(completed_run.status, ReviewAgentRunStatus::Completed);

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(
            persisted.review_agent_runs[0].status,
            ReviewAgentRunStatus::Queued
        );
        assert_eq!(
            persisted.review_agent_runs[1].status,
            ReviewAgentRunStatus::Completed
        );

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn claim_next_review_agent_run_from_store_persists_the_claimed_run() {
        let store = InMemoryOrganizationStore::new(OrganizationState {
            review_agent_runs: vec![
                review_agent_run(
                    "run_queued_newer",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:06Z",
                ),
                review_agent_run(
                    "run_queued_oldest",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:05Z",
                ),
            ],
            ..OrganizationState::default()
        });

        let claimed_run = claim_next_review_agent_run_from_store(&store)
            .await
            .unwrap()
            .expect("queued run to be claimed");

        assert_eq!(claimed_run.id, "run_queued_oldest");
        assert_eq!(claimed_run.status, ReviewAgentRunStatus::Claimed);

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(
            persisted.review_agent_runs[0].status,
            ReviewAgentRunStatus::Queued
        );
        assert_eq!(
            persisted.review_agent_runs[1].status,
            ReviewAgentRunStatus::Claimed
        );
    }

    #[tokio::test]
    async fn run_review_agent_tick_records_a_failed_run_in_the_file_store_after_stub_execution() {
        let path = unique_test_path("worker-tick-file-store-failed");
        let store = FileOrganizationStore::new(&path);
        store
            .store_organization_state(OrganizationState {
                review_agent_runs: vec![
                    review_agent_run(
                        "run_queued_newer",
                        ReviewAgentRunStatus::Queued,
                        "2026-04-25T00:10:06Z",
                    ),
                    review_agent_run(
                        "run_queued_oldest",
                        ReviewAgentRunStatus::Queued,
                        "2026-04-25T00:10:05Z",
                    ),
                ],
                ..OrganizationState::default()
            })
            .await
            .unwrap();

        let failed_run = run_review_agent_tick(&store, StubReviewAgentRunExecutionOutcome::Failed)
            .await
            .unwrap()
            .expect("queued run to be failed");

        assert_eq!(failed_run.id, "run_queued_oldest");
        assert_eq!(failed_run.status, ReviewAgentRunStatus::Failed);

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(
            persisted.review_agent_runs[0].status,
            ReviewAgentRunStatus::Queued
        );
        assert_eq!(
            persisted.review_agent_runs[1].status,
            ReviewAgentRunStatus::Failed
        );

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn persist_stub_review_agent_run_execution_outcome_records_a_failed_run_when_requested() {
        let store = InMemoryOrganizationStore::new(OrganizationState {
            review_agent_runs: vec![
                review_agent_run(
                    "run_queued_newer",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:06Z",
                ),
                review_agent_run(
                    "run_queued_oldest",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:05Z",
                ),
            ],
            ..OrganizationState::default()
        });

        let claimed_run = claim_next_review_agent_run_from_store(&store)
            .await
            .unwrap()
            .expect("queued run to be claimed");
        assert_eq!(claimed_run.id, "run_queued_oldest");

        let failed_run = persist_stub_review_agent_run_execution_outcome(
            &store,
            &claimed_run.id,
            StubReviewAgentRunExecutionOutcome::Failed,
        )
        .await
        .unwrap()
        .expect("claimed run to be failed");

        assert_eq!(failed_run.id, "run_queued_oldest");
        assert_eq!(failed_run.status, ReviewAgentRunStatus::Failed);

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(
            persisted.review_agent_runs[0].status,
            ReviewAgentRunStatus::Queued
        );
        assert_eq!(
            persisted.review_agent_runs[1].status,
            ReviewAgentRunStatus::Failed
        );
    }

    #[tokio::test]
    async fn run_worker_tick_prioritizes_review_agent_work_before_repository_sync_claims() {
        let store = InMemoryOrganizationStore::new(OrganizationState {
            review_agent_runs: vec![review_agent_run(
                "run_queued_oldest",
                ReviewAgentRunStatus::Queued,
                "2026-04-25T00:10:05Z",
            )],
            repository_sync_jobs: vec![repository_sync_job(
                "sync_job_oldest",
                RepositorySyncJobStatus::Queued,
                "2026-04-26T10:01:00Z",
            )],
            ..OrganizationState::default()
        });

        let outcome = run_worker_tick(
            &store,
            StubReviewAgentRunExecutionOutcome::Completed,
            StubRepositorySyncJobExecutionOutcome::Succeeded,
        )
        .await
        .unwrap();

        assert_eq!(
            outcome,
            Some(WorkerTickOutcome::ReviewAgentRun(review_agent_run(
                "run_queued_oldest",
                ReviewAgentRunStatus::Completed,
                "2026-04-25T00:10:05Z",
            )))
        );

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(
            persisted.review_agent_runs[0].status,
            ReviewAgentRunStatus::Completed
        );
        assert_eq!(
            persisted.repository_sync_jobs[0].status,
            RepositorySyncJobStatus::Queued
        );
        assert_eq!(persisted.repository_sync_jobs[0].started_at, None);
    }

    #[test]
    fn execute_claimed_repository_sync_job_stub_records_a_succeeded_terminal_outcome() {
        let mut claimed_job = repository_sync_job(
            "sync_job_claimed",
            RepositorySyncJobStatus::Running,
            "2026-04-26T10:01:00Z",
        );
        claimed_job.started_at = Some("2026-04-26T10:03:00Z".into());

        let stubbed_job = execute_claimed_repository_sync_job_stub_at(
            &OrganizationState::default(),
            claimed_job.clone(),
            StubRepositorySyncJobExecutionOutcome::Succeeded,
            "2026-04-26T10:04:00Z",
        );

        assert_eq!(stubbed_job.id, claimed_job.id);
        assert_eq!(stubbed_job.status, RepositorySyncJobStatus::Succeeded);
        assert_eq!(
            stubbed_job.started_at.as_deref(),
            Some("2026-04-26T10:03:00Z")
        );
        assert_eq!(
            stubbed_job.finished_at.as_deref(),
            Some("2026-04-26T10:04:00Z")
        );
        assert_eq!(stubbed_job.error, None);
    }

    #[test]
    fn execute_claimed_repository_sync_job_stub_records_a_failed_terminal_outcome_when_requested() {
        let mut claimed_job = repository_sync_job(
            "sync_job_claimed",
            RepositorySyncJobStatus::Running,
            "2026-04-26T10:01:00Z",
        );
        claimed_job.started_at = Some("2026-04-26T10:03:00Z".into());

        let stubbed_job = execute_claimed_repository_sync_job_stub_at(
            &OrganizationState::default(),
            claimed_job.clone(),
            StubRepositorySyncJobExecutionOutcome::Failed,
            "2026-04-26T10:04:00Z",
        );

        assert_eq!(stubbed_job.id, claimed_job.id);
        assert_eq!(stubbed_job.status, RepositorySyncJobStatus::Failed);
        assert_eq!(
            stubbed_job.started_at.as_deref(),
            Some("2026-04-26T10:03:00Z")
        );
        assert_eq!(
            stubbed_job.finished_at.as_deref(),
            Some("2026-04-26T10:04:00Z")
        );
        assert_eq!(
            stubbed_job.error.as_deref(),
            Some("repository sync stub execution configured to fail")
        );
    }

    #[tokio::test]
    async fn claim_next_repository_sync_job_from_store_persists_the_oldest_running_job() {
        let store = InMemoryOrganizationStore::new(OrganizationState {
            repository_sync_jobs: vec![
                repository_sync_job(
                    "sync_job_newer",
                    RepositorySyncJobStatus::Queued,
                    "2026-04-26T10:02:00Z",
                ),
                repository_sync_job(
                    "sync_job_oldest",
                    RepositorySyncJobStatus::Queued,
                    "2026-04-26T10:01:00Z",
                ),
            ],
            ..OrganizationState::default()
        });

        let claimed_job =
            claim_next_repository_sync_job_from_store_at(&store, "2026-04-26T10:03:00Z")
                .await
                .unwrap()
                .expect("queued repository sync job to be claimed");

        assert_eq!(claimed_job.id, "sync_job_oldest");
        assert_eq!(claimed_job.status, RepositorySyncJobStatus::Running);
        assert_eq!(
            claimed_job.started_at.as_deref(),
            Some("2026-04-26T10:03:00Z")
        );

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(
            persisted.repository_sync_jobs[0].status,
            RepositorySyncJobStatus::Queued
        );
        assert_eq!(persisted.repository_sync_jobs[1], claimed_job);
    }

    #[tokio::test]
    async fn run_repository_sync_claim_tick_recovers_stale_running_jobs_before_claiming_next() {
        let store = InMemoryOrganizationStore::new(OrganizationState {
            repository_sync_jobs: vec![
                {
                    let mut job = repository_sync_job(
                        "sync_job_stale_running",
                        RepositorySyncJobStatus::Running,
                        "2026-04-26T09:00:00Z",
                    );
                    job.started_at = Some("2026-04-26T09:05:00Z".into());
                    job
                },
                {
                    let mut job = repository_sync_job(
                        "sync_job_fresh_running",
                        RepositorySyncJobStatus::Running,
                        "2099-04-26T09:00:00Z",
                    );
                    job.started_at = Some("2099-04-26T09:05:00Z".into());
                    job
                },
                repository_sync_job(
                    "sync_job_queued",
                    RepositorySyncJobStatus::Queued,
                    "2026-04-26T10:01:00Z",
                ),
            ],
            ..OrganizationState::default()
        });

        let completed_job = run_repository_sync_claim_tick(
            &store,
            StubRepositorySyncJobExecutionOutcome::Succeeded,
        )
        .await
        .unwrap()
        .expect("queued repository sync job to be completed after stale recovery");

        assert_eq!(completed_job.id, "sync_job_queued");
        assert_eq!(completed_job.status, RepositorySyncJobStatus::Succeeded);

        let persisted = store.organization_state().await.unwrap();
        let stale_job = persisted
            .repository_sync_jobs
            .iter()
            .find(|job| job.id == "sync_job_stale_running")
            .expect("stale running job should remain in history");
        assert_eq!(stale_job.status, RepositorySyncJobStatus::Failed);
        assert_eq!(
            stale_job.finished_at.as_deref(),
            Some(
                stale_job
                    .error
                    .as_deref()
                    .unwrap()
                    .split(" at ")
                    .last()
                    .unwrap_or_default()
            )
        );
        assert!(
            stale_job
                .error
                .as_deref()
                .unwrap_or_default()
                .starts_with("repository sync job exceeded worker lease and was marked failed before the next claim"),
            "stale job should carry operator-visible recovery detail: {stale_job:?}"
        );

        let fresh_job = persisted
            .repository_sync_jobs
            .iter()
            .find(|job| job.id == "sync_job_fresh_running")
            .expect("fresh running job should remain in history");
        assert_eq!(fresh_job.status, RepositorySyncJobStatus::Running);
        assert_eq!(fresh_job.finished_at, None);
        assert_eq!(fresh_job.error, None);
    }

    #[tokio::test]
    async fn run_repository_sync_claim_tick_fails_malformed_running_started_at_before_claiming() {
        let store = InMemoryOrganizationStore::new(OrganizationState {
            repository_sync_jobs: vec![
                {
                    let mut job = repository_sync_job(
                        "sync_job_malformed_running",
                        RepositorySyncJobStatus::Running,
                        "2026-04-26T09:00:00Z",
                    );
                    job.started_at = Some("not-rfc3339".into());
                    job
                },
                repository_sync_job(
                    "sync_job_queued",
                    RepositorySyncJobStatus::Queued,
                    "2026-04-26T10:01:00Z",
                ),
            ],
            ..OrganizationState::default()
        });

        let completed_job = run_repository_sync_claim_tick(
            &store,
            StubRepositorySyncJobExecutionOutcome::Succeeded,
        )
        .await
        .unwrap()
        .expect(
            "queued repository sync job should still be claimed after malformed running recovery",
        );

        assert_eq!(completed_job.id, "sync_job_queued");
        assert_eq!(completed_job.status, RepositorySyncJobStatus::Succeeded);

        let persisted = store.organization_state().await.unwrap();
        let malformed_job = persisted
            .repository_sync_jobs
            .iter()
            .find(|job| job.id == "sync_job_malformed_running")
            .expect("malformed running job should remain in history");
        assert_eq!(malformed_job.status, RepositorySyncJobStatus::Failed);
        assert_eq!(
            malformed_job.finished_at,
            malformed_job
                .error
                .as_ref()
                .and_then(|error| error.split(" at ").last())
                .map(str::to_owned)
        );
        assert!(
            malformed_job
                .error
                .as_deref()
                .unwrap_or_default()
                .starts_with("repository sync job had malformed running lease timestamp and was marked failed before the next claim"),
            "malformed running lease timestamp should produce operator-visible failure detail: {malformed_job:?}"
        );
    }

    #[tokio::test]
    async fn run_repository_sync_claim_tick_requeues_old_stale_lease_failure_once_before_claiming()
    {
        let store = InMemoryOrganizationStore::new(OrganizationState {
            repository_sync_jobs: vec![repository_sync_job(
                "sync_job_stale_running",
                RepositorySyncJobStatus::Failed,
                "2026-04-26T10:01:00Z",
            )],
            ..OrganizationState::default()
        });
        {
            let mut state = store.state.lock().unwrap();
            state.repository_sync_jobs[0].finished_at = Some("2026-04-26T11:05:00Z".to_string());
            state.repository_sync_jobs[0].error = Some(format!(
                "{REPOSITORY_SYNC_STALE_RUNNING_RECOVERY_PREFIX}: started_at 2026-04-26T10:01:00Z exceeded 3600000ms lease"
            ));
        }

        let completed_job = run_repository_sync_claim_tick(
            &store,
            StubRepositorySyncJobExecutionOutcome::Succeeded,
        )
        .await
        .unwrap()
        .expect("old stale-lease failure should be automatically retried and claimed");

        assert_eq!(completed_job.id, "sync_job_stale_running_auto_retry_1");
        assert_eq!(completed_job.status, RepositorySyncJobStatus::Succeeded);
        let persisted = store.organization_state().await.unwrap();
        assert_eq!(persisted.repository_sync_jobs.len(), 2);
        assert_eq!(
            persisted.repository_sync_jobs[0].status,
            RepositorySyncJobStatus::Failed
        );
        assert_eq!(persisted.repository_sync_jobs[1], completed_job);
    }

    #[tokio::test]
    async fn run_repository_sync_claim_tick_requeues_old_malformed_lease_failure_once_before_claiming(
    ) {
        let store = InMemoryOrganizationStore::new(OrganizationState {
            repository_sync_jobs: vec![repository_sync_job(
                "sync_job_malformed_running",
                RepositorySyncJobStatus::Failed,
                "2026-04-26T10:01:00Z",
            )],
            ..OrganizationState::default()
        });
        {
            let mut state = store.state.lock().unwrap();
            state.repository_sync_jobs[0].finished_at = Some("2026-04-26T11:05:00Z".to_string());
            state.repository_sync_jobs[0].error = Some(format!(
                "{REPOSITORY_SYNC_MALFORMED_RUNNING_LEASE_PREFIX}: missing started_at at 2026-04-26T11:05:00Z"
            ));
        }

        let completed_job = run_repository_sync_claim_tick(
            &store,
            StubRepositorySyncJobExecutionOutcome::Succeeded,
        )
        .await
        .unwrap()
        .expect("old malformed-lease failure should be automatically retried and claimed");

        assert_eq!(completed_job.id, "sync_job_malformed_running_auto_retry_1");
        assert_eq!(completed_job.status, RepositorySyncJobStatus::Succeeded);
        let persisted = store.organization_state().await.unwrap();
        assert_eq!(persisted.repository_sync_jobs.len(), 2);
        assert_eq!(
            persisted.repository_sync_jobs[0].status,
            RepositorySyncJobStatus::Failed
        );
        assert_eq!(persisted.repository_sync_jobs[1], completed_job);
    }

    #[tokio::test]
    async fn run_repository_sync_claim_tick_requeues_only_one_stale_lease_failure_for_same_target()
    {
        let store = InMemoryOrganizationStore::new(OrganizationState {
            repository_sync_jobs: vec![
                repository_sync_job(
                    "sync_job_stale_running_a",
                    RepositorySyncJobStatus::Failed,
                    "2026-04-26T10:01:00Z",
                ),
                repository_sync_job(
                    "sync_job_stale_running_b",
                    RepositorySyncJobStatus::Failed,
                    "2026-04-26T10:02:00Z",
                ),
            ],
            ..OrganizationState::default()
        });
        {
            let mut state = store.state.lock().unwrap();
            for job in &mut state.repository_sync_jobs {
                job.repository_id = "repo_shared_target".into();
                job.finished_at = Some("2026-04-26T11:05:00Z".to_string());
                job.error = Some(format!(
                    "{REPOSITORY_SYNC_STALE_RUNNING_RECOVERY_PREFIX}: started_at 2026-04-26T10:01:00Z exceeded 3600000ms lease"
                ));
            }
        }

        let completed_job = run_repository_sync_claim_tick(
            &store,
            StubRepositorySyncJobExecutionOutcome::Succeeded,
        )
        .await
        .unwrap()
        .expect("one old stale-lease failure should be retried and claimed");

        assert_eq!(completed_job.repository_id, "repo_shared_target");
        let persisted = store.organization_state().await.unwrap();
        let replacement_jobs = persisted
            .repository_sync_jobs
            .iter()
            .filter(|job| job.id.contains("_auto_retry_"))
            .count();
        assert_eq!(replacement_jobs, 1, "only one replacement may be queued or claimed for the same organization/repository/connection target");
    }

    #[tokio::test]
    async fn run_repository_sync_claim_tick_does_not_replay_stale_lease_failure_after_terminal_retry_for_same_target(
    ) {
        let store = InMemoryOrganizationStore::new(OrganizationState {
            repository_sync_jobs: vec![
                repository_sync_job(
                    "sync_job_stale_running_a",
                    RepositorySyncJobStatus::Failed,
                    "2026-04-26T10:01:00Z",
                ),
                repository_sync_job(
                    "sync_job_stale_running_a_auto_retry_1",
                    RepositorySyncJobStatus::Succeeded,
                    "2026-04-26T12:10:00Z",
                ),
                repository_sync_job(
                    "sync_job_stale_running_b",
                    RepositorySyncJobStatus::Failed,
                    "2026-04-26T10:02:00Z",
                ),
            ],
            ..OrganizationState::default()
        });
        {
            let mut state = store.state.lock().unwrap();
            for job in &mut state.repository_sync_jobs {
                job.repository_id = "repo_shared_target".into();
                job.finished_at = Some("2026-04-26T11:05:00Z".to_string());
            }
            state.repository_sync_jobs[0].error = Some(format!(
                "{REPOSITORY_SYNC_STALE_RUNNING_RECOVERY_PREFIX}: started_at 2026-04-26T10:01:00Z exceeded 3600000ms lease"
            ));
            state.repository_sync_jobs[1].error = None;
            state.repository_sync_jobs[2].error = Some(format!(
                "{REPOSITORY_SYNC_STALE_RUNNING_RECOVERY_PREFIX}: started_at 2026-04-26T10:02:00Z exceeded 3600000ms lease"
            ));
        }

        let completed_job = run_repository_sync_claim_tick(
            &store,
            StubRepositorySyncJobExecutionOutcome::Succeeded,
        )
        .await
        .unwrap();

        assert_eq!(completed_job, None);
        let persisted = store.organization_state().await.unwrap();
        let replacement_jobs = persisted
            .repository_sync_jobs
            .iter()
            .filter(|job| job.id.contains("_auto_retry_"))
            .count();
        assert_eq!(replacement_jobs, 1, "terminal auto-retry history for the same target must suppress repeated automatic replay");
    }

    #[tokio::test]
    async fn run_repository_sync_claim_tick_records_a_stub_completed_repository_sync_job_in_the_file_store(
    ) {
        let path = unique_test_path("worker-tick-file-store-repository-sync-job");
        let store = FileOrganizationStore::new(&path);
        store
            .store_organization_state(OrganizationState {
                repository_sync_jobs: vec![
                    repository_sync_job(
                        "sync_job_newer",
                        RepositorySyncJobStatus::Queued,
                        "2026-04-26T10:02:00Z",
                    ),
                    repository_sync_job(
                        "sync_job_oldest",
                        RepositorySyncJobStatus::Queued,
                        "2026-04-26T10:01:00Z",
                    ),
                ],
                ..OrganizationState::default()
            })
            .await
            .unwrap();

        let completed_job = run_repository_sync_claim_tick(
            &store,
            StubRepositorySyncJobExecutionOutcome::Succeeded,
        )
        .await
        .unwrap()
        .expect("queued repository sync job to be completed");

        assert_eq!(completed_job.id, "sync_job_oldest");
        assert_eq!(completed_job.status, RepositorySyncJobStatus::Succeeded);
        let started_at = completed_job
            .started_at
            .as_deref()
            .expect("started_at to be set");
        assert!(OffsetDateTime::parse(started_at, &Rfc3339).is_ok());
        let finished_at = completed_job
            .finished_at
            .as_deref()
            .expect("finished_at to be set");
        assert!(OffsetDateTime::parse(finished_at, &Rfc3339).is_ok());
        assert_eq!(completed_job.error, None);

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(
            persisted.repository_sync_jobs[0].status,
            RepositorySyncJobStatus::Queued
        );
        assert_eq!(persisted.repository_sync_jobs[1], completed_job);

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn run_repository_sync_claim_tick_records_a_stub_failed_repository_sync_job_in_the_file_store(
    ) {
        let path = unique_test_path("worker-tick-file-store-repository-sync-job-failed");
        let store = FileOrganizationStore::new(&path);
        store
            .store_organization_state(OrganizationState {
                repository_sync_jobs: vec![
                    repository_sync_job(
                        "sync_job_newer",
                        RepositorySyncJobStatus::Queued,
                        "2026-04-26T10:02:00Z",
                    ),
                    repository_sync_job(
                        "sync_job_oldest",
                        RepositorySyncJobStatus::Queued,
                        "2026-04-26T10:01:00Z",
                    ),
                ],
                ..OrganizationState::default()
            })
            .await
            .unwrap();

        let failed_job =
            run_repository_sync_claim_tick(&store, StubRepositorySyncJobExecutionOutcome::Failed)
                .await
                .unwrap()
                .expect("queued repository sync job to be failed");

        assert_eq!(failed_job.id, "sync_job_oldest");
        assert_eq!(failed_job.status, RepositorySyncJobStatus::Failed);
        let started_at = failed_job
            .started_at
            .as_deref()
            .expect("started_at to be set");
        assert!(OffsetDateTime::parse(started_at, &Rfc3339).is_ok());
        let finished_at = failed_job
            .finished_at
            .as_deref()
            .expect("finished_at to be set");
        assert!(OffsetDateTime::parse(finished_at, &Rfc3339).is_ok());
        assert_eq!(
            failed_job.error.as_deref(),
            Some(REPOSITORY_SYNC_STUB_FAILURE_ERROR)
        );

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(
            persisted.repository_sync_jobs[0].status,
            RepositorySyncJobStatus::Queued
        );
        assert_eq!(persisted.repository_sync_jobs[1], failed_job);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn local_repository_sync_blank_repo_path_fails_closed_before_spawning_git() {
        let state = OrganizationState {
            connections: vec![local_connection("   ")],
            repository_sync_jobs: vec![local_repository_sync_job(
                "sync_job_local_blank_path",
                RepositorySyncJobStatus::Running,
                "2026-04-26T10:01:00Z",
            )],
            ..OrganizationState::default()
        };

        let failed_job = complete_local_repository_sync_job_with_git_command_if_applicable(
            &state,
            &state.repository_sync_jobs[0],
            "2026-04-26T10:02:00Z",
            OsStr::new("/definitely/missing/sourcebot-test-git"),
            Duration::from_millis(50),
        )
        .expect("blank local repo_path should terminally fail the job before git is spawned");

        assert_eq!(failed_job.id, "sync_job_local_blank_path");
        assert_eq!(failed_job.status, RepositorySyncJobStatus::Failed);
        assert_eq!(
            failed_job.error.as_deref(),
            Some("local repository sync preflight failed: local repository repo_path is empty")
        );
    }

    #[test]
    fn generic_git_blank_base_url_fails_closed_before_spawning_git() {
        let error = match run_generic_git_repository_sync_execution(
            OsStr::new("/definitely/missing/sourcebot-test-git"),
            "\t  ",
            Duration::from_millis(50),
        ) {
            Ok(_) => panic!("blank Generic Git base_url should fail before git is spawned"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: Generic Git base_url is empty"
        );
    }

    #[test]
    fn generic_git_base_url_with_surrounding_whitespace_fails_closed_before_spawning_git() {
        let error = match run_generic_git_repository_sync_execution(
            OsStr::new("/definitely/missing/sourcebot-test-git"),
            " https://example.invalid/org/repo.git ",
            Duration::from_millis(50),
        ) {
            Ok(_) => panic!(
                "Generic Git base_url with surrounding whitespace should fail before git is spawned"
            ),
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: Generic Git base_url must not include surrounding whitespace"
        );
    }

    #[test]
    fn generic_git_base_url_with_control_character_fails_closed_before_spawning_git() {
        let error = match run_generic_git_repository_sync_execution(
            OsStr::new("/definitely/missing/sourcebot-test-git"),
            "https://example.invalid/org/repo.git\n--upload-pack=/bin/sh",
            Duration::from_millis(50),
        ) {
            Ok(_) => panic!("Generic Git base_url containing control characters should fail before git is spawned"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: Generic Git base_url must not include control characters"
        );
        assert!(!error.contains("--upload-pack"));
    }

    #[test]
    fn generic_git_http_url_with_empty_authority_fails_closed_before_spawning_git() {
        let error = match run_generic_git_repository_sync_execution(
            OsStr::new("/definitely/missing/sourcebot-test-git"),
            "https:///org/repo.git",
            Duration::from_millis(50),
        ) {
            Ok(_) => panic!(
                "Generic Git URL base_url with empty authority should fail before git is spawned"
            ),
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: Generic Git base_url URL authority must not be empty"
        );
    }

    #[test]
    fn generic_git_file_url_with_empty_authority_is_not_rejected_before_spawning_git() {
        let error = match run_generic_git_repository_sync_execution(
            OsStr::new("/definitely/missing/sourcebot-test-git"),
            "file:///tmp/sourcebot-generic-git-fixture.git",
            Duration::from_millis(50),
        ) {
            Ok(_) => panic!("file:// Generic Git URL should reach git execution"),
            Err(error) => error,
        };

        assert!(
            error.contains("No such file or directory") || error.contains("os error 2"),
            "file:// empty-authority URL should be preserved until git execution, got {error}"
        );
        assert!(!error.contains("URL authority must not be empty"));
    }

    #[test]
    fn generic_git_ext_transport_url_fails_closed_before_spawning_git() {
        let dangerous_url = "ext::sh -c 'printf owned >&2'";
        let error = match run_generic_git_repository_sync_execution(
            OsStr::new("/definitely/missing/sourcebot-test-git"),
            dangerous_url,
            Duration::from_millis(50),
        ) {
            Ok(_) => panic!("Generic Git ext transport base_url should fail before git is spawned"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: Generic Git base_url scheme is not supported"
        );
        assert!(!error.contains("owned"));
        assert!(!error.contains(dangerous_url));
    }

    #[test]
    fn generic_git_unknown_scheme_url_fails_closed_before_spawning_git() {
        let error = match run_generic_git_repository_sync_execution(
            OsStr::new("/definitely/missing/sourcebot-test-git"),
            "ftp://example.invalid/org/repo.git",
            Duration::from_millis(50),
        ) {
            Ok(_) => panic!("Generic Git unsupported URL scheme should fail before git is spawned"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: Generic Git base_url scheme is not supported"
        );
        assert!(!error.contains("example.invalid"));
    }

    #[test]
    fn generic_git_url_with_embedded_credentials_fails_closed_before_spawning_git() {
        let error = match run_generic_git_repository_sync_execution(
            OsStr::new("definitely-not-a-real-git-binary"),
            "https://token@example.com/org/repo.git",
            Duration::from_millis(100),
        ) {
            Ok(_) => {
                panic!("credential-bearing Generic Git base_url must fail before git is spawned")
            }
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: Generic Git base_url must not include embedded credentials"
        );
        assert!(!error.contains("token@example.com"));
    }

    #[test]
    fn generic_git_mixed_case_http_url_with_embedded_credentials_fails_closed_before_spawning_git()
    {
        let secret_url = format!("{}{}", "HtTpS://user:pass", "@example.com/org/repo.git");
        let error = match run_generic_git_repository_sync_execution(
            OsStr::new("definitely-not-a-real-git-binary"),
            &secret_url,
            Duration::from_millis(100),
        ) {
            Ok(_) => panic!(
                "mixed-case credential-bearing Generic Git base_url must fail before git is spawned"
            ),
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: Generic Git base_url must not include embedded credentials"
        );
        assert!(!error.contains(&secret_url));
        assert!(!error.contains("user:pass"));
    }

    #[test]
    fn generic_git_percent_encoded_userinfo_fails_closed_before_spawning_git() {
        let secret_url = "https://user%3Apass%40example.com/org/repo.git";
        let error = match run_generic_git_repository_sync_execution(
            OsStr::new("definitely-not-a-real-git-binary"),
            secret_url,
            Duration::from_millis(100),
        ) {
            Ok(_) => panic!(
                "percent-encoded credential-bearing Generic Git base_url must fail before git is spawned"
            ),
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: Generic Git base_url must not include embedded credentials"
        );
        assert!(!error.contains(secret_url));
        assert!(!error.contains("user%3Apass"));
        assert!(!error.contains("user:pass"));
    }

    #[test]
    fn generic_git_ssh_url_with_embedded_userinfo_fails_closed_before_spawning_git() {
        let secret_url = "ssh://deploy-token@example.org/repo.git";
        let error = match run_generic_git_repository_sync_execution(
            OsStr::new("definitely-not-a-real-git-binary"),
            secret_url,
            Duration::from_millis(100),
        ) {
            Ok(_) => panic!(
                "non-HTTP credential-bearing Generic Git base_url must fail before git is spawned"
            ),
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: Generic Git base_url must not include embedded credentials"
        );
        assert!(!error.contains(secret_url));
        assert!(!error.contains("deploy-token"));
    }

    #[test]
    fn generic_git_sync_forces_non_interactive_git_auth_environment() {
        let fake_git_dir = std::env::temp_dir().join(format!(
            "sourcebot-worker-generic-git-non-interactive-env-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&fake_git_dir).unwrap();
        let fake_git_path = fake_git_dir.join("fake-git");
        fs::write(
            &fake_git_path,
            "#!/bin/sh\nif [ \"${GIT_TERMINAL_PROMPT:-}\" != \"0\" ]; then\n  printf 'GIT_TERMINAL_PROMPT was not forced off: %s\\n' \"${GIT_TERMINAL_PROMPT:-<unset>}\" >&2\n  exit 43\nfi\nif [ \"${GCM_INTERACTIVE:-}\" != \"never\" ]; then\n  printf 'GCM_INTERACTIVE was not forced off: %s\\n' \"${GCM_INTERACTIVE:-<unset>}\" >&2\n  exit 44\nfi\nif [ \"$1 $2 $3\" = \"ls-remote --symref --\" ]; then\n  printf 'ref: refs/heads/main\\tHEAD\\n'\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\tHEAD\\n'\n  exit 0\nfi\nif [ \"$1 $2 $3\" = \"ls-remote --heads --\" ]; then\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\trefs/heads/main\\n'\n  exit 0\nfi\nprintf 'unexpected git invocation: %s\\n' \"$*\" >&2\nexit 2\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let execution = run_generic_git_repository_sync_execution(
            fake_git_path.as_os_str(),
            "https://example.invalid/acme/repo.git",
            Duration::from_secs(5),
        )
        .expect("Generic Git sync should force non-interactive auth env before spawning git");

        assert_eq!(execution.branch, "main");
        assert_eq!(
            execution.revision,
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        );

        fs::remove_dir_all(fake_git_dir).unwrap();
    }

    #[test]
    fn generic_git_sync_forces_ssh_prompt_suppression_environment() {
        let fake_git_dir = std::env::temp_dir().join(format!(
            "sourcebot-worker-generic-git-ssh-prompt-env-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&fake_git_dir).unwrap();
        let fake_git_path = fake_git_dir.join("fake-git");
        fs::write(
            &fake_git_path,
            "#!/bin/sh\nif [ \"${GIT_ASKPASS:-}\" != \"/bin/false\" ]; then\n  printf 'GIT_ASKPASS was not forced to /bin/false: %s\\n' \"${GIT_ASKPASS:-<unset>}\" >&2\n  exit 45\nfi\nif [ \"${SSH_ASKPASS:-}\" != \"/bin/false\" ]; then\n  printf 'SSH_ASKPASS was not forced to /bin/false: %s\\n' \"${SSH_ASKPASS:-<unset>}\" >&2\n  exit 46\nfi\ncase \"${GIT_SSH_COMMAND:-}\" in\n  *\"BatchMode=yes\"*\"NumberOfPasswordPrompts=0\"*) ;;\n  *) printf 'GIT_SSH_COMMAND did not disable ssh prompts: %s\\n' \"${GIT_SSH_COMMAND:-<unset>}\" >&2; exit 47 ;;\nesac\nif [ \"$1 $2 $3\" = \"ls-remote --symref --\" ]; then\n  printf 'ref: refs/heads/main\\tHEAD\\n'\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\tHEAD\\n'\n  exit 0\nfi\nif [ \"$1 $2 $3\" = \"ls-remote --heads --\" ]; then\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\trefs/heads/main\\n'\n  exit 0\nfi\nprintf 'unexpected git invocation: %s\\n' \"$*\" >&2\nexit 2\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let execution = run_generic_git_repository_sync_execution(
            fake_git_path.as_os_str(),
            "ssh://example.invalid/acme/repo.git",
            Duration::from_secs(5),
        )
        .expect("Generic Git sync should force non-interactive SSH auth env before spawning git");

        assert_eq!(execution.branch, "main");
        assert_eq!(
            execution.revision,
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        );

        fs::remove_dir_all(fake_git_dir).unwrap();
    }

    #[test]
    fn generic_git_sync_prefers_remote_head_symref_default_branch() {
        let fake_git_dir = std::env::temp_dir().join(format!(
            "sourcebot-worker-generic-git-default-branch-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&fake_git_dir).unwrap();
        let fake_git_path = fake_git_dir.join("fake-git");
        fs::write(
            &fake_git_path,
            "#!/bin/sh\nif [ \"$1 $2 $3\" = \"ls-remote --symref --\" ]; then\n  printf 'ref: refs/heads/main\\tHEAD\\n'\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\tHEAD\\n'\n  exit 0\nfi\nif [ \"$1 $2 $3\" = \"ls-remote --heads --\" ]; then\n  printf 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\\trefs/heads/feature\\n'\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\trefs/heads/main\\n'\n  exit 0\nfi\nprintf 'unexpected git invocation: %s\\n' \"$*\" >&2\nexit 2\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let execution = run_generic_git_repository_sync_execution(
            fake_git_path.as_os_str(),
            "https://example.invalid/acme/repo.git",
            Duration::from_secs(5),
        )
        .expect("Generic Git sync should resolve the remote HEAD default branch before falling back to the first advertised head");

        assert_eq!(execution.branch, "main");
        assert_eq!(
            execution.revision,
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        );

        fs::remove_dir_all(fake_git_dir).unwrap();
    }

    #[test]
    fn generic_git_sync_requires_advertised_heads_even_when_remote_head_symref_exists() {
        let fake_git_dir = std::env::temp_dir().join(format!(
            "sourcebot-worker-generic-git-heads-required-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&fake_git_dir).unwrap();
        let fake_git_path = fake_git_dir.join("fake-git");
        let heads_marker_path = fake_git_dir.join("heads-invoked");
        fs::write(
            &fake_git_path,
            format!(
                "#!/bin/sh\nif [ \"$1 $2 $3\" = \"ls-remote --symref --\" ]; then\n  printf 'ref: refs/heads/main\\tHEAD\\n'\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\tHEAD\\n'\n  exit 0\nfi\nif [ \"$1 $2 $3\" = \"ls-remote --heads --\" ]; then\n  : > '{}'\n  exit 0\nfi\nprintf 'unexpected git invocation: %s\\n' \"$*\" >&2\nexit 2\n",
                heads_marker_path.display()
            ),
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let error = match run_generic_git_repository_sync_execution(
            fake_git_path.as_os_str(),
            "https://example.invalid/acme/repo.git",
            Duration::from_secs(5),
        ) {
            Ok(execution) => panic!(
                "Generic Git sync must require an advertised --heads branch even when HEAD symref exists, got revision={} branch={}",
                execution.revision, execution.branch
            ),
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: git ls-remote --heads advertised no valid branch refs"
        );
        assert!(
            heads_marker_path.exists(),
            "Generic Git sync must invoke the --heads probe before claiming success from HEAD symref metadata"
        );

        fs::remove_dir_all(fake_git_dir).unwrap();
    }

    #[test]
    fn generic_git_sync_requires_head_symref_branch_to_be_advertised_by_heads() {
        let fake_git_dir = std::env::temp_dir().join(format!(
            "sourcebot-worker-generic-git-head-symref-unadvertised-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&fake_git_dir).unwrap();
        let fake_git_path = fake_git_dir.join("fake-git");
        fs::write(
            &fake_git_path,
            "#!/bin/sh\nif [ \"$1 $2 $3\" = \"ls-remote --symref --\" ]; then\n  printf 'ref: refs/heads/main\\tHEAD\\n'\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\tHEAD\\n'\n  exit 0\nfi\nif [ \"$1 $2 $3\" = \"ls-remote --heads --\" ]; then\n  printf 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\\trefs/heads/feature\\n'\n  exit 0\nfi\nprintf 'unexpected git invocation: %s\\n' \"$*\" >&2\nexit 2\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let error = match run_generic_git_repository_sync_execution(
            fake_git_path.as_os_str(),
            "https://example.invalid/acme/repo.git",
            Duration::from_secs(5),
        ) {
            Ok(execution) => panic!(
                "Generic Git HEAD symref metadata must match an advertised --heads branch before success, got revision={} branch={}",
                execution.revision, execution.branch
            ),
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: git ls-remote --symref HEAD branch was not advertised by --heads"
        );

        fs::remove_dir_all(fake_git_dir).unwrap();
    }

    #[test]
    fn generic_git_ls_remote_malformed_head_revision_fails_closed_before_heads_fallback() {
        let fake_git_dir = std::env::temp_dir().join(format!(
            "sourcebot-worker-generic-git-malformed-head-revision-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&fake_git_dir).unwrap();
        let fake_git_path = fake_git_dir.join("fake-git");
        fs::write(
            &fake_git_path,
            "#!/bin/sh\nif [ \"$1 $2 $3\" = \"ls-remote --symref --\" ]; then\n  printf 'not-a-valid-object-id\\tHEAD\\n'\n  exit 0\nfi\nif [ \"$1 $2 $3\" = \"ls-remote --heads --\" ]; then\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\trefs/heads/main\\n'\n  exit 0\nfi\nprintf 'unexpected git invocation: %s\\n' \"$*\" >&2\nexit 2\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let error = match run_generic_git_repository_sync_execution(
            fake_git_path.as_os_str(),
            "https://example.invalid/acme/repo.git",
            Duration::from_secs(5),
        ) {
            Ok(execution) => panic!(
                "malformed HEAD revision in Generic Git --symref output must fail closed before --heads fallback, got revision={} branch={}",
                execution.revision, execution.branch
            ),
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: git ls-remote --symref HEAD advertised malformed HEAD revision"
        );

        fs::remove_dir_all(fake_git_dir).unwrap();
    }

    #[test]
    fn generic_git_ls_remote_duplicate_head_revisions_fail_closed_before_heads_fallback() {
        let fake_git_dir = std::env::temp_dir().join(format!(
            "sourcebot-worker-generic-git-duplicate-head-revisions-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&fake_git_dir).unwrap();
        let fake_git_path = fake_git_dir.join("fake-git");
        fs::write(
            &fake_git_path,
            "#!/bin/sh\nif [ \"$1 $2 $3\" = \"ls-remote --symref --\" ]; then\n  printf 'ref: refs/heads/main\\tHEAD\\n'\n  printf 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\\tHEAD\\n'\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\tHEAD\\n'\n  exit 0\nfi\nif [ \"$1 $2 $3\" = \"ls-remote --heads --\" ]; then\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\trefs/heads/main\\n'\n  exit 0\nfi\nprintf 'unexpected git invocation: %s\\n' \"$*\" >&2\nexit 2\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let error = match run_generic_git_repository_sync_execution(
            fake_git_path.as_os_str(),
            "https://example.invalid/acme/repo.git",
            Duration::from_secs(5),
        ) {
            Ok(execution) => panic!(
                "duplicate HEAD revisions in Generic Git --symref output must fail closed before --heads fallback, got revision={} branch={}",
                execution.revision, execution.branch
            ),
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: git ls-remote --symref HEAD advertised ambiguous HEAD revision"
        );

        fs::remove_dir_all(fake_git_dir).unwrap();
    }

    #[test]
    fn generic_git_ls_remote_heads_malformed_ref_line_fails_closed() {
        let fake_git_dir = std::env::temp_dir().join(format!(
            "sourcebot-worker-generic-git-malformed-heads-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&fake_git_dir).unwrap();
        let fake_git_path = fake_git_dir.join("fake-git");
        fs::write(
            &fake_git_path,
            "#!/bin/sh\nif [ \"$1 $2 $3\" = \"ls-remote --symref --\" ]; then\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\tHEAD\\n'\n  exit 0\nfi\nif [ \"$1 $2 $3\" = \"ls-remote --heads --\" ]; then\n  printf 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\\trefs/heads/feature\\n'\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb refs/heads/main\\n'\n  exit 0\nfi\nprintf 'unexpected git invocation: %s\\n' \"$*\" >&2\nexit 2\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let error = match run_generic_git_repository_sync_execution(
            fake_git_path.as_os_str(),
            "https://example.invalid/acme/repo.git",
            Duration::from_secs(5),
        ) {
            Ok(_) => panic!("malformed Generic Git ls-remote --heads output must fail closed"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: git ls-remote --heads advertised malformed ref line"
        );

        fs::remove_dir_all(fake_git_dir).unwrap();
    }

    #[test]
    fn generic_git_ls_remote_heads_validates_later_lines_after_preferred_match() {
        let fake_git_dir = std::env::temp_dir().join(format!(
            "sourcebot-worker-generic-git-late-malformed-heads-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&fake_git_dir).unwrap();
        let fake_git_path = fake_git_dir.join("fake-git");
        fs::write(
            &fake_git_path,
            "#!/bin/sh\nif [ \"$1 $2 $3\" = \"ls-remote --symref --\" ]; then\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\tHEAD\\n'\n  exit 0\nfi\nif [ \"$1 $2 $3\" = \"ls-remote --heads --\" ]; then\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\trefs/heads/main\\n'\n  printf 'malformed refs/heads/bad\\n'\n  exit 0\nfi\nprintf 'unexpected git invocation: %s\\n' \"$*\" >&2\nexit 2\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let error = match run_generic_git_repository_sync_execution(
            fake_git_path.as_os_str(),
            "https://example.invalid/acme/repo.git",
            Duration::from_secs(5),
        ) {
            Ok(_) => panic!("later malformed Generic Git --heads line must fail closed"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: git ls-remote --heads advertised malformed ref line"
        );

        fs::remove_dir_all(fake_git_dir).unwrap();
    }

    #[test]
    fn generic_git_ls_remote_heads_combined_stdout_stderr_over_limit_fails_closed() {
        let fake_git_dir = std::env::temp_dir().join(format!(
            "sourcebot-worker-generic-git-combined-output-limit-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&fake_git_dir).unwrap();
        let fake_git_path = fake_git_dir.join("fake-git");
        fs::write(
            &fake_git_path,
            "#!/bin/sh\nif [ \"$1 $2 $3\" = \"ls-remote --symref --\" ]; then\n  exit 0\nfi\nif [ \"$1 $2 $3\" = \"ls-remote --heads --\" ]; then\n  i=0\n  while [ $i -lt 10000 ]; do\n    printf 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\\trefs/heads/main\\n'\n    i=$((i + 1))\n  done\n  dd if=/dev/zero bs=1000 count=600 2>/dev/null | tr '\\000' 'e' >&2\n  exit 0\nfi\nprintf 'unexpected git invocation: %s\\n' \"$*\" >&2\nexit 2\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let error = match run_generic_git_repository_sync_execution(
            fake_git_path.as_os_str(),
            "https://example.invalid/acme/repo.git",
            Duration::from_secs(5),
        ) {
            Ok(execution) => panic!(
                "combined Generic Git ls-remote --heads stdout/stderr over the cap must fail closed, got revision={} branch={}",
                execution.revision, execution.branch
            ),
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: git ls-remote --heads output exceeded 1048576 bytes"
        );

        fs::remove_dir_all(fake_git_dir).unwrap();
    }

    #[test]
    fn generic_git_ls_remote_malformed_head_symref_fails_closed_before_heads_fallback() {
        let fake_git_dir = std::env::temp_dir().join(format!(
            "sourcebot-worker-generic-git-malformed-head-symref-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&fake_git_dir).unwrap();
        let fake_git_path = fake_git_dir.join("fake-git");
        fs::write(
            &fake_git_path,
            "#!/bin/sh\nif [ \"$1 $2 $3\" = \"ls-remote --symref --\" ]; then\n  printf 'ref: refs/heads/.hidden\\tHEAD\\n'\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\tHEAD\\n'\n  exit 0\nfi\nif [ \"$1 $2 $3\" = \"ls-remote --heads --\" ]; then\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\trefs/heads/main\\n'\n  exit 0\nfi\nprintf 'unexpected git invocation: %s\\n' \"$*\" >&2\nexit 2\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let error = match run_generic_git_repository_sync_execution(
            fake_git_path.as_os_str(),
            "https://example.invalid/acme/repo.git",
            Duration::from_secs(5),
        ) {
            Ok(execution) => panic!(
                "malformed HEAD symref must not fall back to --heads, got revision={} branch={}",
                execution.revision, execution.branch
            ),
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: git ls-remote --symref HEAD advertised malformed HEAD symref"
        );

        fs::remove_dir_all(fake_git_dir).unwrap();
    }

    #[test]
    fn generic_git_ls_remote_head_symref_extra_field_fails_closed_before_heads_fallback() {
        let fake_git_dir = std::env::temp_dir().join(format!(
            "sourcebot-worker-generic-git-head-symref-extra-field-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&fake_git_dir).unwrap();
        let fake_git_path = fake_git_dir.join("fake-git");
        fs::write(
            &fake_git_path,
            "#!/bin/sh\nif [ \"$1 $2 $3\" = \"ls-remote --symref --\" ]; then\n  printf 'ref: refs/heads/main\\tHEAD\\textra\\n'\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\tHEAD\\n'\n  exit 0\nfi\nif [ \"$1 $2 $3\" = \"ls-remote --heads --\" ]; then\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\trefs/heads/main\\n'\n  exit 0\nfi\nprintf 'unexpected git invocation: %s\\n' \"$*\" >&2\nexit 2\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let error = match run_generic_git_repository_sync_execution(
            fake_git_path.as_os_str(),
            "https://example.invalid/acme/repo.git",
            Duration::from_secs(5),
        ) {
            Ok(execution) => panic!(
                "malformed HEAD symref with an extra field must not fall back to --heads, got revision={} branch={}",
                execution.revision, execution.branch
            ),
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: git ls-remote --symref HEAD advertised malformed HEAD symref"
        );

        fs::remove_dir_all(fake_git_dir).unwrap();
    }

    #[test]
    fn generic_git_ls_remote_head_symref_non_head_target_fails_closed_before_heads_fallback() {
        let fake_git_dir = std::env::temp_dir().join(format!(
            "sourcebot-worker-generic-git-head-symref-non-head-target-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&fake_git_dir).unwrap();
        let fake_git_path = fake_git_dir.join("fake-git");
        fs::write(
            &fake_git_path,
            "#!/bin/sh\nif [ \"$1 $2 $3\" = \"ls-remote --symref --\" ]; then\n  printf 'ref: refs/heads/main\\trefs/heads/other\\n'\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\tHEAD\\n'\n  exit 0\nfi\nif [ \"$1 $2 $3\" = \"ls-remote --heads --\" ]; then\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\trefs/heads/main\\n'\n  exit 0\nfi\nprintf 'unexpected git invocation: %s\\n' \"$*\" >&2\nexit 2\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let error = match run_generic_git_repository_sync_execution(
            fake_git_path.as_os_str(),
            "https://example.invalid/acme/repo.git",
            Duration::from_secs(5),
        ) {
            Ok(execution) => panic!(
                "malformed HEAD symref with a non-HEAD target must not fall back to --heads, got revision={} branch={}",
                execution.revision, execution.branch
            ),
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: git ls-remote --symref HEAD advertised malformed HEAD symref"
        );

        fs::remove_dir_all(fake_git_dir).unwrap();
    }

    #[test]
    fn generic_git_ls_remote_head_symref_unexpected_ref_line_fails_closed_before_heads_fallback() {
        let fake_git_dir = std::env::temp_dir().join(format!(
            "sourcebot-worker-generic-git-head-symref-unexpected-ref-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&fake_git_dir).unwrap();
        let fake_git_path = fake_git_dir.join("fake-git");
        fs::write(
            &fake_git_path,
            "#!/bin/sh\nif [ \"$1 $2 $3\" = \"ls-remote --symref --\" ]; then\n  printf 'ref: refs/heads/main\\tHEAD\\n'\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\tHEAD\\n'\n  printf 'cccccccccccccccccccccccccccccccccccccccc\\trefs/heads/side\\n'\n  exit 0\nfi\nif [ \"$1 $2 $3\" = \"ls-remote --heads --\" ]; then\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\trefs/heads/main\\n'\n  exit 0\nfi\nprintf 'unexpected git invocation: %s\\n' \"$*\" >&2\nexit 2\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let error = match run_generic_git_repository_sync_execution(
            fake_git_path.as_os_str(),
            "https://example.invalid/acme/repo.git",
            Duration::from_secs(5),
        ) {
            Ok(execution) => panic!(
                "unexpected non-HEAD ref lines in Generic Git --symref output must not fall back to --heads, got revision={} branch={}",
                execution.revision, execution.branch
            ),
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: git ls-remote --symref HEAD advertised malformed HEAD metadata"
        );

        fs::remove_dir_all(fake_git_dir).unwrap();
    }

    #[test]
    fn generic_git_ls_remote_head_symref_without_revision_fails_closed_before_heads_fallback() {
        let fake_git_dir = std::env::temp_dir().join(format!(
            "sourcebot-worker-generic-git-head-symref-without-revision-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&fake_git_dir).unwrap();
        let fake_git_path = fake_git_dir.join("fake-git");
        fs::write(
            &fake_git_path,
            "#!/bin/sh\nif [ \"$1 $2 $3\" = \"ls-remote --symref --\" ]; then\n  printf 'ref: refs/heads/main\\tHEAD\\n'\n  exit 0\nfi\nif [ \"$1 $2 $3\" = \"ls-remote --heads --\" ]; then\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\trefs/heads/main\\n'\n  exit 0\nfi\nprintf 'unexpected git invocation: %s\\n' \"$*\" >&2\nexit 2\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let error = match run_generic_git_repository_sync_execution(
            fake_git_path.as_os_str(),
            "https://example.invalid/acme/repo.git",
            Duration::from_secs(5),
        ) {
            Ok(execution) => panic!(
                "incomplete Generic Git HEAD symref metadata must not fall back to --heads, got revision={} branch={}",
                execution.revision, execution.branch
            ),
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: git ls-remote --symref HEAD advertised incomplete HEAD symref metadata"
        );

        fs::remove_dir_all(fake_git_dir).unwrap();
    }

    #[test]
    fn generic_git_sync_resolves_head_revision_when_remote_omits_symref() {
        let fake_git_dir = std::env::temp_dir().join(format!(
            "sourcebot-worker-generic-git-head-revision-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&fake_git_dir).unwrap();
        let fake_git_path = fake_git_dir.join("fake-git");
        fs::write(
            &fake_git_path,
            "#!/bin/sh\nif [ \"$1 $2 $3\" = \"ls-remote --symref --\" ]; then\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\tHEAD\\n'\n  exit 0\nfi\nif [ \"$1 $2 $3\" = \"ls-remote --heads --\" ]; then\n  printf 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\\trefs/heads/feature\\n'\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\trefs/heads/main\\n'\n  exit 0\nfi\nprintf 'unexpected git invocation: %s\\n' \"$*\" >&2\nexit 2\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let execution = run_generic_git_repository_sync_execution(
            fake_git_path.as_os_str(),
            "https://example.invalid/acme/repo.git",
            Duration::from_secs(5),
        )
        .expect("Generic Git sync should map a direct HEAD revision back to the matching branch before falling back to the first advertised head");

        assert_eq!(execution.branch, "main");
        assert_eq!(
            execution.revision,
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        );

        fs::remove_dir_all(fake_git_dir).unwrap();
    }

    #[test]
    fn generic_git_sync_fails_closed_when_heads_advertise_duplicate_branch_ref() {
        let fake_git_dir = std::env::temp_dir().join(format!(
            "sourcebot-worker-generic-git-duplicate-branch-ref-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&fake_git_dir).unwrap();
        let fake_git_path = fake_git_dir.join("fake-git");
        fs::write(
            &fake_git_path,
            "#!/bin/sh\nif [ \"$1 $2 $3\" = \"ls-remote --symref --\" ]; then\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\tHEAD\\n'\n  exit 0\nfi\nif [ \"$1 $2 $3\" = \"ls-remote --heads --\" ]; then\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\trefs/heads/main\\n'\n  printf 'cccccccccccccccccccccccccccccccccccccccc\\trefs/heads/main\\n'\n  exit 0\nfi\nprintf 'unexpected git invocation: %s\\n' \"$*\" >&2\nexit 2\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let error = match run_generic_git_repository_sync_execution(
            fake_git_path.as_os_str(),
            "https://example.invalid/acme/repo.git",
            Duration::from_secs(5),
        ) {
            Ok(execution) => panic!(
                "duplicate Generic Git branch advertisements must fail closed instead of picking branch={}, revision={}",
                execution.branch, execution.revision
            ),
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: git ls-remote --heads advertised duplicate branch ref"
        );

        fs::remove_dir_all(fake_git_dir).unwrap();
    }

    #[test]
    fn generic_git_ls_remote_failure_detail_is_bounded() {
        let fake_git_dir = std::env::temp_dir().join(format!(
            "sourcebot-worker-generic-git-large-error-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&fake_git_dir).unwrap();
        let fake_git_path = fake_git_dir.join("fake-git");
        fs::write(
            &fake_git_path,
            "#!/bin/sh\npython3 - <<'PY' >&2\nimport sys\nsys.stderr.write('remote failure detail: ' + ('x' * 8192))\nPY\nexit 42\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let error = match run_generic_git_repository_sync_execution(
            fake_git_path.as_os_str(),
            "https://example.invalid/acme/repo.git",
            Duration::from_secs(5),
        ) {
            Ok(_) => panic!("failing Generic Git ls-remote should fail closed"),
            Err(error) => error,
        };

        assert!(
            error.starts_with(
                "generic Git repository sync execution failed: remote failure detail: "
            ),
            "bounded error should preserve useful prefix: {error:?}"
        );
        assert!(
            error.len() <= 4200,
            "Generic Git failure detail must be bounded before persistence/logging: {} bytes",
            error.len()
        );
        assert!(
            error.ends_with("...[truncated]"),
            "bounded Generic Git error should make truncation explicit: {error:?}"
        );

        fs::remove_dir_all(fake_git_dir).unwrap();
    }

    #[test]
    fn generic_git_ls_remote_head_stdout_over_cap_fails_closed() {
        let fake_git_dir = std::env::temp_dir().join(format!(
            "sourcebot-worker-large-generic-git-head-output-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&fake_git_dir).unwrap();
        let fake_git_path = fake_git_dir.join("fake-git");
        fs::write(
            &fake_git_path,
            format!(
                "#!/bin/sh\npython3 - <<'PY'\nimport sys\nsys.stdout.write('a' * {})\nPY\n",
                crate::GENERIC_GIT_LS_REMOTE_OUTPUT_LIMIT_BYTES + 1
            ),
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let error = match run_generic_git_repository_sync_execution(
            fake_git_path.as_os_str(),
            "https://example.invalid/acme/repo.git",
            Duration::from_secs(5),
        ) {
            Ok(_) => panic!("oversized Generic Git ls-remote stdout should fail closed"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            format!(
                "{GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git ls-remote --symref HEAD output exceeded {} bytes",
                crate::GENERIC_GIT_LS_REMOTE_OUTPUT_LIMIT_BYTES
            )
        );

        fs::remove_dir_all(fake_git_dir).unwrap();
    }

    #[test]
    fn generic_git_ls_remote_heads_stdout_over_cap_fails_closed() {
        let fake_git_dir = std::env::temp_dir().join(format!(
            "sourcebot-worker-large-generic-git-heads-output-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&fake_git_dir).unwrap();
        let fake_git_path = fake_git_dir.join("fake-git");
        fs::write(
            &fake_git_path,
            format!(
                "#!/bin/sh\nif [ \"$1 $2 $3\" = \"ls-remote --symref --\" ]; then\n  printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\tHEAD\\n'\n  exit 0\nfi\nif [ \"$1 $2 $3\" = \"ls-remote --heads --\" ]; then\n  python3 - <<'PY'\nimport sys\nsys.stdout.write('b' * {})\nPY\n  exit 0\nfi\nprintf 'unexpected git invocation: %s\\n' \"$*\" >&2\nexit 2\n",
                crate::GENERIC_GIT_LS_REMOTE_OUTPUT_LIMIT_BYTES + 1
            ),
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let error = match run_generic_git_repository_sync_execution(
            fake_git_path.as_os_str(),
            "https://example.invalid/acme/repo.git",
            Duration::from_secs(5),
        ) {
            Ok(_) => panic!("oversized Generic Git ls-remote --heads stdout should fail closed"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            format!(
                "{GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git ls-remote --heads output exceeded {} bytes",
                crate::GENERIC_GIT_LS_REMOTE_OUTPUT_LIMIT_BYTES
            )
        );

        fs::remove_dir_all(fake_git_dir).unwrap();
    }

    #[test]
    fn bounded_child_output_kills_process_as_soon_as_stdout_exceeds_limit() {
        let child = Command::new("python3")
            .arg("-c")
            .arg("import sys, time; sys.stdout.write('x' * 65); sys.stdout.flush(); time.sleep(5)")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("oversized output fixture should spawn");

        let started_at = Instant::now();
        let output =
            wait_for_child_output_with_timeout_and_output_limit(child, Duration::from_secs(5), 64)
                .expect("bounded output reader should complete")
                .expect("output-limit termination should return captured output, not a timeout");

        assert!(
            started_at.elapsed() < Duration::from_secs(2),
            "output-limit termination should not wait for the child sleep or full timeout"
        );
        assert_eq!(output.stdout.len(), 65);
        assert!(
            !output.status.success(),
            "child should be killed once retained output exceeds the configured limit"
        );
    }

    #[test]
    fn bounded_child_output_kills_process_as_soon_as_stderr_exceeds_limit() {
        let child = Command::new("python3")
            .arg("-c")
            .arg("import sys, time; sys.stderr.write('e' * 65); sys.stderr.flush(); time.sleep(5)")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("oversized stderr fixture should spawn");

        let started_at = Instant::now();
        let output =
            wait_for_child_output_with_timeout_and_output_limit(child, Duration::from_secs(5), 64)
                .expect("bounded output reader should complete")
                .expect("output-limit termination should return captured output, not a timeout");

        assert!(
            started_at.elapsed() < Duration::from_secs(2),
            "stderr output-limit termination should not wait for the child sleep or full timeout"
        );
        assert_eq!(output.stderr.len(), 65);
        assert!(
            !output.status.success(),
            "child should be killed once retained stderr exceeds the configured limit"
        );
    }

    #[tokio::test]
    async fn run_repository_sync_claim_tick_fails_closed_for_missing_local_repository_path() {
        let missing_repo_path = unique_test_path("missing-local-repository-root");
        let store = InMemoryOrganizationStore::new(OrganizationState {
            connections: vec![local_connection(missing_repo_path.display().to_string())],
            repository_sync_jobs: vec![local_repository_sync_job(
                "sync_job_local_missing",
                RepositorySyncJobStatus::Queued,
                "2026-04-26T10:01:00Z",
            )],
            ..OrganizationState::default()
        });

        let failed_job = run_repository_sync_claim_tick(
            &store,
            StubRepositorySyncJobExecutionOutcome::Succeeded,
        )
        .await
        .unwrap()
        .expect("queued local repository sync job should be terminally recorded");

        assert_eq!(failed_job.id, "sync_job_local_missing");
        assert_eq!(failed_job.status, RepositorySyncJobStatus::Failed);
        assert!(failed_job.started_at.is_some());
        assert!(failed_job.finished_at.is_some());
        assert!(
            failed_job
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("local repository sync preflight failed"),
            "missing local repo path should produce operator-visible failure detail: {failed_job:?}"
        );

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(persisted.repository_sync_jobs[0], failed_job);
    }

    #[tokio::test]
    async fn run_repository_sync_claim_tick_fails_closed_for_bare_local_git_repository_path() {
        let repo_path = std::env::temp_dir().join(format!(
            "sourcebot-worker-bare-local-git-repo-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        Command::new("git")
            .args(["init", "--bare"])
            .arg(&repo_path)
            .output()
            .expect("git init --bare should run");
        let store = InMemoryOrganizationStore::new(OrganizationState {
            connections: vec![local_connection(repo_path.display().to_string())],
            repository_sync_jobs: vec![local_repository_sync_job(
                "sync_job_local_bare",
                RepositorySyncJobStatus::Queued,
                "2026-04-26T10:01:00Z",
            )],
            ..OrganizationState::default()
        });

        let failed_job = run_repository_sync_claim_tick(
            &store,
            StubRepositorySyncJobExecutionOutcome::Succeeded,
        )
        .await
        .unwrap()
        .expect("queued bare local repository sync job should be terminally recorded");

        assert_eq!(failed_job.id, "sync_job_local_bare");
        assert_eq!(failed_job.status, RepositorySyncJobStatus::Failed);
        assert!(
            failed_job
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("local repository sync preflight failed"),
            "bare local repo path should fail the working-tree preflight: {failed_job:?}"
        );

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(persisted.repository_sync_jobs[0], failed_job);

        fs::remove_dir_all(repo_path).unwrap();
    }

    #[test]
    fn local_repository_sync_malformed_head_revision_fails_closed() {
        let repo_path = std::env::temp_dir().join(format!(
            "sourcebot-worker-malformed-head-local-git-repo-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&repo_path).unwrap();
        let fake_git_path = repo_path.join("fake-git");
        fs::write(
            &fake_git_path,
            "#!/bin/sh\nif [ \"$1\" = \"-C\" ] && [ \"$3 $4\" = \"rev-parse --is-inside-work-tree\" ]; then\n  printf 'true\\n'\n  exit 0\nfi\nif [ \"$1\" = \"-C\" ] && [ \"$3 $4\" = \"rev-parse HEAD\" ]; then\n  printf 'not-a-git-object-id\\n'\n  exit 0\nfi\nprintf 'unexpected git invocation after malformed HEAD: %s\\n' \"$*\" >&2\nexit 2\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let state = OrganizationState {
            connections: vec![local_connection(repo_path.display().to_string())],
            repository_sync_jobs: vec![local_repository_sync_job(
                "sync_job_local_malformed_head",
                RepositorySyncJobStatus::Running,
                "2026-04-26T10:01:00Z",
            )],
            ..OrganizationState::default()
        };
        let failed_job =
            complete_local_repository_sync_job_with_git_command_and_output_limit_if_applicable(
                &state,
                &state.repository_sync_jobs[0],
                "2026-04-26T10:02:00Z",
                fake_git_path.as_os_str(),
                Duration::from_secs(2),
                4096,
            )
            .expect("malformed local HEAD revision should terminally fail the job");

        assert_eq!(failed_job.id, "sync_job_local_malformed_head");
        assert_eq!(failed_job.status, RepositorySyncJobStatus::Failed);
        assert_eq!(
            failed_job.error.as_deref(),
            Some("local repository sync execution failed: git rev-parse HEAD returned malformed revision")
        );
        assert_eq!(failed_job.synced_revision, None);
        assert_eq!(failed_job.synced_branch, None);
        assert_eq!(failed_job.synced_content_file_count, None);

        fs::remove_dir_all(repo_path).unwrap();
    }

    #[test]
    fn local_repository_sync_preflight_stdout_over_cap_fails_closed() {
        let repo_path = std::env::temp_dir().join(format!(
            "sourcebot-worker-oversized-preflight-local-git-repo-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&repo_path).unwrap();
        let fake_git_path = repo_path.join("fake-git");
        fs::write(
            &fake_git_path,
            "#!/bin/sh\npython3 - <<'PY'\nimport sys\nsys.stdout.write('t' * 65)\nPY\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let state = OrganizationState {
            connections: vec![local_connection(repo_path.display().to_string())],
            repository_sync_jobs: vec![local_repository_sync_job(
                "sync_job_local_oversized_preflight",
                RepositorySyncJobStatus::Running,
                "2026-04-26T10:01:00Z",
            )],
            ..OrganizationState::default()
        };
        let failed_job =
            complete_local_repository_sync_job_with_git_command_and_output_limit_if_applicable(
                &state,
                &state.repository_sync_jobs[0],
                "2026-04-26T10:02:00Z",
                fake_git_path.as_os_str(),
                Duration::from_secs(2),
                64,
            )
            .expect("oversized local repository preflight output should terminally fail the job");

        assert_eq!(failed_job.id, "sync_job_local_oversized_preflight");
        assert_eq!(failed_job.status, RepositorySyncJobStatus::Failed);
        assert_eq!(
            failed_job.error.as_deref(),
            Some("local repository sync preflight failed: git preflight output exceeded 64 bytes")
        );

        fs::remove_dir_all(repo_path).unwrap();
    }

    #[test]
    fn local_repository_sync_preflight_combined_output_over_cap_fails_closed() {
        let repo_path = std::env::temp_dir().join(format!(
            "sourcebot-worker-combined-preflight-local-git-repo-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&repo_path).unwrap();
        let fake_git_path = repo_path.join("fake-git");
        fs::write(
            &fake_git_path,
            "#!/bin/sh\nif [ \"$1\" = \"-C\" ] && [ \"$3 $4\" = \"rev-parse --is-inside-work-tree\" ]; then\n  printf 'true\\n'\n  printf 'warn\\n' >&2\n  exit 0\nfi\nprintf 'unexpected git invocation after oversized preflight: %s\\n' \"$*\" >&2\nexit 2\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let state = OrganizationState {
            connections: vec![local_connection(repo_path.display().to_string())],
            repository_sync_jobs: vec![local_repository_sync_job(
                "sync_job_local_combined_oversized_preflight",
                RepositorySyncJobStatus::Running,
                "2026-04-26T10:01:00Z",
            )],
            ..OrganizationState::default()
        };
        let failed_job =
            complete_local_repository_sync_job_with_git_command_and_output_limit_if_applicable(
                &state,
                &state.repository_sync_jobs[0],
                "2026-04-26T10:02:00Z",
                fake_git_path.as_os_str(),
                Duration::from_secs(2),
                8,
            )
            .expect("combined oversized local repository preflight output should terminally fail the job");

        assert_eq!(failed_job.id, "sync_job_local_combined_oversized_preflight");
        assert_eq!(failed_job.status, RepositorySyncJobStatus::Failed);
        assert_eq!(
            failed_job.error.as_deref(),
            Some("local repository sync preflight failed: git preflight output exceeded 8 bytes")
        );

        fs::remove_dir_all(repo_path).unwrap();
    }

    #[test]
    fn local_repository_sync_rev_parse_head_combined_output_over_cap_fails_closed() {
        let repo_path = std::env::temp_dir().join(format!(
            "sourcebot-worker-combined-head-local-git-repo-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&repo_path).unwrap();
        let fake_git_path = repo_path.join("fake-git");
        fs::write(
            &fake_git_path,
            "#!/bin/sh\nif [ \"$1\" = \"-C\" ] && [ \"$3 $4\" = \"rev-parse --is-inside-work-tree\" ]; then\n  printf 'true\\n'\n  exit 0\nfi\nif [ \"$1\" = \"-C\" ] && [ \"$3 $4\" = \"rev-parse HEAD\" ]; then\n  printf '0123456789abcdef0123456789abcdef01234567\\n'\n  printf 'combined stderr warning\\n' >&2\n  exit 0\nfi\nprintf 'unexpected git invocation after oversized HEAD output: %s\\n' \"$*\" >&2\nexit 2\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let state = OrganizationState {
            connections: vec![local_connection(repo_path.display().to_string())],
            repository_sync_jobs: vec![local_repository_sync_job(
                "sync_job_local_combined_oversized_head",
                RepositorySyncJobStatus::Running,
                "2026-04-26T10:01:00Z",
            )],
            ..OrganizationState::default()
        };
        let failed_job =
            complete_local_repository_sync_job_with_git_command_and_output_limit_if_applicable(
                &state,
                &state.repository_sync_jobs[0],
                "2026-04-26T10:02:00Z",
                fake_git_path.as_os_str(),
                Duration::from_secs(2),
                64,
            )
            .expect(
                "combined oversized local repository HEAD output should terminally fail the job",
            );

        assert_eq!(failed_job.id, "sync_job_local_combined_oversized_head");
        assert_eq!(failed_job.status, RepositorySyncJobStatus::Failed);
        assert_eq!(
            failed_job.error.as_deref(),
            Some("local repository sync execution failed: git rev-parse HEAD output exceeded 64 bytes")
        );

        fs::remove_dir_all(repo_path).unwrap();
    }

    #[test]
    fn local_repository_sync_preflight_times_out_and_fails_closed() {
        let repo_path = std::env::temp_dir().join(format!(
            "sourcebot-worker-timeout-local-git-repo-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&repo_path).unwrap();
        let fake_git_path = repo_path.join("fake-git");
        fs::write(&fake_git_path, "#!/bin/sh\nsleep 2\necho true\n").unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let state = OrganizationState {
            connections: vec![local_connection(repo_path.display().to_string())],
            repository_sync_jobs: vec![local_repository_sync_job(
                "sync_job_local_timeout",
                RepositorySyncJobStatus::Running,
                "2026-04-26T10:01:00Z",
            )],
            ..OrganizationState::default()
        };
        let failed_job = complete_local_repository_sync_job_with_git_command_if_applicable(
            &state,
            &state.repository_sync_jobs[0],
            "2026-04-26T10:02:00Z",
            fake_git_path.as_os_str(),
            Duration::from_millis(50),
        )
        .expect("timed-out local repository preflight should terminally fail the job");

        assert_eq!(failed_job.id, "sync_job_local_timeout");
        assert_eq!(failed_job.status, RepositorySyncJobStatus::Failed);
        assert!(
            failed_job
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("local repository sync preflight failed: git preflight timed out"),
            "timed out preflight should surface an operator-visible bounded failure: {failed_job:?}"
        );

        fs::remove_dir_all(repo_path).unwrap();
    }

    #[test]
    fn local_repository_sync_git_show_output_over_cap_fails_before_artifact_persistence() {
        let repo_path = std::env::temp_dir().join(format!(
            "sourcebot-worker-oversized-local-git-repo-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&repo_path).unwrap();
        Command::new("git")
            .arg("init")
            .arg(&repo_path)
            .output()
            .expect("git init should run");
        fs::write(repo_path.join("large.txt"), "x".repeat(128)).unwrap();
        Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .args(["add", "large.txt"])
            .output()
            .expect("git add should run");
        Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .args(["-c", "user.email=worker@example.test"])
            .args(["-c", "user.name=Worker Test"])
            .args(["commit", "-m", "large fixture"])
            .output()
            .expect("git commit should run");

        let state = OrganizationState {
            connections: vec![local_connection(repo_path.display().to_string())],
            repository_sync_jobs: vec![local_repository_sync_job(
                "sync_job_local_oversized_output",
                RepositorySyncJobStatus::Running,
                "2026-04-26T10:01:00Z",
            )],
            ..OrganizationState::default()
        };
        let failed_job =
            complete_local_repository_sync_job_with_git_command_and_output_limit_if_applicable(
                &state,
                &state.repository_sync_jobs[0],
                "2026-04-26T10:02:00Z",
                OsStr::new("git"),
                Duration::from_secs(2),
                64,
            )
            .expect("oversized local repository Git output should terminally fail the job");

        assert_eq!(failed_job.id, "sync_job_local_oversized_output");
        assert_eq!(failed_job.status, RepositorySyncJobStatus::Failed);
        let error = failed_job.error.as_deref().unwrap_or_default();
        assert!(
            error.starts_with("local repository sync execution failed"),
            "oversized output should use stable execution failure prefix: {failed_job:?}"
        );
        assert!(
            error.contains("git show HEAD:<tracked-path> output exceeded 64 bytes"),
            "oversized output should identify capped local Git command: {failed_job:?}"
        );
        let manifest_path = local_repository_sync_manifest_path(
            &repo_path.display().to_string(),
            &state.repository_sync_jobs[0],
        );
        assert!(
            !manifest_path.exists(),
            "oversized output must fail before manifest persistence"
        );
        assert!(
            !manifest_path
                .parent()
                .unwrap()
                .join("snapshot")
                .join("large.txt")
                .exists(),
            "oversized output must fail before snapshot persistence"
        );

        fs::remove_dir_all(repo_path).unwrap();
    }

    #[test]
    fn local_repository_sync_malformed_current_branch_fails_before_artifact_persistence() {
        let repo_path = std::env::temp_dir().join(format!(
            "sourcebot-worker-malformed-branch-local-git-repo-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&repo_path).unwrap();
        let fake_git_path = repo_path.join("fake-git");
        fs::write(
            &fake_git_path,
            "#!/bin/sh\nif [ \"$1\" = \"-C\" ] && [ \"$3 $4\" = \"rev-parse --is-inside-work-tree\" ]; then\n  printf 'true\\n'\n  exit 0\nfi\nif [ \"$1\" = \"-C\" ] && [ \"$3 $4\" = \"rev-parse HEAD\" ]; then\n  printf '0123456789abcdef0123456789abcdef01234567\\n'\n  exit 0\nfi\nif [ \"$1\" = \"-C\" ] && [ \"$3 $4 $5 $6\" = \"ls-tree -rz --name-only HEAD\" ]; then\n  printf 'README.md\\0'\n  exit 0\nfi\nif [ \"$1\" = \"-C\" ] && [ \"$3 $4 $5\" = \"symbolic-ref --short HEAD\" ]; then\n  printf 'main\\nbranch-injection\\n'\n  exit 0\nfi\nif [ \"$1\" = \"-C\" ] && [ \"$3\" = \"show\" ]; then\n  printf 'safe content\\n'\n  exit 0\nfi\nprintf 'unexpected git invocation: %s\\n' \"$*\" >&2\nexit 2\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let state = OrganizationState {
            connections: vec![local_connection(repo_path.display().to_string())],
            repository_sync_jobs: vec![local_repository_sync_job(
                "sync_job_local_malformed_branch",
                RepositorySyncJobStatus::Running,
                "2026-04-26T10:01:00Z",
            )],
            ..OrganizationState::default()
        };
        let failed_job = complete_local_repository_sync_job_with_git_command_if_applicable(
            &state,
            &state.repository_sync_jobs[0],
            "2026-04-26T10:02:00Z",
            fake_git_path.as_os_str(),
            Duration::from_secs(2),
        )
        .expect("malformed local repository branch metadata should terminally fail the job");

        assert_eq!(failed_job.id, "sync_job_local_malformed_branch");
        assert_eq!(failed_job.status, RepositorySyncJobStatus::Failed);
        assert_eq!(
            failed_job.error.as_deref(),
            Some("local repository sync execution failed: git symbolic-ref --short HEAD returned malformed branch name")
        );
        let manifest_path = local_repository_sync_manifest_path(
            &repo_path.display().to_string(),
            &state.repository_sync_jobs[0],
        );
        assert!(
            !manifest_path.exists(),
            "malformed branch metadata must fail before manifest persistence"
        );
        assert!(
            !manifest_path
                .parent()
                .unwrap()
                .join("snapshot")
                .join("README.md")
                .exists(),
            "malformed branch metadata must fail before snapshot persistence"
        );

        fs::remove_dir_all(repo_path).unwrap();
    }

    #[tokio::test]
    async fn run_repository_sync_claim_tick_succeeds_for_detached_local_git_head() {
        let repo_path = std::env::temp_dir().join(format!(
            "sourcebot-worker-detached-local-git-repo-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&repo_path).unwrap();
        Command::new("git")
            .arg("init")
            .arg(&repo_path)
            .output()
            .expect("git init should run");
        fs::write(repo_path.join("README.md"), "detached local sync fixture\n").unwrap();
        Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .args(["add", "README.md"])
            .output()
            .expect("git add should run");
        Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .args(["-c", "user.email=worker@example.test"])
            .args(["-c", "user.name=Worker Test"])
            .args(["commit", "-m", "detached fixture"])
            .output()
            .expect("git commit should run");
        let expected_head = String::from_utf8(
            Command::new("git")
                .arg("-C")
                .arg(&repo_path)
                .args(["rev-parse", "HEAD"])
                .output()
                .expect("git rev-parse HEAD should run")
                .stdout,
        )
        .unwrap()
        .trim()
        .to_owned();
        Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .args(["checkout", "--detach", "HEAD"])
            .output()
            .expect("git checkout --detach should run");

        let store = InMemoryOrganizationStore::new(OrganizationState {
            connections: vec![local_connection(repo_path.display().to_string())],
            repository_sync_jobs: vec![local_repository_sync_job(
                "sync_job_local_detached",
                RepositorySyncJobStatus::Queued,
                "2026-04-26T10:01:00Z",
            )],
            ..OrganizationState::default()
        });

        let completed_job = run_repository_sync_claim_tick(
            &store,
            StubRepositorySyncJobExecutionOutcome::Succeeded,
        )
        .await
        .unwrap()
        .expect("queued detached local repository sync job should be terminally recorded");

        assert_eq!(completed_job.id, "sync_job_local_detached");
        assert_eq!(completed_job.status, RepositorySyncJobStatus::Succeeded);
        assert_eq!(
            completed_job.synced_revision.as_deref(),
            Some(expected_head.as_str())
        );
        assert_eq!(completed_job.synced_branch.as_deref(), Some("HEAD"));
        assert_eq!(completed_job.synced_content_file_count, Some(1));

        let manifest_path =
            local_repository_sync_manifest_path(&repo_path.display().to_string(), &completed_job);
        let manifest = fs::read_to_string(&manifest_path).expect("manifest should be written");
        assert!(manifest.contains("branch=HEAD\n"));
        assert!(
            manifest_path
                .parent()
                .unwrap()
                .join("snapshot")
                .join("README.md")
                .exists(),
            "detached local sync should still persist tracked HEAD snapshot artifacts"
        );

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(persisted.repository_sync_jobs[0], completed_job);

        fs::remove_dir_all(repo_path).unwrap();
    }

    #[tokio::test]
    async fn run_repository_sync_claim_tick_fails_closed_for_empty_local_git_repository_path() {
        let repo_path = std::env::temp_dir().join(format!(
            "sourcebot-worker-empty-local-git-repo-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&repo_path).unwrap();
        Command::new("git")
            .arg("init")
            .arg(&repo_path)
            .output()
            .expect("git init should run");

        let store = InMemoryOrganizationStore::new(OrganizationState {
            connections: vec![local_connection(repo_path.display().to_string())],
            repository_sync_jobs: vec![local_repository_sync_job(
                "sync_job_local_empty",
                RepositorySyncJobStatus::Queued,
                "2026-04-26T10:01:00Z",
            )],
            ..OrganizationState::default()
        });

        let failed_job = run_repository_sync_claim_tick(
            &store,
            StubRepositorySyncJobExecutionOutcome::Succeeded,
        )
        .await
        .unwrap()
        .expect("queued empty local repository sync job should be terminally recorded");

        assert_eq!(failed_job.id, "sync_job_local_empty");
        assert_eq!(failed_job.status, RepositorySyncJobStatus::Failed);
        assert!(
            failed_job
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("local repository sync execution failed"),
            "empty local repo should fail the real Git execution step after preflight: {failed_job:?}"
        );

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(persisted.repository_sync_jobs[0], failed_job);

        fs::remove_dir_all(repo_path).unwrap();
    }

    #[tokio::test]
    async fn run_repository_sync_claim_tick_succeeds_for_reachable_generic_git_remote_metadata_only(
    ) {
        let repo_path = std::env::temp_dir().join(format!(
            "sourcebot-worker-generic-git-repo-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let bare_repo_path = repo_path.with_extension("git");
        fs::create_dir_all(&repo_path).unwrap();
        Command::new("git")
            .arg("init")
            .arg(&repo_path)
            .output()
            .expect("git init should run");
        fs::write(repo_path.join("README.md"), "generic git sync fixture\n").unwrap();
        Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .args(["add", "README.md"])
            .output()
            .expect("git add should run");
        Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .args(["-c", "user.email=worker@example.test"])
            .args(["-c", "user.name=Worker Test"])
            .args(["commit", "-m", "initial generic fixture"])
            .output()
            .expect("git commit should run");
        Command::new("git")
            .args(["clone", "--bare"])
            .arg(&repo_path)
            .arg(&bare_repo_path)
            .output()
            .expect("git clone --bare should run");

        let expected_head = String::from_utf8(
            Command::new("git")
                .arg("-C")
                .arg(&repo_path)
                .args(["rev-parse", "HEAD"])
                .output()
                .expect("git rev-parse should run")
                .stdout,
        )
        .unwrap()
        .trim()
        .to_owned();
        let store = InMemoryOrganizationStore::new(OrganizationState {
            connections: vec![generic_git_connection(bare_repo_path.display().to_string())],
            repository_sync_jobs: vec![generic_git_repository_sync_job(
                "sync_job_generic_git_valid",
                RepositorySyncJobStatus::Queued,
                "2026-04-26T10:01:00Z",
            )],
            ..OrganizationState::default()
        });

        let completed_job =
            run_repository_sync_claim_tick(&store, StubRepositorySyncJobExecutionOutcome::Failed)
                .await
                .unwrap()
                .expect("queued generic Git repository sync job should be terminally recorded");

        assert_eq!(completed_job.id, "sync_job_generic_git_valid");
        assert_eq!(completed_job.status, RepositorySyncJobStatus::Succeeded);
        assert_eq!(completed_job.error, None);
        assert_eq!(
            completed_job.synced_revision.as_deref(),
            Some(expected_head.as_str())
        );
        assert_eq!(completed_job.synced_branch.as_deref(), Some("master"));
        assert_eq!(completed_job.synced_content_file_count, None);
        assert!(
            !bare_repo_path.join(".sourcebot").exists(),
            "generic Git metadata probe must not claim local manifest/snapshot/search-index artifacts"
        );

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(persisted.repository_sync_jobs[0], completed_job);

        fs::remove_dir_all(repo_path).unwrap();
        fs::remove_dir_all(bare_repo_path).unwrap();
    }

    #[tokio::test]
    async fn run_repository_sync_claim_tick_fails_closed_for_empty_generic_git_remote() {
        let bare_repo_path = std::env::temp_dir().join(format!(
            "sourcebot-worker-empty-generic-git-repo-{}.git",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        Command::new("git")
            .args(["init", "--bare"])
            .arg(&bare_repo_path)
            .output()
            .expect("git init --bare should run");
        let store = InMemoryOrganizationStore::new(OrganizationState {
            connections: vec![generic_git_connection(bare_repo_path.display().to_string())],
            repository_sync_jobs: vec![generic_git_repository_sync_job(
                "sync_job_generic_git_empty",
                RepositorySyncJobStatus::Queued,
                "2026-04-26T10:01:00Z",
            )],
            ..OrganizationState::default()
        });

        let failed_job = run_repository_sync_claim_tick(
            &store,
            StubRepositorySyncJobExecutionOutcome::Succeeded,
        )
        .await
        .unwrap()
        .expect("queued empty generic Git repository sync job should be terminally recorded");

        assert_eq!(failed_job.id, "sync_job_generic_git_empty");
        assert_eq!(failed_job.status, RepositorySyncJobStatus::Failed);
        assert!(
            failed_job
                .error
                .as_deref()
                .unwrap_or_default()
                .starts_with(GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX),
            "empty generic Git remote should produce operator-visible failure detail: {failed_job:?}"
        );
        assert_eq!(failed_job.synced_revision, None);
        assert_eq!(failed_job.synced_branch, None);
        assert_eq!(failed_job.synced_content_file_count, None);

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(persisted.repository_sync_jobs[0], failed_job);

        fs::remove_dir_all(bare_repo_path).unwrap();
    }

    #[tokio::test]
    async fn run_repository_sync_claim_tick_fails_closed_for_unreachable_generic_git_remote() {
        let missing_remote_path = std::env::temp_dir().join(format!(
            "sourcebot-worker-missing-generic-git-repo-{}.git",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = InMemoryOrganizationStore::new(OrganizationState {
            connections: vec![generic_git_connection(
                missing_remote_path.display().to_string(),
            )],
            repository_sync_jobs: vec![generic_git_repository_sync_job(
                "sync_job_generic_git_unreachable",
                RepositorySyncJobStatus::Queued,
                "2026-04-26T10:01:00Z",
            )],
            ..OrganizationState::default()
        });

        let failed_job = run_repository_sync_claim_tick(
            &store,
            StubRepositorySyncJobExecutionOutcome::Succeeded,
        )
        .await
        .unwrap()
        .expect("queued unreachable generic Git repository sync job should be terminally recorded");

        assert_eq!(failed_job.id, "sync_job_generic_git_unreachable");
        assert_eq!(failed_job.status, RepositorySyncJobStatus::Failed);
        assert!(
            failed_job
                .error
                .as_deref()
                .unwrap_or_default()
                .starts_with(GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX),
            "unreachable generic Git remote should produce operator-visible failure detail: {failed_job:?}"
        );
        assert_eq!(failed_job.synced_revision, None);
        assert_eq!(failed_job.synced_branch, None);
        assert_eq!(failed_job.synced_content_file_count, None);

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(persisted.repository_sync_jobs[0], failed_job);
    }

    #[test]
    fn generic_git_remote_scp_like_credential_base_url_fails_closed_before_spawning_git() {
        let repo_path = std::env::temp_dir().join(format!(
            "sourcebot-worker-scp-like-credential-generic-git-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&repo_path).unwrap();
        let marker_path = repo_path.join("spawned-marker");
        let fake_git_path = repo_path.join("fake-git");
        fs::write(
            &fake_git_path,
            format!("#!/bin/sh\ntouch '{}'\nexit 0\n", marker_path.display()),
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let error = match run_generic_git_repository_sync_execution(
            fake_git_path.as_os_str(),
            "git@example.invalid:acme/repo.git",
            Duration::from_secs(2),
        ) {
            Ok(_) => {
                panic!("scp-like Generic Git base_url with a userinfo component must fail closed")
            }
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: Generic Git base_url must not include embedded credentials"
        );
        assert!(
            !marker_path.exists(),
            "scp-like credential base_url validation should fail before spawning git"
        );

        fs::remove_dir_all(repo_path).unwrap();
    }

    #[test]
    fn generic_git_remote_option_like_base_url_fails_closed_before_spawning_git() {
        let repo_path = std::env::temp_dir().join(format!(
            "sourcebot-worker-option-like-generic-git-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&repo_path).unwrap();
        let marker_path = repo_path.join("spawned-marker");
        let fake_git_path = repo_path.join("fake-git");
        fs::write(
            &fake_git_path,
            format!("#!/bin/sh\ntouch '{}'\nexit 0\n", marker_path.display()),
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let error = match run_generic_git_repository_sync_execution(
            fake_git_path.as_os_str(),
            "--upload-pack=/bin/echo",
            Duration::from_secs(2),
        ) {
            Ok(_) => {
                panic!("option-like Generic Git base_url must not be interpreted as git flags")
            }
            Err(error) => error,
        };

        assert_eq!(
            error,
            "generic Git repository sync execution failed: Generic Git base_url must not start with '-'"
        );
        assert!(
            !marker_path.exists(),
            "option-like base_url validation should fail before spawning git"
        );

        fs::remove_dir_all(repo_path).unwrap();
    }

    #[test]
    fn generic_git_remote_with_malformed_head_advertisement_fails_closed() {
        let repo_path = std::env::temp_dir().join(format!(
            "sourcebot-worker-malformed-generic-git-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&repo_path).unwrap();
        let fake_git_path = repo_path.join("fake-git");
        fs::write(
            &fake_git_path,
            "#!/bin/sh\nif [ \"$1 $2 $3\" = \"ls-remote --symref --\" ]; then\n  printf '%s\\t%s\\n' '0123456789abcdef0123456789abcdef01234567' 'HEAD'\n  exit 0\nfi\nprintf '%s\\t%s\\n' 'not-a-revision' 'refs/heads/main'\nprintf '%s\\t%s\\n' '0123456789abcdef0123456789abcdef01234567' 'refs/heads/.hidden'\nprintf '%s\\t%s\\n' '0123456789abcdef0123456789abcdef01234567' 'refs/heads/release.lock'\nprintf '%s\\t%s\\n' '0123456789abcdef0123456789abcdef01234567' 'refs/heads/bad\\177ref'\nprintf '%s\\t%s\\t%s\\n' '0123456789abcdef0123456789abcdef01234567' 'refs/heads/bad' 'ref'\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_git_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_git_path, permissions).unwrap();

        let error = match run_generic_git_repository_sync_execution(
            fake_git_path.as_os_str(),
            "https://example.test/repo.git",
            Duration::from_secs(2),
        ) {
            Ok(execution) => panic!(
                "malformed Generic Git head advertisement should fail closed, got revision={} branch={}",
                execution.revision, execution.branch
            ),
            Err(error) => error,
        };

        assert!(
            error.starts_with(GENERIC_GIT_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX),
            "malformed head advertisements should fail closed with operator-visible prefix: {error}"
        );
        assert!(
            error.contains("advertised malformed ref line"),
            "malformed head advertisements should not be treated as synced metadata: {error}"
        );

        fs::remove_dir_all(repo_path).unwrap();
    }

    #[test]
    fn oversized_child_output_is_terminated_after_the_retained_cap() {
        let child = Command::new("python3")
            .args([
                "-c",
                "import os, sys, time\nchunk = b'x' * 65536\nfor _ in range(4096):\n    os.write(sys.stdout.fileno(), chunk)\ntime.sleep(5)\n",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("oversized output fixture should spawn");

        let output = wait_for_child_output_with_timeout_and_output_limit(
            child,
            Duration::from_secs(2),
            1024,
        )
        .expect("oversized child output should be collected without io failure")
        .expect("oversized output should trigger output-limit termination, not timeout");

        assert!(!output.status.success());
        assert_eq!(output.stdout.len(), 1025);
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn generic_git_remote_with_many_heads_does_not_deadlock_on_piped_output() {
        let repo_path = std::env::temp_dir().join(format!(
            "sourcebot-worker-many-heads-generic-git-repo-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let bare_repo_path = repo_path.with_extension("git");
        fs::create_dir_all(&repo_path).unwrap();
        Command::new("git")
            .arg("init")
            .arg(&repo_path)
            .output()
            .expect("git init should run");
        fs::write(repo_path.join("README.md"), "many heads fixture\n").unwrap();
        Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .args(["add", "README.md"])
            .output()
            .expect("git add should run");
        Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .args(["-c", "user.email=worker@example.test"])
            .args(["-c", "user.name=Worker Test"])
            .args(["commit", "-m", "many heads fixture"])
            .output()
            .expect("git commit should run");
        for index in 0..3_000 {
            Command::new("git")
                .arg("-C")
                .arg(&repo_path)
                .args(["branch", &format!("branch-{index}")])
                .output()
                .expect("git branch should run");
        }
        Command::new("git")
            .args(["clone", "--bare"])
            .arg(&repo_path)
            .arg(&bare_repo_path)
            .output()
            .expect("git clone --bare should run");

        let execution = run_generic_git_repository_sync_execution(
            OsStr::new("git"),
            &bare_repo_path.display().to_string(),
            Duration::from_secs(2),
        )
        .expect("large Generic Git head listings should be drained while waiting for process exit");

        assert!(!execution.revision.is_empty());
        assert!(!execution.branch.is_empty());

        fs::remove_dir_all(repo_path).unwrap();
        fs::remove_dir_all(bare_repo_path).unwrap();
    }

    #[tokio::test]
    async fn run_repository_sync_claim_tick_fails_closed_for_local_git_repository_without_tracked_content(
    ) {
        let repo_path = std::env::temp_dir().join(format!(
            "sourcebot-worker-empty-tree-local-git-repo-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&repo_path).unwrap();
        Command::new("git")
            .arg("init")
            .arg(&repo_path)
            .output()
            .expect("git init should run");
        Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .args(["-c", "user.email=worker@example.test"])
            .args(["-c", "user.name=Worker Test"])
            .args(["commit", "--allow-empty", "-m", "empty fixture"])
            .output()
            .expect("git commit should run");

        let store = InMemoryOrganizationStore::new(OrganizationState {
            connections: vec![local_connection(repo_path.display().to_string())],
            repository_sync_jobs: vec![local_repository_sync_job(
                "sync_job_local_empty_tree",
                RepositorySyncJobStatus::Queued,
                "2026-04-26T10:01:00Z",
            )],
            ..OrganizationState::default()
        });

        let failed_job = run_repository_sync_claim_tick(
            &store,
            StubRepositorySyncJobExecutionOutcome::Succeeded,
        )
        .await
        .unwrap()
        .expect("queued local repository sync job should be terminally recorded");

        assert_eq!(failed_job.id, "sync_job_local_empty_tree");
        assert_eq!(failed_job.status, RepositorySyncJobStatus::Failed);
        assert!(
            failed_job
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("local repository sync execution failed: git ls-tree -rz --name-only HEAD found no tracked content"),
            "empty committed local repo should fail the content discovery step: {failed_job:?}"
        );

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(persisted.repository_sync_jobs[0], failed_job);

        fs::remove_dir_all(repo_path).unwrap();
    }

    #[tokio::test]
    async fn run_repository_sync_claim_tick_succeeds_for_real_local_git_repository_path() {
        let repo_path = std::env::temp_dir().join(format!(
            "sourcebot-worker-local-git-repo-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&repo_path).unwrap();
        Command::new("git")
            .arg("init")
            .arg(&repo_path)
            .output()
            .expect("git init should run");
        fs::write(repo_path.join("README.md"), "local repo sync fixture\n").unwrap();
        fs::create_dir_all(repo_path.join("src")).unwrap();
        fs::write(repo_path.join("src/lib.rs"), "pub fn fixture() {}\n").unwrap();
        fs::write(repo_path.join("untracked.txt"), "must not appear\n").unwrap();
        Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .args(["add", "README.md", "src/lib.rs"])
            .output()
            .expect("git add should run");
        Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .args(["-c", "user.email=worker@example.test"])
            .args(["-c", "user.name=Worker Test"])
            .args(["commit", "-m", "initial fixture"])
            .output()
            .expect("git commit should run");

        let store = InMemoryOrganizationStore::new(OrganizationState {
            connections: vec![local_connection(repo_path.display().to_string())],
            repository_sync_jobs: vec![local_repository_sync_job(
                "sync_job_local_valid",
                RepositorySyncJobStatus::Queued,
                "2026-04-26T10:01:00Z",
            )],
            ..OrganizationState::default()
        });

        let completed_job =
            run_repository_sync_claim_tick(&store, StubRepositorySyncJobExecutionOutcome::Failed)
                .await
                .unwrap()
                .expect("queued local repository sync job should be terminally recorded");

        assert_eq!(completed_job.id, "sync_job_local_valid");
        assert_eq!(completed_job.status, RepositorySyncJobStatus::Succeeded);
        assert!(completed_job.started_at.is_some());
        assert!(completed_job.finished_at.is_some());
        assert_eq!(completed_job.error, None);
        let expected_head = String::from_utf8(
            Command::new("git")
                .arg("-C")
                .arg(&repo_path)
                .args(["rev-parse", "HEAD"])
                .output()
                .expect("git rev-parse should run")
                .stdout,
        )
        .unwrap()
        .trim()
        .to_owned();
        assert_eq!(
            completed_job.synced_revision.as_deref(),
            Some(expected_head.as_str())
        );
        assert_eq!(completed_job.synced_branch.as_deref(), Some("master"));
        assert_eq!(completed_job.synced_content_file_count, Some(2));

        let manifest_path = repo_path
            .join(".sourcebot")
            .join("local-sync")
            .join("org_acme")
            .join("repo_sync_job_local_valid")
            .join("sync_job_local_valid")
            .join("manifest.txt");
        let manifest =
            fs::read_to_string(&manifest_path).expect("tracked-content manifest to exist");
        assert_eq!(
            manifest,
            format!(
                "revision={expected_head}\nbranch=master\ntracked_content_file_count=2\ntracked_content_paths:\nREADME.md\nsrc/lib.rs\n"
            )
        );
        assert!(
            !manifest.contains("untracked.txt"),
            "manifest should contain only tracked HEAD paths: {manifest}"
        );
        let snapshot_dir = manifest_path
            .parent()
            .expect("manifest path should have a job directory")
            .join("snapshot");
        assert_eq!(
            fs::read_to_string(snapshot_dir.join("README.md")).unwrap(),
            "local repo sync fixture\n"
        );
        assert_eq!(
            fs::read_to_string(snapshot_dir.join("src/lib.rs")).unwrap(),
            "pub fn fixture() {}\n"
        );
        assert!(
            !snapshot_dir.join("untracked.txt").exists(),
            "snapshot should contain only tracked HEAD content"
        );
        let search_index_path = manifest_path
            .parent()
            .expect("manifest path should have a job directory")
            .join("search-index.json");
        assert!(
            search_index_path.is_file(),
            "successful local sync should persist a bounded search-index artifact"
        );
        fs::write(
            snapshot_dir.join("README.md"),
            "changed after search index\n",
        )
        .unwrap();
        let search_index = sourcebot_search::LocalSearchStore::from_index_artifact(
            "repo_sync_job_local_valid",
            &search_index_path,
        )
        .unwrap();
        assert!(search_index
            .search("local repo sync fixture", Some("repo_sync_job_local_valid"))
            .unwrap()
            .results
            .iter()
            .any(|result| result.path == "README.md"));
        assert!(search_index
            .search(
                "changed after search index",
                Some("repo_sync_job_local_valid")
            )
            .unwrap()
            .results
            .is_empty());

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(persisted.repository_sync_jobs[0], completed_job);

        fs::remove_dir_all(repo_path).unwrap();
    }

    #[tokio::test]
    async fn run_repository_sync_claim_tick_persists_index_artifact_for_skipped_only_local_repositories(
    ) {
        let repo_path = std::env::temp_dir().join(format!(
            "sourcebot-worker-local-git-skipped-only-repo-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&repo_path).unwrap();
        Command::new("git")
            .arg("init")
            .arg(&repo_path)
            .output()
            .expect("git init should run");
        fs::write(repo_path.join("binary.bin"), b"\0\0\0skipped\0").unwrap();
        Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .args(["add", "binary.bin"])
            .output()
            .expect("git add should run");
        Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .args(["-c", "user.email=worker@example.test"])
            .args(["-c", "user.name=Worker Test"])
            .args(["commit", "-m", "skipped-only fixture"])
            .output()
            .expect("git commit should run");

        let store = InMemoryOrganizationStore::new(OrganizationState {
            connections: vec![local_connection(repo_path.display().to_string())],
            repository_sync_jobs: vec![local_repository_sync_job(
                "sync_job_local_skipped_only",
                RepositorySyncJobStatus::Queued,
                "2026-04-26T10:00:30Z",
            )],
            ..OrganizationState::default()
        });

        let completed_job =
            run_repository_sync_claim_tick(&store, StubRepositorySyncJobExecutionOutcome::Failed)
                .await
                .unwrap()
                .expect(
                    "queued skipped-only local repository sync job should be terminally recorded",
                );

        assert_eq!(completed_job.id, "sync_job_local_skipped_only");
        assert_eq!(completed_job.status, RepositorySyncJobStatus::Succeeded);
        assert_eq!(completed_job.error, None);
        assert_eq!(completed_job.synced_content_file_count, Some(1));

        let search_index_path = repo_path
            .join(".sourcebot")
            .join("local-sync")
            .join("org_acme")
            .join("repo_sync_job_local_skipped_only")
            .join("sync_job_local_skipped_only")
            .join("search-index.json");
        assert!(
            search_index_path.is_file(),
            "successful skipped-only local sync should still persist an indexed_empty artifact"
        );
        let search_index = sourcebot_search::LocalSearchStore::from_index_artifact(
            "repo_sync_job_local_skipped_only",
            &search_index_path,
        )
        .unwrap();
        let index_status = search_index
            .repository_index_status("repo_sync_job_local_skipped_only")
            .unwrap()
            .expect("empty repository artifact should expose index status");
        assert_eq!(
            index_status.status,
            sourcebot_search::RepositoryIndexState::IndexedEmpty
        );
        assert_eq!(index_status.indexed_file_count, 0);

        fs::remove_dir_all(repo_path).unwrap();
    }

    #[tokio::test]
    async fn run_repository_sync_claim_tick_snapshots_tracked_head_content_for_local_connections() {
        let repo_path = std::env::temp_dir().join(format!(
            "sourcebot-worker-local-git-spaced-path-repo-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&repo_path).unwrap();
        Command::new("git")
            .arg("init")
            .arg(&repo_path)
            .output()
            .expect("git init should run");
        let spaced_path = " spaced fixture .txt";
        let tabbed_path = "tabbed\tfixture.txt";
        fs::write(repo_path.join(spaced_path), "tracked path with spaces\n").unwrap();
        fs::write(repo_path.join(tabbed_path), "tracked path with tab\n").unwrap();
        Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .args(["add", spaced_path, tabbed_path])
            .output()
            .expect("git add should run");
        Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .args(["-c", "user.email=worker@example.test"])
            .args(["-c", "user.name=Worker Test"])
            .args(["commit", "-m", "spaced fixture"])
            .output()
            .expect("git commit should run");

        let store = InMemoryOrganizationStore::new(OrganizationState {
            connections: vec![local_connection(repo_path.display().to_string())],
            repository_sync_jobs: vec![local_repository_sync_job(
                "sync_job_local_spaced_path",
                RepositorySyncJobStatus::Queued,
                "2026-04-26T10:01:00Z",
            )],
            ..OrganizationState::default()
        });

        let completed_job =
            run_repository_sync_claim_tick(&store, StubRepositorySyncJobExecutionOutcome::Failed)
                .await
                .unwrap()
                .expect("queued local repository sync job should be terminally recorded");

        assert_eq!(completed_job.id, "sync_job_local_spaced_path");
        assert_eq!(completed_job.status, RepositorySyncJobStatus::Succeeded);
        assert_eq!(completed_job.synced_content_file_count, Some(2));

        let snapshot_dir = repo_path
            .join(".sourcebot")
            .join("local-sync")
            .join("org_acme")
            .join("repo_sync_job_local_spaced_path")
            .join("sync_job_local_spaced_path")
            .join("snapshot");
        assert_eq!(
            fs::read_to_string(snapshot_dir.join(spaced_path)).unwrap(),
            "tracked path with spaces\n"
        );
        assert_eq!(
            fs::read_to_string(snapshot_dir.join(tabbed_path)).unwrap(),
            "tracked path with tab\n"
        );

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(persisted.repository_sync_jobs[0], completed_job);

        fs::remove_dir_all(repo_path).unwrap();
    }

    #[tokio::test]
    async fn run_repository_sync_claim_tick_surfaces_atomic_claim_and_complete_failures_without_mutating_state(
    ) {
        let store = FailingStoreOrganizationWriteStore::new(OrganizationState {
            repository_sync_jobs: vec![
                repository_sync_job(
                    "sync_job_newer",
                    RepositorySyncJobStatus::Queued,
                    "2026-04-26T10:02:00Z",
                ),
                repository_sync_job(
                    "sync_job_oldest",
                    RepositorySyncJobStatus::Queued,
                    "2026-04-26T10:01:00Z",
                ),
            ],
            ..OrganizationState::default()
        });

        let error = run_repository_sync_claim_tick(
            &store,
            StubRepositorySyncJobExecutionOutcome::Succeeded,
        )
        .await
        .expect_err("synthetic claim-and-complete should fail");

        assert!(error
            .to_string()
            .contains("synthetic claim_and_complete_next_repository_sync_job failure"));

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(
            persisted.repository_sync_jobs[0].status,
            RepositorySyncJobStatus::Queued
        );
        assert_eq!(
            persisted.repository_sync_jobs[1].status,
            RepositorySyncJobStatus::Queued
        );
        assert_eq!(persisted.repository_sync_jobs[1].started_at, None);
        assert_eq!(persisted.repository_sync_jobs[1].finished_at, None);
    }
}
