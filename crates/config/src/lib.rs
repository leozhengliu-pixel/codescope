use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{env, path::PathBuf};

fn configured_data_dir() -> Option<String> {
    env::var("SOURCEBOT_DATA_DIR").ok().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn runtime_state_path(
    explicit_var: &str,
    data_dir: Option<&str>,
    default_path: &str,
    file_name: &str,
) -> String {
    env::var(explicit_var).unwrap_or_else(|_| match data_dir {
        Some(data_dir) => PathBuf::from(data_dir)
            .join(file_name)
            .display()
            .to_string(),
        None => default_path.to_string(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StubReviewAgentRunExecutionOutcomeConfig {
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StubRepositorySyncJobExecutionOutcomeConfig {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub service_name: String,
    pub bind_addr: String,
    pub database_url: Option<String>,
    pub bootstrap_state_path: String,
    pub local_session_state_path: String,
    pub organization_state_path: String,
    pub worker_status_path: Option<String>,
    pub llm_provider: Option<String>,
    pub llm_model: Option<String>,
    pub llm_api_base: Option<String>,
    pub llm_api_key: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            service_name: "sourcebot-api".to_string(),
            bind_addr: "127.0.0.1:3000".to_string(),
            database_url: None,
            bootstrap_state_path: ".sourcebot/bootstrap-state.json".to_string(),
            local_session_state_path: ".sourcebot/local-sessions.json".to_string(),
            organization_state_path: ".sourcebot/organizations.json".to_string(),
            worker_status_path: None,
            llm_provider: Some("disabled".to_string()),
            llm_model: None,
            llm_api_base: None,
            llm_api_key: None,
        }
    }
}

impl AppConfig {
    pub fn from_env() -> Self {
        let data_dir = configured_data_dir();

        Self {
            service_name: env::var("SOURCEBOT_SERVICE_NAME")
                .unwrap_or_else(|_| "sourcebot-api".to_string()),
            bind_addr: env::var("SOURCEBOT_BIND_ADDR")
                .unwrap_or_else(|_| "127.0.0.1:3000".to_string()),
            database_url: env::var("DATABASE_URL").ok(),
            bootstrap_state_path: runtime_state_path(
                "SOURCEBOT_BOOTSTRAP_STATE_PATH",
                data_dir.as_deref(),
                ".sourcebot/bootstrap-state.json",
                "bootstrap-state.json",
            ),
            local_session_state_path: runtime_state_path(
                "SOURCEBOT_LOCAL_SESSION_STATE_PATH",
                data_dir.as_deref(),
                ".sourcebot/local-sessions.json",
                "local-sessions.json",
            ),
            organization_state_path: runtime_state_path(
                "SOURCEBOT_ORGANIZATION_STATE_PATH",
                data_dir.as_deref(),
                ".sourcebot/organizations.json",
                "organizations.json",
            ),
            worker_status_path: env::var("SOURCEBOT_WORKER_STATUS_PATH")
                .ok()
                .and_then(|value| {
                    let trimmed = value.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                }),
            llm_provider: env::var("SOURCEBOT_LLM_PROVIDER")
                .ok()
                .or_else(|| Some("disabled".to_string())),
            llm_model: env::var("SOURCEBOT_LLM_MODEL").ok(),
            llm_api_base: env::var("SOURCEBOT_LLM_API_BASE").ok(),
            llm_api_key: env::var("SOURCEBOT_LLM_API_KEY").ok(),
        }
    }

    pub fn stub_review_agent_run_execution_outcome(
        &self,
    ) -> Result<StubReviewAgentRunExecutionOutcomeConfig> {
        match env::var("SOURCEBOT_STUB_REVIEW_AGENT_RUN_EXECUTION_OUTCOME") {
            Ok(value) if value.eq_ignore_ascii_case("completed") => {
                Ok(StubReviewAgentRunExecutionOutcomeConfig::Completed)
            }
            Ok(value) if value.eq_ignore_ascii_case("failed") => {
                Ok(StubReviewAgentRunExecutionOutcomeConfig::Failed)
            }
            Ok(value) => Err(anyhow!(
                "unsupported SOURCEBOT_STUB_REVIEW_AGENT_RUN_EXECUTION_OUTCOME value: {value}"
            )),
            Err(env::VarError::NotPresent) => {
                Ok(StubReviewAgentRunExecutionOutcomeConfig::Completed)
            }
            Err(env::VarError::NotUnicode(_)) => Err(anyhow!(
                "SOURCEBOT_STUB_REVIEW_AGENT_RUN_EXECUTION_OUTCOME must be valid unicode"
            )),
        }
    }

    pub fn stub_repository_sync_job_execution_outcome(
        &self,
    ) -> Result<StubRepositorySyncJobExecutionOutcomeConfig> {
        match env::var("SOURCEBOT_STUB_REPOSITORY_SYNC_JOB_EXECUTION_OUTCOME") {
            Ok(value) if value.eq_ignore_ascii_case("succeeded") => {
                Ok(StubRepositorySyncJobExecutionOutcomeConfig::Succeeded)
            }
            Ok(value) if value.eq_ignore_ascii_case("failed") => {
                Ok(StubRepositorySyncJobExecutionOutcomeConfig::Failed)
            }
            Ok(value) => Err(anyhow!(
                "unsupported SOURCEBOT_STUB_REPOSITORY_SYNC_JOB_EXECUTION_OUTCOME value: {value}"
            )),
            Err(env::VarError::NotPresent) => {
                Ok(StubRepositorySyncJobExecutionOutcomeConfig::Succeeded)
            }
            Err(env::VarError::NotUnicode(_)) => Err(anyhow!(
                "SOURCEBOT_STUB_REPOSITORY_SYNC_JOB_EXECUTION_OUTCOME must be valid unicode"
            )),
        }
    }

    pub fn public_view(&self) -> PublicAppConfig {
        PublicAppConfig {
            service_name: self.service_name.clone(),
            bind_addr: self.bind_addr.clone(),
            has_database_url: self.database_url.is_some(),
            llm_provider: self.llm_provider.clone(),
            llm_model: self.llm_model.clone(),
            has_llm_api_key: self.llm_api_key.is_some(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicAppConfig {
    pub service_name: String,
    pub bind_addr: String,
    pub has_database_url: bool,
    pub llm_provider: Option<String>,
    pub llm_model: Option<String>,
    pub has_llm_api_key: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, sync::Mutex};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn default_config_disables_llm_provider() {
        let config = AppConfig::default();

        assert_eq!(config.llm_provider.as_deref(), Some("disabled"));
        assert_eq!(
            config.bootstrap_state_path,
            ".sourcebot/bootstrap-state.json"
        );
        assert_eq!(
            config.local_session_state_path,
            ".sourcebot/local-sessions.json"
        );
        assert_eq!(
            config.organization_state_path,
            ".sourcebot/organizations.json"
        );
        assert_eq!(config.llm_model, None);
        assert_eq!(config.llm_api_base, None);
        assert_eq!(config.llm_api_key, None);
    }

    #[test]
    fn from_env_reads_worker_status_path_and_treats_blank_as_unset() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        env::set_var(
            "SOURCEBOT_WORKER_STATUS_PATH",
            " /tmp/sourcebot-worker-status.json ",
        );

        let config = AppConfig::from_env();

        assert_eq!(
            config.worker_status_path.as_deref(),
            Some("/tmp/sourcebot-worker-status.json")
        );

        env::set_var("SOURCEBOT_WORKER_STATUS_PATH", "   ");
        let blank_config = AppConfig::from_env();
        assert_eq!(blank_config.worker_status_path, None);

        env::remove_var("SOURCEBOT_WORKER_STATUS_PATH");
    }

    #[test]
    fn public_view_hides_llm_api_key_value() {
        let config = AppConfig {
            service_name: "sourcebot-api".into(),
            bind_addr: "127.0.0.1:3000".into(),
            database_url: None,
            bootstrap_state_path: ".sourcebot/bootstrap-state.json".into(),
            local_session_state_path: ".sourcebot/local-sessions.json".into(),
            organization_state_path: ".sourcebot/organizations.json".into(),
            worker_status_path: Some("/tmp/worker-status.json".into()),
            llm_provider: Some("stub".into()),
            llm_model: Some("stub-model".into()),
            llm_api_base: Some("https://llm.invalid".into()),
            llm_api_key: Some("super-secret".into()),
        };

        let public = config.public_view();
        assert_eq!(public.llm_provider.as_deref(), Some("stub"));
        assert_eq!(public.llm_model.as_deref(), Some("stub-model"));
        assert!(public.has_llm_api_key);
    }

    #[test]
    fn from_env_data_dir_derives_runtime_state_paths() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        env::set_var("SOURCEBOT_DATA_DIR", "/tmp/sourcebot-runtime");
        env::remove_var("SOURCEBOT_BOOTSTRAP_STATE_PATH");
        env::remove_var("SOURCEBOT_LOCAL_SESSION_STATE_PATH");
        env::remove_var("SOURCEBOT_ORGANIZATION_STATE_PATH");
        env::remove_var("SOURCEBOT_WORKER_STATUS_PATH");

        let config = AppConfig::from_env();

        assert_eq!(
            config.bootstrap_state_path,
            "/tmp/sourcebot-runtime/bootstrap-state.json"
        );
        assert_eq!(
            config.local_session_state_path,
            "/tmp/sourcebot-runtime/local-sessions.json"
        );
        assert_eq!(
            config.organization_state_path,
            "/tmp/sourcebot-runtime/organizations.json"
        );

        env::remove_var("SOURCEBOT_DATA_DIR");
    }

    #[test]
    fn from_env_blank_data_dir_falls_back_to_default_runtime_state_paths() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        env::set_var("SOURCEBOT_DATA_DIR", "   ");
        env::remove_var("SOURCEBOT_BOOTSTRAP_STATE_PATH");
        env::remove_var("SOURCEBOT_LOCAL_SESSION_STATE_PATH");
        env::remove_var("SOURCEBOT_ORGANIZATION_STATE_PATH");
        env::remove_var("SOURCEBOT_WORKER_STATUS_PATH");

        let config = AppConfig::from_env();

        assert_eq!(
            config.bootstrap_state_path,
            ".sourcebot/bootstrap-state.json"
        );
        assert_eq!(
            config.local_session_state_path,
            ".sourcebot/local-sessions.json"
        );
        assert_eq!(
            config.organization_state_path,
            ".sourcebot/organizations.json"
        );

        env::remove_var("SOURCEBOT_DATA_DIR");
    }

    #[test]
    fn from_env_explicit_runtime_state_paths_override_data_dir() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        env::set_var("SOURCEBOT_DATA_DIR", "/tmp/sourcebot-runtime");
        env::set_var(
            "SOURCEBOT_BOOTSTRAP_STATE_PATH",
            "/var/lib/sourcebot/custom-bootstrap.json",
        );
        env::set_var(
            "SOURCEBOT_LOCAL_SESSION_STATE_PATH",
            "/var/lib/sourcebot/custom-local-sessions.json",
        );
        env::set_var(
            "SOURCEBOT_ORGANIZATION_STATE_PATH",
            "/var/lib/sourcebot/custom-organizations.json",
        );
        env::remove_var("SOURCEBOT_WORKER_STATUS_PATH");

        let config = AppConfig::from_env();

        assert_eq!(
            config.bootstrap_state_path,
            "/var/lib/sourcebot/custom-bootstrap.json"
        );
        assert_eq!(
            config.local_session_state_path,
            "/var/lib/sourcebot/custom-local-sessions.json"
        );
        assert_eq!(
            config.organization_state_path,
            "/var/lib/sourcebot/custom-organizations.json"
        );

        env::remove_var("SOURCEBOT_DATA_DIR");
        env::remove_var("SOURCEBOT_BOOTSTRAP_STATE_PATH");
        env::remove_var("SOURCEBOT_LOCAL_SESSION_STATE_PATH");
        env::remove_var("SOURCEBOT_ORGANIZATION_STATE_PATH");
    }

    #[test]
    fn stub_review_agent_run_execution_outcome_defaults_to_completed() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::remove_var("SOURCEBOT_STUB_REVIEW_AGENT_RUN_EXECUTION_OUTCOME");

        let config = AppConfig::from_env();

        assert_eq!(
            config
                .stub_review_agent_run_execution_outcome()
                .expect("missing env var should default to completed"),
            StubReviewAgentRunExecutionOutcomeConfig::Completed
        );
    }

    #[test]
    fn stub_review_agent_run_execution_outcome_reads_completed_from_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::set_var(
            "SOURCEBOT_STUB_REVIEW_AGENT_RUN_EXECUTION_OUTCOME",
            "completed",
        );

        let config = AppConfig::from_env();

        assert_eq!(
            config
                .stub_review_agent_run_execution_outcome()
                .expect("completed should be accepted"),
            StubReviewAgentRunExecutionOutcomeConfig::Completed
        );

        env::remove_var("SOURCEBOT_STUB_REVIEW_AGENT_RUN_EXECUTION_OUTCOME");
    }

    #[test]
    fn stub_review_agent_run_execution_outcome_reads_failed_from_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::set_var(
            "SOURCEBOT_STUB_REVIEW_AGENT_RUN_EXECUTION_OUTCOME",
            "failed",
        );

        let config = AppConfig::from_env();

        assert_eq!(
            config
                .stub_review_agent_run_execution_outcome()
                .expect("failed should be accepted"),
            StubReviewAgentRunExecutionOutcomeConfig::Failed
        );

        env::remove_var("SOURCEBOT_STUB_REVIEW_AGENT_RUN_EXECUTION_OUTCOME");
    }

    #[test]
    fn stub_review_agent_run_execution_outcome_rejects_invalid_env_values() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::set_var("SOURCEBOT_STUB_REVIEW_AGENT_RUN_EXECUTION_OUTCOME", "bogus");

        let config = AppConfig::from_env();
        let error = config
            .stub_review_agent_run_execution_outcome()
            .expect_err("invalid outcomes should fail closed");

        assert!(error
            .to_string()
            .contains("SOURCEBOT_STUB_REVIEW_AGENT_RUN_EXECUTION_OUTCOME"));
        assert!(error.to_string().contains("bogus"));

        env::remove_var("SOURCEBOT_STUB_REVIEW_AGENT_RUN_EXECUTION_OUTCOME");
    }

    #[test]
    fn stub_repository_sync_job_execution_outcome_defaults_to_succeeded() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::remove_var("SOURCEBOT_STUB_REPOSITORY_SYNC_JOB_EXECUTION_OUTCOME");

        let config = AppConfig::from_env();

        assert_eq!(
            config
                .stub_repository_sync_job_execution_outcome()
                .expect("missing env var should default to succeeded"),
            StubRepositorySyncJobExecutionOutcomeConfig::Succeeded
        );
    }

    #[test]
    fn stub_repository_sync_job_execution_outcome_reads_failed_from_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::set_var(
            "SOURCEBOT_STUB_REPOSITORY_SYNC_JOB_EXECUTION_OUTCOME",
            "failed",
        );

        let config = AppConfig::from_env();

        assert_eq!(
            config
                .stub_repository_sync_job_execution_outcome()
                .expect("failed should be accepted"),
            StubRepositorySyncJobExecutionOutcomeConfig::Failed
        );

        env::remove_var("SOURCEBOT_STUB_REPOSITORY_SYNC_JOB_EXECUTION_OUTCOME");
    }

    #[test]
    fn stub_repository_sync_job_execution_outcome_rejects_invalid_env_values() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::set_var(
            "SOURCEBOT_STUB_REPOSITORY_SYNC_JOB_EXECUTION_OUTCOME",
            "bogus",
        );

        let config = AppConfig::from_env();
        let error = config
            .stub_repository_sync_job_execution_outcome()
            .expect_err("invalid outcomes should fail closed");

        assert!(error
            .to_string()
            .contains("SOURCEBOT_STUB_REPOSITORY_SYNC_JOB_EXECUTION_OUTCOME"));
        assert!(error.to_string().contains("bogus"));

        env::remove_var("SOURCEBOT_STUB_REPOSITORY_SYNC_JOB_EXECUTION_OUTCOME");
    }
}
