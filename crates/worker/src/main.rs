use sourcebot_api::auth::build_organization_store;
use sourcebot_config::{
    AppConfig, StubRepositorySyncJobExecutionOutcomeConfig,
    StubReviewAgentRunExecutionOutcomeConfig,
};
use sourcebot_worker::{
    run_worker_tick, StubRepositorySyncJobExecutionOutcome, StubReviewAgentRunExecutionOutcome,
    WorkerTickOutcome,
};
use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Duration,
};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stderr)
        .compact()
        .init();

    let config = AppConfig::from_env();
    let store = build_organization_store(
        config.organization_state_path.clone(),
        config.database_url.as_deref(),
    );
    let review_agent_stub_outcome = match config.stub_review_agent_run_execution_outcome()? {
        StubReviewAgentRunExecutionOutcomeConfig::Completed => {
            StubReviewAgentRunExecutionOutcome::Completed
        }
        StubReviewAgentRunExecutionOutcomeConfig::Failed => {
            StubReviewAgentRunExecutionOutcome::Failed
        }
    };
    let repository_sync_stub_outcome = match config.stub_repository_sync_job_execution_outcome()? {
        StubRepositorySyncJobExecutionOutcomeConfig::Succeeded => {
            StubRepositorySyncJobExecutionOutcome::Succeeded
        }
        StubRepositorySyncJobExecutionOutcomeConfig::Failed => {
            StubRepositorySyncJobExecutionOutcome::Failed
        }
    };

    let max_ticks = worker_max_ticks_from_env()?;
    let idle_sleep = worker_idle_sleep_from_env()?;
    let status_path = worker_status_path_from_env()?;

    info!(
        organization_state_path = %config.organization_state_path,
        review_agent_stub_outcome = ?review_agent_stub_outcome,
        repository_sync_stub_outcome = ?repository_sync_stub_outcome,
        max_ticks,
        idle_sleep_ms = idle_sleep.as_millis(),
        worker_status_path = status_path.as_ref().map(|path| path.display().to_string()).as_deref().unwrap_or(""),
        "worker runtime baseline resolved"
    );

    let mut last_tick = 0;
    let mut last_outcome = "not_started".to_string();
    let mut last_work_item_id: Option<String> = None;
    let mut last_work_item_status: Option<String> = None;
    let mut last_work_item_error: Option<String> = None;
    let mut last_work_item_synced_revision: Option<String> = None;
    let mut last_work_item_synced_branch: Option<String> = None;
    let mut last_work_item_synced_content_file_count: Option<i64> = None;

    if let Some(path) = status_path.as_ref() {
        write_worker_status_snapshot(
            path,
            max_ticks,
            0,
            0,
            &last_outcome,
            None,
            None,
            None,
            None,
            None,
            None,
            false,
        )?;
    }

    for tick in 1..=max_ticks {
        let outcome = run_worker_tick(
            store.as_ref(),
            review_agent_stub_outcome,
            repository_sync_stub_outcome,
        )
        .await?;
        last_tick = tick;

        let no_work = outcome.is_none();
        match outcome {
            Some(WorkerTickOutcome::ReviewAgentRun(run)) => {
                last_outcome = "review_agent_run".to_string();
                last_work_item_id = Some(run.id.clone());
                last_work_item_status = Some(format!("{:?}", run.status));
                last_work_item_error = None;
                last_work_item_synced_revision = None;
                last_work_item_synced_branch = None;
                last_work_item_synced_content_file_count = None;
                info!(
                    tick,
                    review_agent_run_id = %run.id,
                    status = ?run.status,
                    "recorded review-agent run terminal status after worker execution"
                )
            }
            Some(WorkerTickOutcome::RepositorySyncJob(job)) => {
                last_outcome = "repository_sync_job".to_string();
                last_work_item_id = Some(job.id.clone());
                last_work_item_status = Some(format!("{:?}", job.status));
                last_work_item_error = job.error.as_deref().map(sanitize_worker_status_error);
                last_work_item_synced_revision = job.synced_revision.clone();
                last_work_item_synced_branch = job.synced_branch.clone();
                last_work_item_synced_content_file_count = job.synced_content_file_count;
                info!(
                    tick,
                    repository_sync_job_id = %job.id,
                    organization_id = %job.organization_id,
                    repository_id = %job.repository_id,
                    connection_id = %job.connection_id,
                    status = ?job.status,
                    synced_revision = job.synced_revision.as_deref().unwrap_or(""),
                    synced_branch = job.synced_branch.as_deref().unwrap_or(""),
                    synced_content_file_count = job.synced_content_file_count,
                    error = job.error.as_deref().unwrap_or(""),
                    "recorded repository-sync terminal status after worker execution"
                )
            }
            None => {
                last_outcome = "no_work".to_string();
                last_work_item_id = None;
                last_work_item_status = None;
                last_work_item_error = None;
                last_work_item_synced_revision = None;
                last_work_item_synced_branch = None;
                last_work_item_synced_content_file_count = None;
                info!(
                    tick,
                    "no queued review-agent run or repository sync job available"
                );
            }
        }

        if let Some(path) = status_path.as_ref() {
            write_worker_status_snapshot(
                path,
                max_ticks,
                tick,
                last_tick,
                &last_outcome,
                last_work_item_id.as_deref(),
                last_work_item_status.as_deref(),
                last_work_item_error.as_deref(),
                last_work_item_synced_revision.as_deref(),
                last_work_item_synced_branch.as_deref(),
                last_work_item_synced_content_file_count,
                false,
            )?;
        }

        if no_work && tick < max_ticks && !idle_sleep.is_zero() {
            tokio::time::sleep(idle_sleep).await;
        }
    }

    if let Some(path) = status_path.as_ref() {
        write_worker_status_snapshot(
            path,
            max_ticks,
            max_ticks,
            last_tick,
            &last_outcome,
            last_work_item_id.as_deref(),
            last_work_item_status.as_deref(),
            last_work_item_error.as_deref(),
            last_work_item_synced_revision.as_deref(),
            last_work_item_synced_branch.as_deref(),
            last_work_item_synced_content_file_count,
            true,
        )?;
    }

    info!(max_ticks, "completed configured bounded worker runtime");

    Ok(())
}

const WORKER_MAX_TICKS_CAP: u64 = 1_000_000;
const WORKER_IDLE_SLEEP_MS_CAP: u64 = 60 * 60 * 1000;

fn worker_max_ticks_from_env() -> anyhow::Result<u64> {
    let value = parse_positive_u64_env("SOURCEBOT_WORKER_MAX_TICKS", 1)?;
    if value > WORKER_MAX_TICKS_CAP {
        anyhow::bail!(
            "SOURCEBOT_WORKER_MAX_TICKS must be less than or equal to {WORKER_MAX_TICKS_CAP}"
        );
    }
    Ok(value)
}

fn worker_status_path_from_env() -> anyhow::Result<Option<PathBuf>> {
    match env::var("SOURCEBOT_WORKER_STATUS_PATH") {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                anyhow::bail!("SOURCEBOT_WORKER_STATUS_PATH must not be empty when set");
            }
            Ok(Some(PathBuf::from(trimmed)))
        }
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => {
            anyhow::bail!("SOURCEBOT_WORKER_STATUS_PATH must be valid unicode")
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn write_worker_status_snapshot(
    path: &Path,
    max_ticks: u64,
    ticks_completed: u64,
    last_tick: u64,
    last_outcome: &str,
    last_work_item_id: Option<&str>,
    last_work_item_status: Option<&str>,
    last_work_item_error: Option<&str>,
    last_work_item_synced_revision: Option<&str>,
    last_work_item_synced_branch: Option<&str>,
    last_work_item_synced_content_file_count: Option<i64>,
    completed: bool,
) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let payload = serde_json::json!({
        "schema_version": 1,
        "completed": completed,
        "updated_at": OffsetDateTime::now_utc().format(&Rfc3339)?,
        "process_id": std::process::id(),
        "max_ticks": max_ticks,
        "ticks_completed": ticks_completed,
        "last_tick": last_tick,
        "last_outcome": last_outcome,
        "last_work_item_id": last_work_item_id,
        "last_work_item_status": last_work_item_status,
        "last_work_item_error": last_work_item_error,
        "last_work_item_synced_revision": last_work_item_synced_revision,
        "last_work_item_synced_branch": last_work_item_synced_branch,
        "last_work_item_synced_content_file_count": last_work_item_synced_content_file_count,
    });
    fs::write(path, serde_json::to_vec_pretty(&payload)?)?;
    Ok(())
}

fn sanitize_worker_status_error(error: &str) -> String {
    let trimmed = error.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    const STUB_FAILURE: &str = "repository sync stub execution configured to fail";
    const KNOWN_PREFIXES: [&str; 5] = [
        "local repository sync preflight failed",
        "local repository sync execution failed",
        "generic Git repository sync execution failed",
        "repository sync job exceeded worker lease and was marked failed before the next claim",
        "repository sync job had malformed running lease timestamp and was marked failed before the next claim",
    ];

    if trimmed == STUB_FAILURE {
        return STUB_FAILURE.to_string();
    }

    for prefix in KNOWN_PREFIXES {
        if trimmed == prefix {
            return format!("{prefix}: details redacted");
        }
        if let Some(suffix) = trimmed.strip_prefix(prefix) {
            if suffix.starts_with(':') || suffix.starts_with(char::is_whitespace) {
                return format!("{prefix}: details redacted");
            }
        }
    }

    "repository sync failed: details redacted".to_string()
}

fn worker_idle_sleep_from_env() -> anyhow::Result<Duration> {
    let value = parse_u64_env("SOURCEBOT_WORKER_IDLE_SLEEP_MS", 1000)?;
    if value > WORKER_IDLE_SLEEP_MS_CAP {
        anyhow::bail!(
            "SOURCEBOT_WORKER_IDLE_SLEEP_MS must be less than or equal to {WORKER_IDLE_SLEEP_MS_CAP}"
        );
    }
    Ok(Duration::from_millis(value))
}

#[cfg(test)]
mod tests {
    use super::{
        sanitize_worker_status_error, worker_idle_sleep_from_env, worker_max_ticks_from_env,
    };
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn worker_max_ticks_fails_closed_when_configured_above_runtime_cap() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("SOURCEBOT_WORKER_MAX_TICKS", "1000001");
        let error = worker_max_ticks_from_env()
            .expect_err("oversized max tick configuration must fail closed");
        std::env::remove_var("SOURCEBOT_WORKER_MAX_TICKS");

        assert_eq!(
            error.to_string(),
            "SOURCEBOT_WORKER_MAX_TICKS must be less than or equal to 1000000"
        );
    }

    #[test]
    fn worker_idle_sleep_fails_closed_when_configured_above_operator_cap() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("SOURCEBOT_WORKER_IDLE_SLEEP_MS", "3600001");
        let error = worker_idle_sleep_from_env()
            .expect_err("oversized idle sleep configuration must fail closed");
        std::env::remove_var("SOURCEBOT_WORKER_IDLE_SLEEP_MS");

        assert_eq!(
            error.to_string(),
            "SOURCEBOT_WORKER_IDLE_SLEEP_MS must be less than or equal to 3600000"
        );
    }

    #[test]
    fn worker_status_error_redacts_known_prefix_details() {
        let sanitized = sanitize_worker_status_error(
            "local repository sync preflight failed: git -C /secret/repo rev-parse failed: token=abc123",
        );

        assert_eq!(
            sanitized,
            "local repository sync preflight failed: details redacted"
        );
        assert!(!sanitized.contains("/secret/repo"));
        assert!(!sanitized.contains("token=abc123"));
    }

    #[test]
    fn worker_status_error_redacts_unknown_prefixes_and_colonless_details() {
        let url_prefix = sanitize_worker_status_error(
            "https://token@example.com/repo.git: generic Git repository sync execution failed",
        );
        assert_eq!(url_prefix, "repository sync failed: details redacted");
        assert!(!url_prefix.contains("token@example.com"));

        let colonless = sanitize_worker_status_error("token abc123 leaked without stable prefix");
        assert_eq!(colonless, "repository sync failed: details redacted");
        assert!(!colonless.contains("abc123"));
    }

    #[test]
    fn worker_status_error_preserves_stub_failure_summary() {
        assert_eq!(
            sanitize_worker_status_error("repository sync stub execution configured to fail"),
            "repository sync stub execution configured to fail"
        );
    }

    #[test]
    fn worker_status_snapshot_includes_repository_sync_terminal_metadata_fields() {
        let path = std::env::temp_dir().join(format!(
            "sourcebot-worker-status-metadata-{}.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        super::write_worker_status_snapshot(
            &path,
            3,
            2,
            2,
            "repository_sync_job",
            Some("sync_job_1"),
            Some("Succeeded"),
            None,
            Some("abc123"),
            Some("main"),
            Some(7),
            false,
        )
        .expect("status snapshot should be written");

        let payload: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&path).expect("status snapshot should be readable"),
        )
        .expect("status snapshot should be JSON");
        let _ = std::fs::remove_file(&path);

        assert_eq!(payload["last_work_item_synced_revision"], "abc123");
        assert_eq!(payload["last_work_item_synced_branch"], "main");
        assert_eq!(payload["last_work_item_synced_content_file_count"], 7);
    }

    #[test]
    fn worker_status_error_preserves_lease_recovery_prefixes_without_details() {
        let stale = sanitize_worker_status_error(
            "repository sync job exceeded worker lease and was marked failed before the next claim at 2026-04-29T14:00:00Z",
        );
        assert_eq!(
            stale,
            "repository sync job exceeded worker lease and was marked failed before the next claim: details redacted"
        );
        assert!(!stale.contains("2026-04-29"));

        let malformed = sanitize_worker_status_error(
            "repository sync job had malformed running lease timestamp and was marked failed before the next claim: invalid started_at at 2026-04-29T14:00:00Z",
        );
        assert_eq!(
            malformed,
            "repository sync job had malformed running lease timestamp and was marked failed before the next claim: details redacted"
        );
        assert!(!malformed.contains("invalid started_at"));
    }
}

fn parse_positive_u64_env(name: &str, default: u64) -> anyhow::Result<u64> {
    let value = parse_u64_env(name, default)?;
    if value == 0 {
        anyhow::bail!("{name} must be greater than zero");
    }
    Ok(value)
}

fn parse_u64_env(name: &str, default: u64) -> anyhow::Result<u64> {
    match env::var(name) {
        Ok(value) => value
            .parse::<u64>()
            .map_err(|_| anyhow::anyhow!("{name} must be an unsigned integer")),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(env::VarError::NotUnicode(_)) => anyhow::bail!("{name} must be valid unicode"),
    }
}
