use serde_json::json;
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
async fn worker_binary_exits_cleanly_and_preserves_default_empty_state_when_persisted_state_has_no_review_agent_runs(
) {
    let path = unique_test_path("empty-persisted-state-idle-smoke");
    let store = FileOrganizationStore::new(&path);
    let initial_state = OrganizationState::default();
    store
        .store_organization_state(initial_state.clone())
        .await
        .unwrap();

    let worker_bin = std::env::var("CARGO_BIN_EXE_sourcebot-worker")
        .expect("cargo should expose the built sourcebot-worker binary path");
    let output = Command::new(worker_bin)
        .env("SOURCEBOT_ORGANIZATION_STATE_PATH", &path)
        .output()
        .expect("worker binary should run");

    assert!(
        output.status.success(),
        "worker should exit cleanly when persisted state contains no review-agent runs"
    );

    let persisted = store.organization_state().await.unwrap();
    assert_eq!(persisted, initial_state);

    fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn worker_binary_exits_cleanly_without_rewriting_existing_state_when_no_queued_review_agent_run_exists(
) {
    let path = unique_test_path("no-queued-review-agent-run-idle-smoke");
    let store = FileOrganizationStore::new(&path);
    let initial_state = OrganizationState {
        review_agent_runs: vec![
            review_agent_run(
                "run_claimed",
                ReviewAgentRunStatus::Claimed,
                "2026-04-25T00:10:05Z",
            ),
            review_agent_run(
                "run_completed",
                ReviewAgentRunStatus::Completed,
                "2026-04-25T00:10:06Z",
            ),
        ],
        ..OrganizationState::default()
    };
    let initial_bytes = serde_json::to_vec(&json!({
        "connections": [],
        "local_sessions": [],
        "review_agent_runs": [
            {
                "id": "run_claimed",
                "organization_id": "org_acme",
                "webhook_id": "webhook_run_claimed",
                "delivery_attempt_id": "delivery_run_claimed",
                "connection_id": "conn_github",
                "repository_id": "repo_sourcebot_rewrite",
                "review_id": "review_run_claimed",
                "status": "claimed",
                "created_at": "2026-04-25T00:10:05Z"
            },
            {
                "id": "run_completed",
                "organization_id": "org_acme",
                "webhook_id": "webhook_run_completed",
                "delivery_attempt_id": "delivery_run_completed",
                "connection_id": "conn_github",
                "repository_id": "repo_sourcebot_rewrite",
                "review_id": "review_run_completed",
                "status": "completed",
                "created_at": "2026-04-25T00:10:06Z"
            }
        ]
    }))
    .unwrap();
    fs::write(&path, &initial_bytes).unwrap();
    let initial_modified = fs::metadata(&path).unwrap().modified().unwrap();

    let worker_bin = std::env::var("CARGO_BIN_EXE_sourcebot-worker")
        .expect("cargo should expose the built sourcebot-worker binary path");
    let output = Command::new(worker_bin)
        .env("SOURCEBOT_ORGANIZATION_STATE_PATH", &path)
        .output()
        .expect("worker binary should run");

    assert!(
        output.status.success(),
        "worker should exit cleanly when no queued review-agent run exists"
    );

    let final_bytes = fs::read(&path).unwrap();
    let final_modified = fs::metadata(&path).unwrap().modified().unwrap();
    assert_eq!(
        final_bytes, initial_bytes,
        "worker idle path should not rewrite an existing organization-state file when no queued review-agent run exists"
    );
    assert_eq!(
        final_modified, initial_modified,
        "worker idle path should leave the existing organization-state file modification time unchanged"
    );

    let persisted = store.organization_state().await.unwrap();
    assert_eq!(persisted, initial_state);

    fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn worker_binary_exits_cleanly_and_leaves_default_empty_effective_state_when_organization_state_file_is_missing(
) {
    let path = unique_test_path("missing-organization-state-idle-smoke");
    let store = FileOrganizationStore::new(&path);
    let worker_bin = std::env::var("CARGO_BIN_EXE_sourcebot-worker")
        .expect("cargo should expose the built sourcebot-worker binary path");

    let output = Command::new(worker_bin)
        .env("SOURCEBOT_ORGANIZATION_STATE_PATH", &path)
        .output()
        .expect("worker binary should run");

    assert!(
        output.status.success(),
        "worker should exit cleanly when the organization-state file is missing"
    );
    assert!(
        !path.exists(),
        "worker should leave the missing organization-state file absent when it has no work to persist"
    );

    let persisted = store.organization_state().await.unwrap();
    assert_eq!(persisted, OrganizationState::default());
}
