use sourcebot_api::auth::build_organization_store;
use sourcebot_config::{
    AppConfig, StubRepositorySyncJobExecutionOutcomeConfig,
    StubReviewAgentRunExecutionOutcomeConfig,
};
use sourcebot_worker::{
    run_worker_tick, StubRepositorySyncJobExecutionOutcome, StubReviewAgentRunExecutionOutcome,
    WorkerTickOutcome,
};
use std::{env, time::Duration};
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

    info!(
        organization_state_path = %config.organization_state_path,
        review_agent_stub_outcome = ?review_agent_stub_outcome,
        repository_sync_stub_outcome = ?repository_sync_stub_outcome,
        max_ticks,
        idle_sleep_ms = idle_sleep.as_millis(),
        "worker runtime baseline resolved"
    );

    for tick in 1..=max_ticks {
        let outcome = run_worker_tick(
            store.as_ref(),
            review_agent_stub_outcome,
            repository_sync_stub_outcome,
        )
        .await?;

        match outcome {
            Some(WorkerTickOutcome::ReviewAgentRun(run)) => info!(
                tick,
                review_agent_run_id = %run.id,
                status = ?run.status,
                "recorded review-agent run terminal status after worker execution"
            ),
            Some(WorkerTickOutcome::RepositorySyncJob(job)) => info!(
                tick,
                repository_sync_job_id = %job.id,
                organization_id = %job.organization_id,
                repository_id = %job.repository_id,
                connection_id = %job.connection_id,
                status = ?job.status,
                error = job.error.as_deref().unwrap_or(""),
                "recorded repository-sync terminal status after worker execution"
            ),
            None => {
                info!(
                    tick,
                    "no queued review-agent run or repository sync job available"
                );
                if tick < max_ticks && !idle_sleep.is_zero() {
                    tokio::time::sleep(idle_sleep).await;
                }
            }
        }
    }

    info!(max_ticks, "completed configured bounded worker runtime");

    Ok(())
}

fn worker_max_ticks_from_env() -> anyhow::Result<u64> {
    parse_positive_u64_env("SOURCEBOT_WORKER_MAX_TICKS", 1)
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
