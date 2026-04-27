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
async fn worker_binary_rejects_invalid_stub_outcome_without_mutating_state() {
    let path = unique_test_path("invalid-stub-outcome-smoke");
    let store = store_with_queued_review_run(&path).await;

    let output = worker_command(&path)
        .env("SOURCEBOT_STUB_REVIEW_AGENT_RUN_EXECUTION_OUTCOME", "bogus")
        .output()
        .expect("worker binary should run");

    assert!(!output.status.success(), "worker should fail closed");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("SOURCEBOT_STUB_REVIEW_AGENT_RUN_EXECUTION_OUTCOME"));
    assert!(stderr.contains("bogus"));

    assert_review_run_still_queued(&store).await;

    fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn worker_binary_rejects_zero_tick_limit_without_mutating_state() {
    let path = unique_test_path("zero-tick-limit-smoke");
    let store = store_with_queued_review_run(&path).await;

    let output = worker_command(&path)
        .env("SOURCEBOT_WORKER_MAX_TICKS", "0")
        .output()
        .expect("worker binary should run");

    assert!(!output.status.success(), "worker should fail closed");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("SOURCEBOT_WORKER_MAX_TICKS"));
    assert!(stderr.contains("greater than zero"));

    assert_review_run_still_queued(&store).await;

    fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn worker_binary_rejects_invalid_idle_sleep_without_mutating_state() {
    let path = unique_test_path("invalid-idle-sleep-smoke");
    let store = store_with_queued_review_run(&path).await;

    let output = worker_command(&path)
        .env("SOURCEBOT_WORKER_IDLE_SLEEP_MS", "-1")
        .output()
        .expect("worker binary should run");

    assert!(!output.status.success(), "worker should fail closed");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("SOURCEBOT_WORKER_IDLE_SLEEP_MS"));
    assert!(stderr.contains("unsigned integer"));

    assert_review_run_still_queued(&store).await;

    fs::remove_file(path).unwrap();
}

async fn store_with_queued_review_run(path: &std::path::Path) -> FileOrganizationStore {
    let store = FileOrganizationStore::new(path);
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
    store
}

fn worker_command(path: &std::path::Path) -> Command {
    let worker_bin = std::env::var("CARGO_BIN_EXE_sourcebot-worker")
        .expect("cargo should expose the built sourcebot-worker binary path");
    let mut command = Command::new(worker_bin);
    command.env("SOURCEBOT_ORGANIZATION_STATE_PATH", path);
    command
}

async fn assert_review_run_still_queued(store: &FileOrganizationStore) {
    let persisted = store.organization_state().await.unwrap();
    assert_eq!(
        persisted.review_agent_runs[0].status,
        ReviewAgentRunStatus::Queued
    );
}
