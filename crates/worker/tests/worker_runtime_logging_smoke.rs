use sourcebot_api::auth::FileOrganizationStore;
use sourcebot_core::OrganizationStore;
use sourcebot_models::{
    Connection, ConnectionConfig, ConnectionKind, OrganizationState, RepositorySyncJob,
    RepositorySyncJobStatus, ReviewAgentRun, ReviewAgentRunStatus,
};
use std::{
    fs,
    path::Path,
    process::Command,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
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

fn local_connection(repo_path: &Path) -> Connection {
    Connection {
        id: "conn_local".into(),
        name: "Local fixture".into(),
        kind: ConnectionKind::Local,
        config: Some(ConnectionConfig::Local {
            repo_path: repo_path.display().to_string(),
        }),
    }
}

fn initialize_local_git_fixture(name: &str) -> std::path::PathBuf {
    let repo_path = unique_test_path(name).with_extension("repo");
    fs::create_dir_all(&repo_path).expect("local Git fixture directory should be created");
    let init_output = Command::new("git")
        .arg("init")
        .arg("--initial-branch=main")
        .arg(&repo_path)
        .output()
        .expect("git init should run");
    assert!(
        init_output.status.success(),
        "git init should succeed: stdout={} stderr={}",
        String::from_utf8_lossy(&init_output.stdout),
        String::from_utf8_lossy(&init_output.stderr)
    );
    let run_git = |args: &[&str]| {
        let output = Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .args(args)
            .output()
            .expect("git fixture command should run");
        assert!(
            output.status.success(),
            "git fixture command {:?} should succeed: stdout={} stderr={}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    };
    run_git(&["config", "user.email", "worker@example.invalid"]);
    run_git(&["config", "user.name", "Worker Test"]);
    fs::write(repo_path.join("README.md"), "tracked content\n")
        .expect("tracked fixture file should be written");
    run_git(&["add", "README.md"]);
    run_git(&["commit", "-m", "initial"]);
    repo_path
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
    assert!(
        status["process_id"]
            .as_u64()
            .is_some_and(|process_id| process_id > 0),
        "supervisor status should expose the worker process id: {status}"
    );
    let updated_at = status["updated_at"]
        .as_str()
        .expect("supervisor status should expose an operator heartbeat timestamp");
    time::OffsetDateTime::parse(updated_at, &time::format_description::well_known::Rfc3339)
        .expect("operator heartbeat timestamp should be RFC3339");

    fs::remove_file(path).unwrap();
    fs::remove_file(status_path).unwrap();
}

#[tokio::test]
async fn worker_binary_refreshes_status_snapshot_before_idle_sleep() {
    let path = unique_test_path("worker-runtime-status-before-idle-organizations");
    let status_path = unique_test_path("worker-runtime-status-before-idle-snapshot");
    let store = FileOrganizationStore::new(&path);
    store
        .store_organization_state(OrganizationState {
            ..OrganizationState::default()
        })
        .await
        .unwrap();

    let worker_bin = std::env::var("CARGO_BIN_EXE_sourcebot-worker")
        .expect("cargo should expose the built sourcebot-worker binary path");
    let mut child = Command::new(worker_bin)
        .env("SOURCEBOT_ORGANIZATION_STATE_PATH", &path)
        .env("SOURCEBOT_WORKER_STATUS_PATH", &status_path)
        .env("SOURCEBOT_WORKER_MAX_TICKS", "2")
        .env("SOURCEBOT_WORKER_IDLE_SLEEP_MS", "1000")
        .spawn()
        .expect("worker binary should start");

    let mut observed_tick_heartbeat = None;
    for _ in 0..20 {
        if let Ok(bytes) = fs::read(&status_path) {
            if let Ok(status) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                if status["ticks_completed"] == 1 && status["completed"] == false {
                    observed_tick_heartbeat = Some(status);
                    break;
                }
            }
        }
        thread::sleep(Duration::from_millis(25));
    }

    child.kill().ok();
    let _ = child.wait();

    let status = observed_tick_heartbeat
        .expect("worker should refresh tick-1 heartbeat before the configured idle sleep elapses");
    assert_eq!(status["last_outcome"], "no_work");
    assert!(status["updated_at"].is_string());
    assert!(status["process_id"].as_u64().is_some_and(|pid| pid > 0));

    fs::remove_file(path).unwrap();
    let _ = fs::remove_file(status_path);
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

#[tokio::test]
async fn worker_binary_status_snapshot_includes_failed_repository_sync_error_detail() {
    let path = unique_test_path("worker-runtime-status-repository-sync-failed-state");
    let status_path = unique_test_path("worker-runtime-status-repository-sync-failed-snapshot");
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

    let worker_bin = std::env::var("CARGO_BIN_EXE_sourcebot-worker")
        .expect("cargo should expose the built sourcebot-worker binary path");
    let output = Command::new(worker_bin)
        .env("SOURCEBOT_ORGANIZATION_STATE_PATH", &path)
        .env("SOURCEBOT_WORKER_STATUS_PATH", &status_path)
        .env(
            "SOURCEBOT_STUB_REPOSITORY_SYNC_JOB_EXECUTION_OUTCOME",
            "failed",
        )
        .output()
        .expect("worker binary should run");
    assert!(
        output.status.success(),
        "worker should exit cleanly after writing failed repository-sync status: stderr={}",
        normalized_log_output(&output.stderr)
    );

    let status: serde_json::Value = serde_json::from_slice(
        &fs::read(&status_path).expect("worker should write supervisor status snapshot"),
    )
    .expect("supervisor status should be JSON");
    assert_eq!(status["last_outcome"], "repository_sync_job");
    assert_eq!(status["last_work_item_id"], "sync_job_oldest");
    assert_eq!(status["last_work_item_status"], "Failed");
    assert_eq!(
        status["last_work_item_error"],
        "repository sync stub execution configured to fail"
    );

    fs::remove_file(path).unwrap();
    fs::remove_file(status_path).unwrap();
}

#[tokio::test]
async fn worker_binary_status_snapshot_includes_successful_repository_sync_metadata() {
    let path = unique_test_path("worker-runtime-status-local-sync-state");
    let status_path = unique_test_path("worker-runtime-status-local-sync-snapshot");
    let repo_path = initialize_local_git_fixture("worker-runtime-status-local-sync-repo");
    let store = FileOrganizationStore::new(&path);
    let mut job = repository_sync_job(
        "sync_job_local",
        RepositorySyncJobStatus::Queued,
        "2026-04-26T10:01:00Z",
    );
    job.connection_id = "conn_local".into();
    store
        .store_organization_state(OrganizationState {
            connections: vec![local_connection(&repo_path)],
            repository_sync_jobs: vec![job],
            ..OrganizationState::default()
        })
        .await
        .unwrap();

    let output = run_worker_with_status_path(&path, &status_path);
    assert!(
        output.status.success(),
        "worker should exit cleanly after writing local-sync status metadata: stderr={}",
        normalized_log_output(&output.stderr)
    );

    let persisted = store.organization_state().await.unwrap();
    let completed = persisted
        .repository_sync_jobs
        .iter()
        .find(|job| job.id == "sync_job_local")
        .expect("local sync job should persist");
    assert_eq!(completed.status, RepositorySyncJobStatus::Succeeded);

    let status: serde_json::Value = serde_json::from_slice(
        &fs::read(&status_path).expect("worker should write supervisor status snapshot"),
    )
    .expect("supervisor status should be JSON");
    assert_eq!(status["last_outcome"], "repository_sync_job");
    assert_eq!(status["last_work_item_id"], "sync_job_local");
    assert_eq!(status["last_work_item_status"], "Succeeded");
    assert_eq!(
        status["last_work_item_synced_revision"],
        completed.synced_revision.as_deref().unwrap()
    );
    assert_eq!(
        status["last_work_item_synced_branch"],
        completed.synced_branch.as_deref().unwrap()
    );
    assert_eq!(
        status["last_work_item_synced_content_file_count"],
        completed.synced_content_file_count.unwrap()
    );

    fs::remove_file(path).unwrap();
    fs::remove_file(status_path).unwrap();
    fs::remove_dir_all(repo_path).unwrap();
}
