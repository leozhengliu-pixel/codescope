use anyhow::{anyhow, Result};
use sourcebot_models::{Connection, Repository, RepositoryDetail, RepositorySummary};

pub const PROJECT_NAME: &str = "sourcebot-rewrite";

pub trait CatalogStore: Send + Sync {
    fn list_repositories(&self) -> Result<Vec<RepositorySummary>>;
    fn get_repository_detail(&self, repo_id: &str) -> Result<Option<RepositoryDetail>>;
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
