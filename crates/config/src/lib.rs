use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StubReviewAgentRunExecutionOutcomeConfig {
    Completed,
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
            llm_provider: Some("disabled".to_string()),
            llm_model: None,
            llm_api_base: None,
            llm_api_key: None,
        }
    }
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            service_name: env::var("SOURCEBOT_SERVICE_NAME")
                .unwrap_or_else(|_| "sourcebot-api".to_string()),
            bind_addr: env::var("SOURCEBOT_BIND_ADDR")
                .unwrap_or_else(|_| "127.0.0.1:3000".to_string()),
            database_url: env::var("DATABASE_URL").ok(),
            bootstrap_state_path: env::var("SOURCEBOT_BOOTSTRAP_STATE_PATH")
                .unwrap_or_else(|_| ".sourcebot/bootstrap-state.json".to_string()),
            local_session_state_path: env::var("SOURCEBOT_LOCAL_SESSION_STATE_PATH")
                .unwrap_or_else(|_| ".sourcebot/local-sessions.json".to_string()),
            organization_state_path: env::var("SOURCEBOT_ORGANIZATION_STATE_PATH")
                .unwrap_or_else(|_| ".sourcebot/organizations.json".to_string()),
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
    fn public_view_hides_llm_api_key_value() {
        let config = AppConfig {
            service_name: "sourcebot-api".into(),
            bind_addr: "127.0.0.1:3000".into(),
            database_url: None,
            bootstrap_state_path: ".sourcebot/bootstrap-state.json".into(),
            local_session_state_path: ".sourcebot/local-sessions.json".into(),
            organization_state_path: ".sourcebot/organizations.json".into(),
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
}
