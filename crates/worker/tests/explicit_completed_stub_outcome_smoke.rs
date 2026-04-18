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

fn run_worker_binary(path: &std::path::Path) {
    let worker_bin = std::env::var("CARGO_BIN_EXE_sourcebot-worker")
        .expect("cargo should expose the built sourcebot-worker binary path");
    let output = Command::new(worker_bin)
        .env("SOURCEBOT_ORGANIZATION_STATE_PATH", path)
        .env(
            "SOURCEBOT_STUB_REVIEW_AGENT_RUN_EXECUTION_OUTCOME",
            "completed",
        )
        .output()
        .expect("worker binary should run");

    assert!(
        output.status.success(),
        "worker should exit cleanly after processing one queued run"
    );
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

#[tokio::test]
async fn worker_binary_processes_only_the_oldest_queued_run_per_invocation() {
    let path = unique_test_path("single-oldest-queued-run-smoke");
    let store = FileOrganizationStore::new(&path);
    store
        .store_organization_state(OrganizationState {
            review_agent_runs: vec![
                review_agent_run(
                    "run_queued_newest",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:07Z",
                ),
                review_agent_run(
                    "run_queued_oldest",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:05Z",
                ),
                review_agent_run(
                    "run_queued_middle",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:06Z",
                ),
            ],
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
        "worker should exit cleanly after processing one queued run"
    );

    let persisted = store.organization_state().await.unwrap();
    assert_eq!(persisted.review_agent_runs.len(), 3);
    assert_eq!(persisted.review_agent_runs[0].id, "run_queued_newest");
    assert_eq!(
        persisted.review_agent_runs[0].status,
        ReviewAgentRunStatus::Queued
    );
    assert_eq!(persisted.review_agent_runs[1].id, "run_queued_oldest");
    assert_eq!(
        persisted.review_agent_runs[1].status,
        ReviewAgentRunStatus::Completed
    );
    assert_eq!(persisted.review_agent_runs[2].id, "run_queued_middle");
    assert_eq!(
        persisted.review_agent_runs[2].status,
        ReviewAgentRunStatus::Queued
    );

    assert_eq!(
        persisted
            .review_agent_runs
            .iter()
            .filter(|run| run.status == ReviewAgentRunStatus::Completed)
            .count(),
        1,
        "one worker invocation should record exactly one completed run"
    );
    assert_eq!(
        persisted
            .review_agent_runs
            .iter()
            .filter(|run| run.status == ReviewAgentRunStatus::Queued)
            .count(),
        2,
        "one worker invocation should leave later queued runs untouched"
    );

    fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn worker_binary_advances_to_the_next_oldest_queued_run_on_a_second_invocation() {
    let path = unique_test_path("second-invocation-next-oldest-smoke");
    let store = FileOrganizationStore::new(&path);
    store
        .store_organization_state(OrganizationState {
            review_agent_runs: vec![
                review_agent_run(
                    "run_queued_newest",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:07Z",
                ),
                review_agent_run(
                    "run_queued_oldest",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:05Z",
                ),
                review_agent_run(
                    "run_queued_middle",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:06Z",
                ),
            ],
            ..OrganizationState::default()
        })
        .await
        .unwrap();

    run_worker_binary(&path);

    let after_first_invocation = store.organization_state().await.unwrap();
    assert_eq!(
        after_first_invocation.review_agent_runs[0].status,
        ReviewAgentRunStatus::Queued
    );
    assert_eq!(
        after_first_invocation.review_agent_runs[1].status,
        ReviewAgentRunStatus::Completed
    );
    assert_eq!(
        after_first_invocation.review_agent_runs[2].status,
        ReviewAgentRunStatus::Queued
    );

    run_worker_binary(&path);

    let after_second_invocation = store.organization_state().await.unwrap();
    assert_eq!(after_second_invocation.review_agent_runs.len(), 3);
    assert_eq!(
        after_second_invocation.review_agent_runs[0].id,
        "run_queued_newest"
    );
    assert_eq!(
        after_second_invocation.review_agent_runs[0].status,
        ReviewAgentRunStatus::Queued
    );
    assert_eq!(
        after_second_invocation.review_agent_runs[1].id,
        "run_queued_oldest"
    );
    assert_eq!(
        after_second_invocation.review_agent_runs[1].status,
        ReviewAgentRunStatus::Completed
    );
    assert_eq!(
        after_second_invocation.review_agent_runs[2].id,
        "run_queued_middle"
    );
    assert_eq!(
        after_second_invocation.review_agent_runs[2].status,
        ReviewAgentRunStatus::Completed
    );

    assert_eq!(
        after_second_invocation
            .review_agent_runs
            .iter()
            .filter(|run| run.status == ReviewAgentRunStatus::Completed)
            .count(),
        2,
        "two worker invocations should record two completed runs"
    );
    assert_eq!(
        after_second_invocation
            .review_agent_runs
            .iter()
            .filter(|run| run.status == ReviewAgentRunStatus::Queued)
            .count(),
        1,
        "the second worker invocation should advance to the next-oldest remaining queued run"
    );

    fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn worker_binary_advances_the_final_remaining_queued_run_on_a_third_invocation() {
    let path = unique_test_path("third-invocation-final-queued-run-smoke");
    let store = FileOrganizationStore::new(&path);
    store
        .store_organization_state(OrganizationState {
            review_agent_runs: vec![
                review_agent_run(
                    "run_queued_newest",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:07Z",
                ),
                review_agent_run(
                    "run_queued_oldest",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:05Z",
                ),
                review_agent_run(
                    "run_queued_middle",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:06Z",
                ),
            ],
            ..OrganizationState::default()
        })
        .await
        .unwrap();

    run_worker_binary(&path);
    run_worker_binary(&path);

    let after_second_invocation = store.organization_state().await.unwrap();
    assert_eq!(
        after_second_invocation.review_agent_runs[0].status,
        ReviewAgentRunStatus::Queued
    );
    assert_eq!(
        after_second_invocation.review_agent_runs[1].status,
        ReviewAgentRunStatus::Completed
    );
    assert_eq!(
        after_second_invocation.review_agent_runs[2].status,
        ReviewAgentRunStatus::Completed
    );

    run_worker_binary(&path);

    let after_third_invocation = store.organization_state().await.unwrap();
    assert_eq!(after_third_invocation.review_agent_runs.len(), 3);
    assert_eq!(
        after_third_invocation.review_agent_runs[0].id,
        "run_queued_newest"
    );
    assert_eq!(
        after_third_invocation.review_agent_runs[0].status,
        ReviewAgentRunStatus::Completed
    );
    assert_eq!(
        after_third_invocation.review_agent_runs[1].id,
        "run_queued_oldest"
    );
    assert_eq!(
        after_third_invocation.review_agent_runs[1].status,
        ReviewAgentRunStatus::Completed
    );
    assert_eq!(
        after_third_invocation.review_agent_runs[2].id,
        "run_queued_middle"
    );
    assert_eq!(
        after_third_invocation.review_agent_runs[2].status,
        ReviewAgentRunStatus::Completed
    );

    assert_eq!(
        after_third_invocation
            .review_agent_runs
            .iter()
            .filter(|run| run.status == ReviewAgentRunStatus::Completed)
            .count(),
        3,
        "three worker invocations should record three completed runs"
    );
    assert_eq!(
        after_third_invocation
            .review_agent_runs
            .iter()
            .filter(|run| run.status == ReviewAgentRunStatus::Queued)
            .count(),
        0,
        "the third worker invocation should advance the final remaining queued run"
    );

    fs::remove_file(path).unwrap();
}
