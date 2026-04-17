use sourcebot_models::{OrganizationState, ReviewAgentRun, ReviewAgentRunStatus};

pub fn claim_next_review_agent_run(state: &mut OrganizationState) -> Option<ReviewAgentRun> {
    let next_run_index = state
        .review_agent_runs
        .iter()
        .enumerate()
        .filter(|(_, run)| run.status == ReviewAgentRunStatus::Queued)
        .min_by(|(left_index, left_run), (right_index, right_run)| {
            left_run
                .created_at
                .cmp(&right_run.created_at)
                .then_with(|| left_index.cmp(right_index))
        })
        .map(|(index, _)| index)?;

    let run = state.review_agent_runs.get_mut(next_run_index)?;
    run.status = ReviewAgentRunStatus::Claimed;
    Some(run.clone())
}

#[cfg(test)]
mod tests {
    use super::claim_next_review_agent_run;
    use sourcebot_models::{OrganizationState, ReviewAgentRun, ReviewAgentRunStatus};

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
}
