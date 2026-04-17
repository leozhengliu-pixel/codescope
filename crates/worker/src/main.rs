use sourcebot_api::auth::build_organization_store;
use sourcebot_config::AppConfig;
use sourcebot_worker::run_worker_tick;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    let config = AppConfig::from_env();
    let store = build_organization_store(config.organization_state_path.clone());
    let claimed_run = run_worker_tick(store.as_ref()).await?;

    match claimed_run {
        Some(run) => {
            info!(review_agent_run_id = %run.id, status = ?run.status, "claimed review-agent run")
        }
        None => info!("no queued review-agent run available"),
    }

    Ok(())
}
