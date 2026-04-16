use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sourcebot_models::{Connection, Repository, RepositoryDetail, RepositorySummary};

pub const PROJECT_NAME: &str = "sourcebot-rewrite";

#[async_trait]
pub trait CatalogStore: Send + Sync {
    async fn list_repositories(&self) -> Result<Vec<RepositorySummary>>;
    async fn get_repository_detail(&self, repo_id: &str) -> Result<Option<RepositoryDetail>>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetrievalToolDefinition {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RetrievalToolContext {
    pub active_repo_id: Option<String>,
    pub repo_scope: Vec<String>,
    pub visible_repo_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "tool", content = "payload", rename_all = "snake_case")]
pub enum RetrievalToolResult {
    ListRepos(ListReposResult),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListReposResult {
    pub repositories: Vec<RepositorySummary>,
}

#[async_trait]
pub trait RetrievalTool: Send + Sync {
    fn definition(&self) -> RetrievalToolDefinition;
    async fn run(
        &self,
        catalog: &dyn CatalogStore,
        context: &RetrievalToolContext,
    ) -> Result<RetrievalToolResult>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ListReposTool;

#[async_trait]
impl RetrievalTool for ListReposTool {
    fn definition(&self) -> RetrievalToolDefinition {
        RetrievalToolDefinition {
            name: "list_repos".into(),
            description: "List repositories available to the retrieval scope.".into(),
        }
    }

    async fn run(
        &self,
        catalog: &dyn CatalogStore,
        context: &RetrievalToolContext,
    ) -> Result<RetrievalToolResult> {
        let repositories = catalog.list_repositories().await?;
        let scoped_repo_ids = if !context.repo_scope.is_empty() {
            context.repo_scope.as_slice()
        } else if let Some(active_repo_id) = context.active_repo_id.as_ref() {
            std::slice::from_ref(active_repo_id)
        } else {
            context.visible_repo_ids.as_slice()
        };

        let repositories = repositories
            .into_iter()
            .filter(|repository| {
                context
                    .visible_repo_ids
                    .iter()
                    .any(|repo_id| repo_id == &repository.id)
                    && scoped_repo_ids
                        .iter()
                        .any(|repo_id| repo_id == &repository.id)
            })
            .collect();

        Ok(RetrievalToolResult::ListRepos(ListReposResult {
            repositories,
        }))
    }
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
    use sourcebot_models::SyncState;

    struct StaticCatalogStore {
        repositories: Vec<RepositorySummary>,
    }

    #[async_trait]
    impl CatalogStore for StaticCatalogStore {
        async fn list_repositories(&self) -> Result<Vec<RepositorySummary>> {
            Ok(self.repositories.clone())
        }

        async fn get_repository_detail(&self, _repo_id: &str) -> Result<Option<RepositoryDetail>> {
            Ok(None)
        }
    }

    #[test]
    fn list_repos_tool_definition_is_machine_readable() {
        let tool = ListReposTool;

        assert_eq!(
            tool.definition(),
            RetrievalToolDefinition {
                name: "list_repos".into(),
                description: "List repositories available to the retrieval scope.".into(),
            }
        );
    }

    #[tokio::test]
    async fn list_repos_tool_returns_only_visible_repositories_in_scope() {
        let tool = ListReposTool;
        let store = StaticCatalogStore {
            repositories: vec![
                RepositorySummary {
                    id: "repo_sourcebot_rewrite".into(),
                    name: "sourcebot-rewrite".into(),
                    default_branch: "main".into(),
                    sync_state: SyncState::Ready,
                },
                RepositorySummary {
                    id: "repo_demo_docs".into(),
                    name: "demo-docs".into(),
                    default_branch: "main".into(),
                    sync_state: SyncState::Pending,
                },
                RepositorySummary {
                    id: "repo_secret".into(),
                    name: "secret".into(),
                    default_branch: "main".into(),
                    sync_state: SyncState::Ready,
                },
            ],
        };

        let result = tool
            .run(
                &store,
                &RetrievalToolContext {
                    active_repo_id: Some("repo_sourcebot_rewrite".into()),
                    repo_scope: vec!["repo_sourcebot_rewrite".into(), "repo_demo_docs".into()],
                    visible_repo_ids: vec!["repo_sourcebot_rewrite".into(), "repo_secret".into()],
                },
            )
            .await
            .expect("list_repos should succeed");

        assert_eq!(
            result,
            RetrievalToolResult::ListRepos(ListReposResult {
                repositories: vec![RepositorySummary {
                    id: "repo_sourcebot_rewrite".into(),
                    name: "sourcebot-rewrite".into(),
                    default_branch: "main".into(),
                    sync_state: SyncState::Ready,
                }],
            })
        );
    }

    #[tokio::test]
    async fn list_repos_tool_falls_back_to_active_repo_when_scope_is_empty() {
        let tool = ListReposTool;
        let store = StaticCatalogStore {
            repositories: vec![
                RepositorySummary {
                    id: "repo_sourcebot_rewrite".into(),
                    name: "sourcebot-rewrite".into(),
                    default_branch: "main".into(),
                    sync_state: SyncState::Ready,
                },
                RepositorySummary {
                    id: "repo_demo_docs".into(),
                    name: "demo-docs".into(),
                    default_branch: "main".into(),
                    sync_state: SyncState::Pending,
                },
            ],
        };

        let result = tool
            .run(
                &store,
                &RetrievalToolContext {
                    active_repo_id: Some("repo_demo_docs".into()),
                    repo_scope: Vec::new(),
                    visible_repo_ids: vec![
                        "repo_sourcebot_rewrite".into(),
                        "repo_demo_docs".into(),
                    ],
                },
            )
            .await
            .expect("list_repos should use the active repo fallback");

        assert_eq!(
            result,
            RetrievalToolResult::ListRepos(ListReposResult {
                repositories: vec![RepositorySummary {
                    id: "repo_demo_docs".into(),
                    name: "demo-docs".into(),
                    default_branch: "main".into(),
                    sync_state: SyncState::Pending,
                }],
            })
        );
    }

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
