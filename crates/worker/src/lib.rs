use anyhow::Result;
use sourcebot_core::OrganizationStore;
use sourcebot_models::ReviewAgentRun;

pub use sourcebot_core::claim_next_review_agent_run;

pub async fn run_worker_tick(store: &dyn OrganizationStore) -> Result<Option<ReviewAgentRun>> {
    let Some(claimed_run) = claim_next_review_agent_run_from_store(store).await? else {
        return Ok(None);
    };

    let stub_run = execute_claimed_review_agent_run_stub(claimed_run);
    fail_review_agent_run_in_store(store, &stub_run.id).await
}

pub fn execute_claimed_review_agent_run_stub(run: ReviewAgentRun) -> ReviewAgentRun {
    run
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

pub async fn fail_review_agent_run_in_store(
    store: &dyn OrganizationStore,
    run_id: &str,
) -> Result<Option<ReviewAgentRun>> {
    store.fail_review_agent_run(run_id).await
}

#[cfg(test)]
mod tests {
    use super::{
        claim_next_review_agent_run, claim_next_review_agent_run_from_store,
        execute_claimed_review_agent_run_stub, run_worker_tick,
    };
    use anyhow::Result;
    use async_trait::async_trait;
    use sourcebot_api::auth::FileOrganizationStore;
    use sourcebot_core::OrganizationStore;
    use sourcebot_models::{OrganizationState, ReviewAgentRun, ReviewAgentRunStatus};
    use std::{
        fs,
        sync::Mutex,
        time::{SystemTime, UNIX_EPOCH},
    };

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
    fn execute_claimed_review_agent_run_stub_preserves_the_claimed_run_as_a_no_op_boundary() {
        let claimed_run = review_agent_run(
            "run_claimed",
            ReviewAgentRunStatus::Claimed,
            "2026-04-25T00:10:05Z",
        );

        let stub_run = execute_claimed_review_agent_run_stub(claimed_run.clone());

        assert_eq!(stub_run, claimed_run);
        assert_eq!(stub_run.status, ReviewAgentRunStatus::Claimed);
    }

    #[tokio::test]
    async fn run_worker_tick_records_a_failed_run_in_the_file_store_after_stub_execution() {
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

        let failed_run = run_worker_tick(&store)
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
    async fn run_worker_tick_fails_the_claimed_run_after_stub_execution() {
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

        let failed_run = run_worker_tick(&store)
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
    }
}
