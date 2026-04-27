use anyhow::Result;
use sourcebot_core::{complete_repository_sync_job, OrganizationStore};
use sourcebot_models::{
    ConnectionConfig, OrganizationState, RepositorySyncJob, RepositorySyncJobStatus, ReviewAgentRun,
};
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
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
const LOCAL_REPOSITORY_SYNC_PREFLIGHT_TIMEOUT: Duration = Duration::from_secs(10);
const REPOSITORY_SYNC_RUNNING_JOB_LEASE_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const REPOSITORY_SYNC_STALE_RUNNING_RECOVERY_PREFIX: &str =
    "repository sync job exceeded worker lease and was marked failed before the next claim";

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
    let connection = state
        .connections
        .iter()
        .find(|connection| connection.id == job.connection_id)?;
    let Some(ConnectionConfig::Local { repo_path }) = &connection.config else {
        return None;
    };

    let preflight = run_git_working_tree_preflight(git_command, repo_path, preflight_timeout);

    match preflight {
        Ok(Some(output))
            if output.status.success() && git_preflight_stdout_is_true(&output.stdout) =>
        {
            let execution = match run_local_repository_sync_execution(
                git_command,
                repo_path,
                job,
                preflight_timeout,
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
) -> Result<LocalRepositorySyncExecution, String> {
    let head = match run_git_command(git_command, repo_path, &["rev-parse", "HEAD"], timeout) {
        Ok(Some(output)) if output.status.success() && !output.stdout.is_empty() => {
            String::from_utf8_lossy(&output.stdout).trim().to_owned()
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

    let content_paths = match run_git_command(
        git_command,
        repo_path,
        &["ls-tree", "-r", "--name-only", "HEAD"],
        timeout,
    ) {
        Ok(Some(output)) if output.status.success() && !output.stdout.is_empty() => {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::trim)
                .filter(|path| !path.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        }
        Ok(Some(output)) if output.status.success() => {
            return Err(format!(
                "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git ls-tree -r --name-only HEAD found no tracked content"
            ))
        }
        Ok(Some(output)) => return Err(git_failure_detail("git ls-tree -r --name-only HEAD", &output)),
        Ok(None) => {
            return Err(format!(
                "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git ls-tree -r --name-only HEAD timed out after {}ms",
                timeout.as_millis()
            ))
        }
        Err(error) => return Err(format!("{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: {error}")),
    };

    match run_git_command(
        git_command,
        repo_path,
        &["symbolic-ref", "--short", "HEAD"],
        timeout,
    ) {
        Ok(Some(output)) if output.status.success() && !output.stdout.is_empty() => {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            write_local_repository_sync_snapshot(
                git_command,
                repo_path,
                job,
                &content_paths,
                timeout,
            )?;
            write_local_repository_sync_manifest(repo_path, job, &head, &branch, &content_paths)?;
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
        Ok(Some(output)) => Err(git_failure_detail("git symbolic-ref --short HEAD", &output)),
        Ok(None) => Err(format!(
            "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: git symbolic-ref --short HEAD timed out after {}ms",
            timeout.as_millis()
        )),
        Err(error) => Err(format!("{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: {error}")),
    }
}

fn write_local_repository_sync_snapshot(
    git_command: &OsStr,
    repo_path: &str,
    job: &RepositorySyncJob,
    content_paths: &[String],
    timeout: Duration,
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
        let output = match run_git_command(
            git_command,
            repo_path,
            &["show", &format!("HEAD:{content_path}")],
            timeout,
        ) {
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
    {
        Ok(path.to_path_buf())
    } else {
        Err(format!(
            "{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: tracked content path cannot be snapshotted safely: {content_path:?}"
        ))
    }
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
            continue;
        };
        let Ok(started_at) = OffsetDateTime::parse(started_at, &Rfc3339) else {
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
    format!("{LOCAL_REPOSITORY_SYNC_EXECUTION_FAILURE_PREFIX}: {detail}")
}

fn run_git_working_tree_preflight(
    git_command: &OsStr,
    repo_path: &str,
    timeout: Duration,
) -> std::io::Result<Option<Output>> {
    run_git_command(
        git_command,
        repo_path,
        &["rev-parse", "--is-inside-work-tree"],
        timeout,
    )
}

fn run_git_command(
    git_command: &OsStr,
    repo_path: &str,
    args: &[&str],
    timeout: Duration,
) -> std::io::Result<Option<Output>> {
    let mut child = Command::new(git_command)
        .arg("-C")
        .arg(repo_path)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let started_at = Instant::now();

    loop {
        if child.try_wait()?.is_some() {
            return child.wait_with_output().map(Some);
        }
        if started_at.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        let remaining = timeout.saturating_sub(started_at.elapsed());
        thread::sleep(remaining.min(Duration::from_millis(10)));
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
        complete_local_repository_sync_job_with_git_command_if_applicable,
        execute_claimed_repository_sync_job_stub_at, execute_claimed_review_agent_run_stub,
        persist_stub_review_agent_run_execution_outcome, run_repository_sync_claim_tick,
        run_review_agent_tick, run_worker_tick, safe_manifest_path_component,
        StubRepositorySyncJobExecutionOutcome, StubReviewAgentRunExecutionOutcome,
        WorkerTickOutcome,
    };
    use crate::REPOSITORY_SYNC_STUB_FAILURE_ERROR;
    use anyhow::Result;
    use async_trait::async_trait;
    use sourcebot_api::auth::FileOrganizationStore;
    use sourcebot_core::OrganizationStore;
    use sourcebot_models::{
        Connection, ConnectionConfig, ConnectionKind, OrganizationState, RepositorySyncJob,
        RepositorySyncJobStatus, ReviewAgentRun, ReviewAgentRunStatus,
    };
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        process::Command,
        sync::Mutex,
        time::{Duration, SystemTime, UNIX_EPOCH},
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

    #[test]
    fn local_sync_manifest_path_components_cannot_escape_manifest_root() {
        assert_eq!(safe_manifest_path_component("../org/acme"), "___org_acme");
        assert_eq!(safe_manifest_path_component("."), "_");
        assert_eq!(safe_manifest_path_component(".."), "__");
        assert_eq!(safe_manifest_path_component("org_acme-1"), "org_acme-1");
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
                .contains("local repository sync execution failed: git ls-tree -r --name-only HEAD found no tracked content"),
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
