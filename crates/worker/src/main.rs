use sourcebot_api::auth::build_organization_store;
use sourcebot_config::{AppConfig, StubReviewAgentRunExecutionOutcomeConfig};
use sourcebot_worker::{run_worker_tick, StubReviewAgentRunExecutionOutcome};
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    let config = AppConfig::from_env();
    let store = build_organization_store(config.organization_state_path.clone());
    let stub_outcome = match config.stub_review_agent_run_execution_outcome()? {
        StubReviewAgentRunExecutionOutcomeConfig::Completed => {
            StubReviewAgentRunExecutionOutcome::Completed
        }
        StubReviewAgentRunExecutionOutcomeConfig::Failed => {
            StubReviewAgentRunExecutionOutcome::Failed
        }
    };
    let terminal_run = run_worker_tick(store.as_ref(), stub_outcome).await?;

    match terminal_run {
        Some(run) => info!(
            review_agent_run_id = %run.id,
            status = ?run.status,
            "recorded review-agent run terminal status after stub worker execution"
        ),
        None => info!("no queued review-agent run available"),
    }

    Ok(())
}
