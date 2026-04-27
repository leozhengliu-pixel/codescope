use sourcebot_api::auth::build_organization_store;
use sourcebot_config::{
    AppConfig, StubRepositorySyncJobExecutionOutcomeConfig,
    StubReviewAgentRunExecutionOutcomeConfig,
};
use sourcebot_worker::{
    run_worker_tick, StubRepositorySyncJobExecutionOutcome, StubReviewAgentRunExecutionOutcome,
    WorkerTickOutcome,
};
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

    info!(
        organization_state_path = %config.organization_state_path,
        review_agent_stub_outcome = ?review_agent_stub_outcome,
        repository_sync_stub_outcome = ?repository_sync_stub_outcome,
        "worker runtime baseline resolved for explicit one-shot runtime"
    );

    let outcome = run_worker_tick(
        store.as_ref(),
        review_agent_stub_outcome,
        repository_sync_stub_outcome,
    )
    .await?;

    match outcome {
        Some(WorkerTickOutcome::ReviewAgentRun(run)) => info!(
            review_agent_run_id = %run.id,
            status = ?run.status,
            "recorded review-agent run terminal status after stub worker execution"
        ),
        Some(WorkerTickOutcome::RepositorySyncJob(job)) => info!(
            repository_sync_job_id = %job.id,
            organization_id = %job.organization_id,
            repository_id = %job.repository_id,
            connection_id = %job.connection_id,
            status = ?job.status,
            error = job.error.as_deref().unwrap_or(""),
            "recorded repository-sync terminal status after stub worker execution for the one-shot runtime baseline"
        ),
        None => info!("no queued review-agent run or repository sync job available"),
    }

    Ok(())
}
