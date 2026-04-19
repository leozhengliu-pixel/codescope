use anyhow::Result;
use sourcebot_core::OrganizationStore;
use sourcebot_models::{RepositorySyncJob, ReviewAgentRun};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub use sourcebot_core::claim_next_review_agent_run;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerTickOutcome {
    ReviewAgentRun(ReviewAgentRun),
    RepositorySyncJob(RepositorySyncJob),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StubReviewAgentRunExecutionOutcome {
    Completed,
    Failed,
}

pub async fn run_worker_tick(
    store: &dyn OrganizationStore,
    stub_outcome: StubReviewAgentRunExecutionOutcome,
) -> Result<Option<WorkerTickOutcome>> {
    if let Some(run) = run_review_agent_tick(store, stub_outcome).await? {
        return Ok(Some(WorkerTickOutcome::ReviewAgentRun(run)));
    }

    Ok(run_repository_sync_claim_tick(store)
        .await?
        .map(WorkerTickOutcome::RepositorySyncJob))
}

pub async fn run_review_agent_tick(
    store: &dyn OrganizationStore,
    stub_outcome: StubReviewAgentRunExecutionOutcome,
) -> Result<Option<ReviewAgentRun>> {
    let Some(claimed_run) = claim_next_review_agent_run_from_store(store).await? else {
        return Ok(None);
    };

    let stub_outcome = execute_claimed_review_agent_run_stub(&claimed_run, stub_outcome);
    persist_stub_review_agent_run_execution_outcome(store, &claimed_run.id, stub_outcome).await
}

pub async fn run_repository_sync_claim_tick(
    store: &dyn OrganizationStore,
) -> Result<Option<RepositorySyncJob>> {
    claim_next_repository_sync_job_from_store(store).await
}

pub fn execute_claimed_review_agent_run_stub(
    _run: &ReviewAgentRun,
    stub_outcome: StubReviewAgentRunExecutionOutcome,
) -> StubReviewAgentRunExecutionOutcome {
    stub_outcome
}

pub async fn claim_next_review_agent_run_from_store(
    store: &dyn OrganizationStore,
) -> Result<Option<ReviewAgentRun>> {
    store.claim_next_review_agent_run().await
}

pub async fn complete_review_agent_run_in_store(
    store: &dyn OrganizationStore,
    run_id: &str,
) -> Result<Option<ReviewAgentRun>> {
    store.complete_review_agent_run(run_id).await
}

pub async fn persist_stub_review_agent_run_execution_outcome(
    store: &dyn OrganizationStore,
    run_id: &str,
    outcome: StubReviewAgentRunExecutionOutcome,
) -> Result<Option<ReviewAgentRun>> {
    match outcome {
        StubReviewAgentRunExecutionOutcome::Completed => {
            complete_review_agent_run_in_store(store, run_id).await
        }
        StubReviewAgentRunExecutionOutcome::Failed => {
            fail_review_agent_run_in_store(store, run_id).await
        }
    }
}

pub async fn fail_review_agent_run_in_store(
    store: &dyn OrganizationStore,
    run_id: &str,
) -> Result<Option<ReviewAgentRun>> {
    store.fail_review_agent_run(run_id).await
}

pub async fn claim_next_repository_sync_job_from_store(
    store: &dyn OrganizationStore,
) -> Result<Option<RepositorySyncJob>> {
    claim_next_repository_sync_job_from_store_at(store, &current_timestamp()).await
}

pub async fn claim_next_repository_sync_job_from_store_at(
    store: &dyn OrganizationStore,
    started_at: &str,
) -> Result<Option<RepositorySyncJob>> {
    store.claim_next_repository_sync_job(started_at).await
}

fn current_timestamp() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("current UTC time should format as RFC3339")
}

#[cfg(test)]
mod tests {
    use super::{
        claim_next_repository_sync_job_from_store_at, claim_next_review_agent_run,
        claim_next_review_agent_run_from_store, execute_claimed_review_agent_run_stub,
        persist_stub_review_agent_run_execution_outcome, run_repository_sync_claim_tick,
        run_review_agent_tick, run_worker_tick, StubReviewAgentRunExecutionOutcome,
        WorkerTickOutcome,
    };
    use anyhow::Result;
    use async_trait::async_trait;
    use sourcebot_api::auth::FileOrganizationStore;
    use sourcebot_core::OrganizationStore;
    use sourcebot_models::{
        OrganizationState, RepositorySyncJob, RepositorySyncJobStatus, ReviewAgentRun,
        ReviewAgentRunStatus,
    };
    use std::{
        fs,
        sync::Mutex,
        time::{SystemTime, UNIX_EPOCH},
    };
    use time::{format_description::well_known::Rfc3339, OffsetDateTime};

    #[derive(Debug)]
    struct InMemoryOrganizationStore {
        state: Mutex<OrganizationState>,
    }

    impl InMemoryOrganizationStore {
        fn new(state: OrganizationState) -> Self {
            Self {
                state: Mutex::new(state),
            }
        }
    }

    #[async_trait]
    impl OrganizationStore for InMemoryOrganizationStore {
        async fn organization_state(&self) -> Result<OrganizationState> {
            Ok(self.state.lock().unwrap().clone())
        }

        async fn store_organization_state(&self, state: OrganizationState) -> Result<()> {
            *self.state.lock().unwrap() = state;
            Ok(())
        }

        async fn store_repository_sync_job(&self, job: RepositorySyncJob) -> Result<()> {
            let mut state = self.state.lock().unwrap();
            sourcebot_core::store_repository_sync_job(&mut state, job);
            Ok(())
        }

        async fn claim_next_repository_sync_job(
            &self,
            started_at: &str,
        ) -> Result<Option<RepositorySyncJob>> {
            let mut state = self.state.lock().unwrap();
            Ok(sourcebot_core::claim_next_repository_sync_job(
                &mut state, started_at,
            ))
        }

        async fn claim_next_review_agent_run(&self) -> Result<Option<ReviewAgentRun>> {
            let mut state = self.state.lock().unwrap();
            Ok(claim_next_review_agent_run(&mut state))
        }

        async fn complete_review_agent_run(&self, run_id: &str) -> Result<Option<ReviewAgentRun>> {
            let mut state = self.state.lock().unwrap();
            Ok(sourcebot_core::complete_review_agent_run(
                &mut state, run_id,
            ))
        }

        async fn fail_review_agent_run(&self, run_id: &str) -> Result<Option<ReviewAgentRun>> {
            let mut state = self.state.lock().unwrap();
            Ok(sourcebot_core::fail_review_agent_run(&mut state, run_id))
        }
    }

    fn unique_test_path(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("sourcebot-worker-{name}-{nanos}.json"))
    }

    fn review_agent_run(
        id: &str,
        status: ReviewAgentRunStatus,
        created_at: &str,
    ) -> ReviewAgentRun {
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
            repository_id: format!("repo_{id}"),
            connection_id: "conn_github".into(),
            status,
            queued_at: queued_at.into(),
            started_at: None,
            finished_at: None,
            error: None,
        }
    }

    #[test]
    fn claim_next_review_agent_run_claims_oldest_queued_run() {
        let mut state = OrganizationState {
            review_agent_runs: vec![
                review_agent_run(
                    "run_queued_newer",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:06Z",
                ),
                review_agent_run(
                    "run_claimed",
                    ReviewAgentRunStatus::Claimed,
                    "2026-04-25T00:10:04Z",
                ),
                review_agent_run(
                    "run_queued_oldest",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:05Z",
                ),
            ],
            ..OrganizationState::default()
        };

        let claimed_run =
            claim_next_review_agent_run(&mut state).expect("queued run to be claimed");

        assert_eq!(claimed_run.id, "run_queued_oldest");
        assert_eq!(claimed_run.status, ReviewAgentRunStatus::Claimed);
        assert_eq!(
            state.review_agent_runs[0].status,
            ReviewAgentRunStatus::Queued
        );
        assert_eq!(
            state.review_agent_runs[1].status,
            ReviewAgentRunStatus::Claimed
        );
        assert_eq!(
            state.review_agent_runs[2].status,
            ReviewAgentRunStatus::Claimed
        );
    }

    #[test]
    fn claim_next_review_agent_run_prefers_earlier_index_when_queued_timestamps_match() {
        let mut state = OrganizationState {
            review_agent_runs: vec![
                review_agent_run(
                    "run_first",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:05Z",
                ),
                review_agent_run(
                    "run_second",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:05Z",
                ),
            ],
            ..OrganizationState::default()
        };

        let claimed_run =
            claim_next_review_agent_run(&mut state).expect("queued run to be claimed");

        assert_eq!(claimed_run.id, "run_first");
        assert_eq!(
            state.review_agent_runs[0].status,
            ReviewAgentRunStatus::Claimed
        );
        assert_eq!(
            state.review_agent_runs[1].status,
            ReviewAgentRunStatus::Queued
        );
    }

    #[test]
    fn claim_next_review_agent_run_returns_none_when_no_queued_runs_exist() {
        let mut state = OrganizationState {
            review_agent_runs: vec![review_agent_run(
                "run_claimed",
                ReviewAgentRunStatus::Claimed,
                "2026-04-25T00:10:05Z",
            )],
            ..OrganizationState::default()
        };

        assert_eq!(claim_next_review_agent_run(&mut state), None);
    }

    #[test]
    fn execute_claimed_review_agent_run_stub_returns_the_requested_outcome() {
        let claimed_run = review_agent_run(
            "run_claimed",
            ReviewAgentRunStatus::Claimed,
            "2026-04-25T00:10:05Z",
        );

        let stub_outcome = execute_claimed_review_agent_run_stub(
            &claimed_run,
            StubReviewAgentRunExecutionOutcome::Failed,
        );

        assert_eq!(stub_outcome, StubReviewAgentRunExecutionOutcome::Failed);
    }

    #[tokio::test]
    async fn run_review_agent_tick_records_a_completed_run_in_the_file_store_after_stub_execution()
    {
        let path = unique_test_path("worker-tick-file-store");
        let store = FileOrganizationStore::new(&path);
        store
            .store_organization_state(OrganizationState {
                review_agent_runs: vec![
                    review_agent_run(
                        "run_queued_newer",
                        ReviewAgentRunStatus::Queued,
                        "2026-04-25T00:10:06Z",
                    ),
                    review_agent_run(
                        "run_queued_oldest",
                        ReviewAgentRunStatus::Queued,
                        "2026-04-25T00:10:05Z",
                    ),
                ],
                ..OrganizationState::default()
            })
            .await
            .unwrap();

        let completed_run =
            run_review_agent_tick(&store, StubReviewAgentRunExecutionOutcome::Completed)
                .await
                .unwrap()
                .expect("queued run to be completed");

        assert_eq!(completed_run.id, "run_queued_oldest");
        assert_eq!(completed_run.status, ReviewAgentRunStatus::Completed);

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(
            persisted.review_agent_runs[0].status,
            ReviewAgentRunStatus::Queued
        );
        assert_eq!(
            persisted.review_agent_runs[1].status,
            ReviewAgentRunStatus::Completed
        );

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn claim_next_review_agent_run_from_store_persists_the_claimed_run() {
        let store = InMemoryOrganizationStore::new(OrganizationState {
            review_agent_runs: vec![
                review_agent_run(
                    "run_queued_newer",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:06Z",
                ),
                review_agent_run(
                    "run_queued_oldest",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:05Z",
                ),
            ],
            ..OrganizationState::default()
        });

        let claimed_run = claim_next_review_agent_run_from_store(&store)
            .await
            .unwrap()
            .expect("queued run to be claimed");

        assert_eq!(claimed_run.id, "run_queued_oldest");
        assert_eq!(claimed_run.status, ReviewAgentRunStatus::Claimed);

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(
            persisted.review_agent_runs[0].status,
            ReviewAgentRunStatus::Queued
        );
        assert_eq!(
            persisted.review_agent_runs[1].status,
            ReviewAgentRunStatus::Claimed
        );
    }

    #[tokio::test]
    async fn run_review_agent_tick_records_a_failed_run_in_the_file_store_after_stub_execution() {
        let path = unique_test_path("worker-tick-file-store-failed");
        let store = FileOrganizationStore::new(&path);
        store
            .store_organization_state(OrganizationState {
                review_agent_runs: vec![
                    review_agent_run(
                        "run_queued_newer",
                        ReviewAgentRunStatus::Queued,
                        "2026-04-25T00:10:06Z",
                    ),
                    review_agent_run(
                        "run_queued_oldest",
                        ReviewAgentRunStatus::Queued,
                        "2026-04-25T00:10:05Z",
                    ),
                ],
                ..OrganizationState::default()
            })
            .await
            .unwrap();

        let failed_run = run_review_agent_tick(&store, StubReviewAgentRunExecutionOutcome::Failed)
            .await
            .unwrap()
            .expect("queued run to be failed");

        assert_eq!(failed_run.id, "run_queued_oldest");
        assert_eq!(failed_run.status, ReviewAgentRunStatus::Failed);

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(
            persisted.review_agent_runs[0].status,
            ReviewAgentRunStatus::Queued
        );
        assert_eq!(
            persisted.review_agent_runs[1].status,
            ReviewAgentRunStatus::Failed
        );

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn persist_stub_review_agent_run_execution_outcome_records_a_failed_run_when_requested() {
        let store = InMemoryOrganizationStore::new(OrganizationState {
            review_agent_runs: vec![
                review_agent_run(
                    "run_queued_newer",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:06Z",
                ),
                review_agent_run(
                    "run_queued_oldest",
                    ReviewAgentRunStatus::Queued,
                    "2026-04-25T00:10:05Z",
                ),
            ],
            ..OrganizationState::default()
        });

        let claimed_run = claim_next_review_agent_run_from_store(&store)
            .await
            .unwrap()
            .expect("queued run to be claimed");
        assert_eq!(claimed_run.id, "run_queued_oldest");

        let failed_run = persist_stub_review_agent_run_execution_outcome(
            &store,
            &claimed_run.id,
            StubReviewAgentRunExecutionOutcome::Failed,
        )
        .await
        .unwrap()
        .expect("claimed run to be failed");

        assert_eq!(failed_run.id, "run_queued_oldest");
        assert_eq!(failed_run.status, ReviewAgentRunStatus::Failed);

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(
            persisted.review_agent_runs[0].status,
            ReviewAgentRunStatus::Queued
        );
        assert_eq!(
            persisted.review_agent_runs[1].status,
            ReviewAgentRunStatus::Failed
        );
    }

    #[tokio::test]
    async fn run_worker_tick_prioritizes_review_agent_work_before_repository_sync_claims() {
        let store = InMemoryOrganizationStore::new(OrganizationState {
            review_agent_runs: vec![review_agent_run(
                "run_queued_oldest",
                ReviewAgentRunStatus::Queued,
                "2026-04-25T00:10:05Z",
            )],
            repository_sync_jobs: vec![repository_sync_job(
                "sync_job_oldest",
                RepositorySyncJobStatus::Queued,
                "2026-04-26T10:01:00Z",
            )],
            ..OrganizationState::default()
        });

        let outcome = run_worker_tick(&store, StubReviewAgentRunExecutionOutcome::Completed)
            .await
            .unwrap();

        assert_eq!(
            outcome,
            Some(WorkerTickOutcome::ReviewAgentRun(review_agent_run(
                "run_queued_oldest",
                ReviewAgentRunStatus::Completed,
                "2026-04-25T00:10:05Z",
            )))
        );

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(
            persisted.review_agent_runs[0].status,
            ReviewAgentRunStatus::Completed
        );
        assert_eq!(
            persisted.repository_sync_jobs[0].status,
            RepositorySyncJobStatus::Queued
        );
        assert_eq!(persisted.repository_sync_jobs[0].started_at, None);
    }

    #[tokio::test]
    async fn claim_next_repository_sync_job_from_store_persists_the_oldest_running_job() {
        let store = InMemoryOrganizationStore::new(OrganizationState {
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
        });

        let claimed_job =
            claim_next_repository_sync_job_from_store_at(&store, "2026-04-26T10:03:00Z")
                .await
                .unwrap()
                .expect("queued repository sync job to be claimed");

        assert_eq!(claimed_job.id, "sync_job_oldest");
        assert_eq!(claimed_job.status, RepositorySyncJobStatus::Running);
        assert_eq!(
            claimed_job.started_at.as_deref(),
            Some("2026-04-26T10:03:00Z")
        );

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(
            persisted.repository_sync_jobs[0].status,
            RepositorySyncJobStatus::Queued
        );
        assert_eq!(persisted.repository_sync_jobs[1], claimed_job);
    }

    #[tokio::test]
    async fn run_repository_sync_claim_tick_records_a_running_repository_sync_job_in_the_file_store(
    ) {
        let path = unique_test_path("worker-tick-file-store-repository-sync-job");
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

        let running_job = run_repository_sync_claim_tick(&store)
            .await
            .unwrap()
            .expect("queued repository sync job to be claimed");

        assert_eq!(running_job.id, "sync_job_oldest");
        assert_eq!(running_job.status, RepositorySyncJobStatus::Running);
        let started_at = running_job
            .started_at
            .as_deref()
            .expect("started_at to be set");
        assert!(OffsetDateTime::parse(started_at, &Rfc3339).is_ok());
        assert_eq!(running_job.finished_at, None);
        assert_eq!(running_job.error, None);

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(
            persisted.repository_sync_jobs[0].status,
            RepositorySyncJobStatus::Queued
        );
        assert_eq!(persisted.repository_sync_jobs[1], running_job);

        fs::remove_file(path).unwrap();
    }
}
