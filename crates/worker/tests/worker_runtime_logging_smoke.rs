use sourcebot_api::auth::FileOrganizationStore;
use sourcebot_core::OrganizationStore;
use sourcebot_models::{
    OrganizationState, RepositorySyncJob, RepositorySyncJobStatus, ReviewAgentRun,
    ReviewAgentRunStatus,
};
use std::{
    fs,
    path::Path,
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

fn repository_sync_job(
    id: &str,
    status: RepositorySyncJobStatus,
    queued_at: &str,
) -> RepositorySyncJob {
    RepositorySyncJob {
        id: id.into(),
        organization_id: "org_acme".into(),
        repository_id: "repo_sourcebot_rewrite".into(),
        connection_id: "conn_github".into(),
        status,
        queued_at: queued_at.into(),
        started_at: None,
        finished_at: None,
        error: None,
        synced_revision: None,
        synced_branch: None,
        synced_content_file_count: None,
    }
}

fn run_worker(path: &Path) -> std::process::Output {
    let worker_bin = std::env::var("CARGO_BIN_EXE_sourcebot-worker")
        .expect("cargo should expose the built sourcebot-worker binary path");
    Command::new(worker_bin)
        .env("SOURCEBOT_ORGANIZATION_STATE_PATH", path)
        .output()
        .expect("worker binary should run")
}

fn run_worker_with_status_path(path: &Path, status_path: &Path) -> std::process::Output {
    let worker_bin = std::env::var("CARGO_BIN_EXE_sourcebot-worker")
        .expect("cargo should expose the built sourcebot-worker binary path");
    Command::new(worker_bin)
        .env("SOURCEBOT_ORGANIZATION_STATE_PATH", path)
        .env("SOURCEBOT_WORKER_STATUS_PATH", status_path)
        .output()
        .expect("worker binary should run")
}

fn run_worker_with_failed_repository_sync(path: &Path) -> std::process::Output {
    let worker_bin = std::env::var("CARGO_BIN_EXE_sourcebot-worker")
        .expect("cargo should expose the built sourcebot-worker binary path");
    Command::new(worker_bin)
        .env("SOURCEBOT_ORGANIZATION_STATE_PATH", path)
        .env(
            "SOURCEBOT_STUB_REPOSITORY_SYNC_JOB_EXECUTION_OUTCOME",
            "failed",
        )
        .output()
        .expect("worker binary should run")
}

fn normalized_log_output(bytes: &[u8]) -> String {
    let raw = String::from_utf8_lossy(bytes);
    let mut normalized = String::new();
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if matches!(chars.peek(), Some('[')) {
                chars.next();
                for next in chars.by_ref() {
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
                continue;
            }
        }

        if ch != '\r' {
            normalized.push(ch);
        }
    }

    normalized
}

#[tokio::test]
async fn worker_binary_logs_runtime_baseline_and_no_work_path() {
    let path = unique_test_path("worker-runtime-logging-no-work-smoke");
    let store = FileOrganizationStore::new(&path);
    store
        .store_organization_state(OrganizationState {
            review_agent_runs: vec![review_agent_run(
                "run_completed",
                ReviewAgentRunStatus::Completed,
                "2026-04-26T10:01:00Z",
            )],
            ..OrganizationState::default()
        })
        .await
        .unwrap();

    let output = run_worker(&path);
    assert!(
        output.status.success(),
        "worker should exit cleanly for no-work runtime logging"
    );

    let stderr = normalized_log_output(&output.stderr);
    assert!(
        stderr.contains("worker runtime baseline"),
        "stderr should include a startup runtime baseline log line: {stderr}"
    );
    assert!(
        stderr.contains("organization_state_path="),
        "stderr should include the organization-state path field name: {stderr}"
    );
    assert!(
        stderr.contains(path.file_name().unwrap().to_str().unwrap()),
        "stderr should include the resolved organization-state path value: {stderr}"
    );
    assert!(
        stderr.contains("review_agent_stub_outcome") && stderr.contains("Completed"),
        "stderr should include the resolved review-agent stub outcome: {stderr}"
    );
    assert!(
        stderr.contains("repository_sync_stub_outcome") && stderr.contains("Succeeded"),
        "stderr should include the resolved repository-sync stub outcome: {stderr}"
    );
    assert!(
        stderr.contains("max_ticks=1") && stderr.contains("idle_sleep_ms=1000"),
        "stderr should include the configured default bounded runtime controls: {stderr}"
    );
    assert!(
        stderr.contains("no queued review-agent run or repository sync job available"),
        "stderr should still log the no-work path: {stderr}"
    );

    fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn worker_binary_writes_supervisor_status_snapshot_for_no_work_tick() {
    let path = unique_test_path("worker-runtime-status-organizations");
    let status_path = unique_test_path("worker-runtime-status-snapshot");
    let store = FileOrganizationStore::new(&path);
    store
        .store_organization_state(OrganizationState {
            ..OrganizationState::default()
        })
        .await
        .unwrap();

    let output = run_worker_with_status_path(&path, &status_path);
    assert!(
        output.status.success(),
        "worker should exit cleanly after writing supervisor status: stderr={}",
        normalized_log_output(&output.stderr)
    );

    let status: serde_json::Value = serde_json::from_slice(
        &fs::read(&status_path).expect("worker should write supervisor status snapshot"),
    )
    .expect("supervisor status should be JSON");
    assert_eq!(status["schema_version"], 1);
    assert_eq!(status["completed"], true);
    assert_eq!(status["max_ticks"], 1);
    assert_eq!(status["ticks_completed"], 1);
    assert_eq!(status["last_tick"], 1);
    assert_eq!(status["last_outcome"], "no_work");
    assert_eq!(status["last_work_item_id"], serde_json::Value::Null);
    assert_eq!(status["last_work_item_status"], serde_json::Value::Null);

    fs::remove_file(path).unwrap();
    fs::remove_file(status_path).unwrap();
}

#[tokio::test]
async fn worker_binary_logs_failed_repository_sync_terminal_status() {
    let path = unique_test_path("worker-runtime-logging-repository-sync-failed-smoke");
    let store = FileOrganizationStore::new(&path);
    store
        .store_organization_state(OrganizationState {
            repository_sync_jobs: vec![repository_sync_job(
                "sync_job_oldest",
                RepositorySyncJobStatus::Queued,
                "2026-04-26T10:01:00Z",
            )],
            ..OrganizationState::default()
        })
        .await
        .unwrap();

    let output = run_worker_with_failed_repository_sync(&path);
    assert!(
        output.status.success(),
        "worker should exit cleanly after logging failed repository-sync terminal status"
    );

    let stderr = normalized_log_output(&output.stderr);
    assert!(
        stderr.contains("worker runtime baseline"),
        "stderr should include the startup runtime baseline log line: {stderr}"
    );
    assert!(
        stderr.contains("repository_sync_stub_outcome") && stderr.contains("Failed"),
        "stderr should include the configured failed repository-sync stub outcome: {stderr}"
    );
    assert!(
        stderr.contains("repository-sync terminal status"),
        "stderr should include a repository-sync terminal-status line: {stderr}"
    );
    assert!(
        stderr.contains("status=Failed"),
        "stderr should report the failed repository-sync terminal status: {stderr}"
    );

    let persisted = store.organization_state().await.unwrap();
    assert_eq!(persisted.repository_sync_jobs.len(), 1);
    assert_eq!(
        persisted.repository_sync_jobs[0].status,
        RepositorySyncJobStatus::Failed
    );

    fs::remove_file(path).unwrap();
}
