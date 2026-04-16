use anyhow::{anyhow, Result};
use async_trait::async_trait;
use sourcebot_models::{Connection, Repository, RepositoryDetail, RepositorySummary};

pub const PROJECT_NAME: &str = "sourcebot-rewrite";

pub trait CatalogStore: Send + Sync {
    fn list_repositories(&self) -> Result<Vec<RepositorySummary>>;
    fn get_repository_detail(&self, repo_id: &str) -> Result<Option<RepositoryDetail>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskRequest {
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub repo_scope: Vec<String>,
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskCitation {
    pub repo_id: String,
    pub path: String,
    pub revision: String,
    pub line_start: usize,
    pub line_end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskResponse {
    pub provider: String,
    pub model: Option<String>,
    pub answer: String,
    pub citations: Vec<AskCitation>,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, request: &AskRequest) -> Result<AskResponse>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmProviderConfig {
    pub provider: String,
    pub model: Option<String>,
    pub api_base: Option<String>,
    pub api_key: Option<String>,
}

impl LlmProviderConfig {
    pub fn disabled() -> Self {
        Self {
            provider: "disabled".into(),
            model: None,
            api_base: None,
            api_key: None,
        }
    }

    pub fn stub(model: Option<String>) -> Self {
        Self {
            provider: "stub".into(),
            model,
            api_base: None,
            api_key: None,
        }
    }
}

pub fn build_llm_provider(config: LlmProviderConfig) -> Box<dyn LlmProvider> {
    match config.provider.as_str() {
        "stub" => Box::new(StubLlmProvider {
            model: config.model,
        }),
        _ => Box::new(DisabledLlmProvider {
            provider: config.provider,
        }),
    }
}

struct DisabledLlmProvider {
    provider: String,
}

#[async_trait]
impl LlmProvider for DisabledLlmProvider {
    async fn complete(&self, _request: &AskRequest) -> Result<AskResponse> {
        Err(anyhow!(
            "llm provider '{}' is disabled or not configured",
            self.provider
        ))
    }
}

struct StubLlmProvider {
    model: Option<String>,
}

#[async_trait]
impl LlmProvider for StubLlmProvider {
    async fn complete(&self, request: &AskRequest) -> Result<AskResponse> {
        Ok(AskResponse {
            provider: "stub".into(),
            model: self.model.clone(),
            answer: format!(
                "stub response: no real provider configured yet for prompt '{}'",
                request.prompt
            ),
            citations: Vec::new(),
        })
    }
}

pub fn build_repository_detail(
    repositories: &[Repository],
    connections: &[Connection],
    repo_id: &str,
) -> Result<Option<RepositoryDetail>> {
    let Some(repository) = repositories.iter().find(|repo| repo.id == repo_id).cloned() else {
        return Ok(None);
    };

    let connection = connections
        .iter()
        .find(|conn| conn.id == repository.connection_id)
        .cloned()
        .ok_or_else(|| anyhow!("missing connection for repository {}", repository.id))?;

    Ok(Some(RepositoryDetail {
        repository,
        connection,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn disabled_provider_returns_actionable_error() {
        let provider = build_llm_provider(LlmProviderConfig::disabled());
        let response = provider
            .complete(&AskRequest {
                prompt: "where is healthz implemented?".into(),
                system_prompt: None,
                repo_scope: vec!["repo_sourcebot_rewrite".into()],
                thread_id: None,
            })
            .await;

        let err = response.expect_err("disabled provider should fail closed");
        assert!(err.to_string().contains("disabled"));
    }

    #[tokio::test]
    async fn stub_provider_returns_answer_and_no_citations() {
        let provider = build_llm_provider(LlmProviderConfig::stub(Some("stub-model".into())));
        let response = provider
            .complete(&AskRequest {
                prompt: "where is healthz implemented?".into(),
                system_prompt: Some("answer with citations".into()),
                repo_scope: vec!["repo_sourcebot_rewrite".into()],
                thread_id: Some("thread_123".into()),
            })
            .await
            .expect("stub provider should succeed");

        assert_eq!(response.provider, "stub");
        assert_eq!(response.model.as_deref(), Some("stub-model"));
        assert!(response.answer.contains("stub response"));
        assert!(response.citations.is_empty());
    }
}
