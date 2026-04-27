use sourcebot_api::auth::FileOrganizationStore;
use sourcebot_core::OrganizationStore;
use sourcebot_models::{OrganizationState, RepositorySyncJob, RepositorySyncJobStatus};
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
    }
}

#[tokio::test]
async fn worker_binary_claims_oldest_queued_repository_sync_job_and_persists_stub_completed_status_when_no_review_agent_run_is_available(
) {
    let path = unique_test_path("claim-oldest-repository-sync-job-smoke");
    let store = FileOrganizationStore::new(&path);
    store
        .store_organization_state(OrganizationState {
            repository_sync_jobs: vec![
                repository_sync_job(
                    "sync_job_newer",
                    RepositorySyncJobStatus::Queued,
                    "2026-04-26T10:02:00Z",
                ),
                repository_sync_job(
                    "sync_job_oldest",
                    RepositorySyncJobStatus::Queued,
                    "2026-04-26T10:01:00Z",
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
        .output()
        .expect("worker binary should run");

    assert!(
        output.status.success(),
        "worker should exit cleanly after claiming one queued repository sync job"
    );

    let persisted = store.organization_state().await.unwrap();
    assert_eq!(persisted.repository_sync_jobs.len(), 2);
    assert_eq!(persisted.repository_sync_jobs[0].id, "sync_job_newer");
    assert_eq!(
        persisted.repository_sync_jobs[0].status,
        RepositorySyncJobStatus::Queued
    );
    assert_eq!(persisted.repository_sync_jobs[0].started_at, None);
    assert_eq!(persisted.repository_sync_jobs[1].id, "sync_job_oldest");
    assert_eq!(
        persisted.repository_sync_jobs[1].status,
        RepositorySyncJobStatus::Succeeded
    );
    assert!(persisted.repository_sync_jobs[1].started_at.is_some());
    assert!(persisted.repository_sync_jobs[1].finished_at.is_some());
    assert_eq!(persisted.repository_sync_jobs[1].error, None);

    fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn worker_binary_persists_stub_failed_repository_sync_status_when_explicitly_configured() {
    let path = unique_test_path("explicit-failed-repository-sync-outcome-smoke");
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
        .env(
            "SOURCEBOT_STUB_REPOSITORY_SYNC_JOB_EXECUTION_OUTCOME",
            "failed",
        )
        .output()
        .expect("worker binary should run");

    assert!(
        output.status.success(),
        "worker should accept explicit failed repository sync stub outcomes"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    for expected in [
        "sync_job_oldest",
        "org_acme",
        "repo_sourcebot_rewrite",
        "conn_github",
        "Failed",
        "repository sync stub execution configured to fail",
    ] {
        assert!(
            stderr.contains(expected),
            "worker log should include {expected:?} for operator triage: {stderr}"
        );
    }

    let persisted = store.organization_state().await.unwrap();
    assert_eq!(persisted.repository_sync_jobs.len(), 1);
    assert_eq!(persisted.repository_sync_jobs[0].id, "sync_job_oldest");
    assert_eq!(
        persisted.repository_sync_jobs[0].status,
        RepositorySyncJobStatus::Failed
    );
    assert!(persisted.repository_sync_jobs[0].started_at.is_some());
    assert!(persisted.repository_sync_jobs[0].finished_at.is_some());
    assert_eq!(
        persisted.repository_sync_jobs[0].error.as_deref(),
        Some("repository sync stub execution configured to fail")
    );

    fs::remove_file(path).unwrap();
}
