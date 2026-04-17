use sourcebot_api::auth::FileOrganizationStore;
use sourcebot_core::OrganizationStore;
use sourcebot_models::{OrganizationState, ReviewAgentRun, ReviewAgentRunStatus};
use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn unique_test_path(name: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("sourcebot-worker-{name}-{nanos}.json"))
}

fn review_agent_run(id: &str, status: ReviewAgentRunStatus, created_at: &str) -> ReviewAgentRun {
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

#[tokio::test]
async fn worker_binary_accepts_explicit_completed_stub_outcome_and_persists_completed_status() {
    let path = unique_test_path("explicit-completed-stub-outcome-smoke");
    let store = FileOrganizationStore::new(&path);
    store
        .store_organization_state(OrganizationState {
            review_agent_runs: vec![review_agent_run(
                "run_queued_oldest",
                ReviewAgentRunStatus::Queued,
                "2026-04-25T00:10:05Z",
            )],
            ..OrganizationState::default()
        })
        .await
        .unwrap();

    let worker_bin = std::env::var("CARGO_BIN_EXE_sourcebot-worker")
        .expect("cargo should expose the built sourcebot-worker binary path");
    let output = Command::new(worker_bin)
        .env("SOURCEBOT_ORGANIZATION_STATE_PATH", &path)
        .env(
            "SOURCEBOT_STUB_REVIEW_AGENT_RUN_EXECUTION_OUTCOME",
            "completed",
        )
        .output()
        .expect("worker binary should run");

    assert!(
        output.status.success(),
        "worker should accept explicit completed"
    );

    let persisted = store.organization_state().await.unwrap();
    assert_eq!(
        persisted.review_agent_runs[0].status,
        ReviewAgentRunStatus::Completed
    );

    fs::remove_file(path).unwrap();
}
