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
        .compact()
        .init();

    let config = AppConfig::from_env();
    let store = build_organization_store(config.organization_state_path.clone());
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
            status = ?job.status,
            "recorded repository sync job stub terminal status after one worker tick"
        ),
        None => info!("no queued review-agent run or repository sync job available"),
    }

    Ok(())
}
