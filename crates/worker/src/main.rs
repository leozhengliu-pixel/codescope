use sourcebot_api::auth::build_organization_store;
use sourcebot_config::{
    AppConfig, StubRepositorySyncJobExecutionOutcomeConfig,
    StubReviewAgentRunExecutionOutcomeConfig,
};
use sourcebot_worker::{
    run_worker_tick, StubRepositorySyncJobExecutionOutcome, StubReviewAgentRunExecutionOutcome,
    WorkerTickOutcome,
};
use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Duration,
};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stderr)
        .compact()
        .init();

    let config = AppConfig::from_env();
    let store = build_organization_store(
        config.organization_state_path.clone(),
        config.database_url.as_deref(),
    );
    let review_agent_stub_outcome = match config.stub_review_agent_run_execution_outcome()? {
        StubReviewAgentRunExecutionOutcomeConfig::Completed => {
            StubReviewAgentRunExecutionOutcome::Completed
        }
        StubReviewAgentRunExecutionOutcomeConfig::Failed => {
            StubReviewAgentRunExecutionOutcome::Failed
        }
    };
    let repository_sync_stub_outcome = match config.stub_repository_sync_job_execution_outcome()? {
        StubRepositorySyncJobExecutionOutcomeConfig::Succeeded => {
            StubRepositorySyncJobExecutionOutcome::Succeeded
        }
        StubRepositorySyncJobExecutionOutcomeConfig::Failed => {
            StubRepositorySyncJobExecutionOutcome::Failed
        }
    };

    let max_ticks = worker_max_ticks_from_env()?;
    let idle_sleep = worker_idle_sleep_from_env()?;
    let status_path = worker_status_path_from_env()?;

    info!(
        organization_state_path = %config.organization_state_path,
        review_agent_stub_outcome = ?review_agent_stub_outcome,
        repository_sync_stub_outcome = ?repository_sync_stub_outcome,
        max_ticks,
        idle_sleep_ms = idle_sleep.as_millis(),
        worker_status_path = status_path.as_ref().map(|path| path.display().to_string()).as_deref().unwrap_or(""),
        "worker runtime baseline resolved"
    );

    let mut last_tick = 0;
    let mut last_outcome = "not_started".to_string();
    let mut last_work_item_id: Option<String> = None;
    let mut last_work_item_status: Option<String> = None;

    if let Some(path) = status_path.as_ref() {
        write_worker_status_snapshot(path, max_ticks, 0, 0, &last_outcome, None, None, false)?;
    }

    for tick in 1..=max_ticks {
        let outcome = run_worker_tick(
            store.as_ref(),
            review_agent_stub_outcome,
            repository_sync_stub_outcome,
        )
        .await?;
        last_tick = tick;

        match outcome {
            Some(WorkerTickOutcome::ReviewAgentRun(run)) => {
                last_outcome = "review_agent_run".to_string();
                last_work_item_id = Some(run.id.clone());
                last_work_item_status = Some(format!("{:?}", run.status));
                info!(
                    tick,
                    review_agent_run_id = %run.id,
                    status = ?run.status,
                    "recorded review-agent run terminal status after worker execution"
                )
            }
            Some(WorkerTickOutcome::RepositorySyncJob(job)) => {
                last_outcome = "repository_sync_job".to_string();
                last_work_item_id = Some(job.id.clone());
                last_work_item_status = Some(format!("{:?}", job.status));
                info!(
                    tick,
                    repository_sync_job_id = %job.id,
                    organization_id = %job.organization_id,
                    repository_id = %job.repository_id,
                    connection_id = %job.connection_id,
                    status = ?job.status,
                    error = job.error.as_deref().unwrap_or(""),
                    "recorded repository-sync terminal status after worker execution"
                )
            }
            None => {
                last_outcome = "no_work".to_string();
                last_work_item_id = None;
                last_work_item_status = None;
                info!(
                    tick,
                    "no queued review-agent run or repository sync job available"
                );
                if tick < max_ticks && !idle_sleep.is_zero() {
                    tokio::time::sleep(idle_sleep).await;
                }
            }
        }

        if let Some(path) = status_path.as_ref() {
            write_worker_status_snapshot(
                path,
                max_ticks,
                tick,
                last_tick,
                &last_outcome,
                last_work_item_id.as_deref(),
                last_work_item_status.as_deref(),
                false,
            )?;
        }
    }

    if let Some(path) = status_path.as_ref() {
        write_worker_status_snapshot(
            path,
            max_ticks,
            max_ticks,
            last_tick,
            &last_outcome,
            last_work_item_id.as_deref(),
            last_work_item_status.as_deref(),
            true,
        )?;
    }

    info!(max_ticks, "completed configured bounded worker runtime");

    Ok(())
}

fn worker_max_ticks_from_env() -> anyhow::Result<u64> {
    parse_positive_u64_env("SOURCEBOT_WORKER_MAX_TICKS", 1)
}

fn worker_status_path_from_env() -> anyhow::Result<Option<PathBuf>> {
    match env::var("SOURCEBOT_WORKER_STATUS_PATH") {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                anyhow::bail!("SOURCEBOT_WORKER_STATUS_PATH must not be empty when set");
            }
            Ok(Some(PathBuf::from(trimmed)))
        }
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => {
            anyhow::bail!("SOURCEBOT_WORKER_STATUS_PATH must be valid unicode")
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn write_worker_status_snapshot(
    path: &Path,
    max_ticks: u64,
    ticks_completed: u64,
    last_tick: u64,
    last_outcome: &str,
    last_work_item_id: Option<&str>,
    last_work_item_status: Option<&str>,
    completed: bool,
) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let payload = serde_json::json!({
        "schema_version": 1,
        "completed": completed,
        "updated_at": OffsetDateTime::now_utc().format(&Rfc3339)?,
        "process_id": std::process::id(),
        "max_ticks": max_ticks,
        "ticks_completed": ticks_completed,
        "last_tick": last_tick,
        "last_outcome": last_outcome,
        "last_work_item_id": last_work_item_id,
        "last_work_item_status": last_work_item_status,
    });
    fs::write(path, serde_json::to_vec_pretty(&payload)?)?;
    Ok(())
}

fn worker_idle_sleep_from_env() -> anyhow::Result<Duration> {
    Ok(Duration::from_millis(parse_u64_env(
        "SOURCEBOT_WORKER_IDLE_SLEEP_MS",
        1000,
    )?))
}

fn parse_positive_u64_env(name: &str, default: u64) -> anyhow::Result<u64> {
    let value = parse_u64_env(name, default)?;
    if value == 0 {
        anyhow::bail!("{name} must be greater than zero");
    }
    Ok(value)
}

fn parse_u64_env(name: &str, default: u64) -> anyhow::Result<u64> {
    match env::var(name) {
        Ok(value) => value
            .parse::<u64>()
            .map_err(|_| anyhow::anyhow!("{name} must be an unsigned integer")),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(env::VarError::NotUnicode(_)) => anyhow::bail!("{name} must be valid unicode"),
    }
}
