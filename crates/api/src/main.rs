mod browse;
mod commits;
mod storage;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use browse::{build_browse_store, BlobResponse, DynBrowseStore, TreeResponse};
use commits::{
    build_commit_store, CommitDetailResponse, CommitDiffResponse, CommitListResponse,
    DynCommitStore,
};
use serde::Serialize;
use sourcebot_config::{AppConfig, PublicAppConfig};
use sourcebot_models::{RepositoryDetail, RepositorySummary};
use sourcebot_search::{build_search_store, DynSearchStore, SearchResponse};
use std::net::SocketAddr;
use storage::{build_catalog_store, DynCatalogStore};
use tracing::info;

#[derive(Clone)]
struct AppState {
    config: AppConfig,
    catalog: DynCatalogStore,
    browse: DynBrowseStore,
    commits: DynCommitStore,
    search: DynSearchStore,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    service: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    let config = AppConfig::from_env();
    let addr: SocketAddr = config.bind_addr.parse()?;
    let service_name = config.service_name.clone();
    let catalog = build_catalog_store(config.database_url.as_deref()).await?;
    let browse = build_browse_store();
    let commits = build_commit_store();
    let search = build_search_store();

    let app = build_router(config, catalog, browse, commits, search);

    info!(%addr, service = %service_name, "starting sourcebot api");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn build_router(
    config: AppConfig,
    catalog: DynCatalogStore,
    browse: DynBrowseStore,
    commits: DynCommitStore,
    search: DynSearchStore,
) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/v1/config", get(public_config))
        .route("/api/v1/repos", get(list_repositories))
        .route("/api/v1/repos/{repo_id}", get(get_repository_detail))
        .route("/api/v1/repos/{repo_id}/tree", get(get_repository_tree))
        .route("/api/v1/repos/{repo_id}/blob", get(get_repository_blob))
        .route(
            "/api/v1/repos/{repo_id}/commits",
            get(list_repository_commits),
        )
        .route(
            "/api/v1/repos/{repo_id}/commits/{commit_id}",
            get(get_repository_commit),
        )
        .route(
            "/api/v1/repos/{repo_id}/commits/{commit_id}/diff",
            get(get_repository_commit_diff),
        )
        .route("/api/v1/search", get(search_repository_contents))
        .with_state(AppState {
            config,
            catalog,
            browse,
            commits,
            search,
        })
}

#[derive(Debug, serde::Deserialize, Default)]
struct BrowseQuery {
    #[serde(default)]
    path: String,
}

#[derive(Debug, serde::Deserialize, Default)]
struct SearchQuery {
    #[serde(default)]
    q: String,
    repo_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct CommitListQuery {
    #[serde(default = "default_commit_limit")]
    limit: usize,
}

fn default_commit_limit() -> usize {
    20
}

async fn healthz(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: state.config.service_name,
    })
}

async fn public_config(State(state): State<AppState>) -> Json<PublicAppConfig> {
    Json(state.config.public_view())
}

async fn list_repositories(
    State(state): State<AppState>,
) -> Result<Json<Vec<RepositorySummary>>, StatusCode> {
    let repositories = state
        .catalog
        .list_repositories()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(repositories))
}

async fn get_repository_detail(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
) -> Result<Json<RepositoryDetail>, StatusCode> {
    let detail = state
        .catalog
        .get_repository_detail(&repo_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(detail))
}

async fn get_repository_tree(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
    Query(query): Query<BrowseQuery>,
) -> Result<Json<TreeResponse>, StatusCode> {
    let tree = state
        .browse
        .get_tree(&repo_id, &query.path)
        .map_err(|_| StatusCode::NOT_FOUND)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(tree))
}

async fn get_repository_blob(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
    Query(query): Query<BrowseQuery>,
) -> Result<Json<BlobResponse>, StatusCode> {
    let blob = state
        .browse
        .get_blob(&repo_id, &query.path)
        .map_err(|_| StatusCode::NOT_FOUND)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(blob))
}

async fn list_repository_commits(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
    Query(query): Query<CommitListQuery>,
) -> Result<Json<CommitListResponse>, StatusCode> {
    let commits = state
        .commits
        .list_commits(&repo_id, query.limit)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(commits))
}

async fn get_repository_commit(
    State(state): State<AppState>,
    Path((repo_id, commit_id)): Path<(String, String)>,
) -> Result<Json<CommitDetailResponse>, StatusCode> {
    let commit = state
        .commits
        .get_commit(&repo_id, &commit_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(commit))
}

async fn get_repository_commit_diff(
    State(state): State<AppState>,
    Path((repo_id, commit_id)): Path<(String, String)>,
) -> Result<Json<CommitDiffResponse>, StatusCode> {
    let diff = state
        .commits
        .get_commit_diff(&repo_id, &commit_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(diff))
}

async fn search_repository_contents(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, StatusCode> {
    if query.q.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let response = state
        .search
        .search(&query.q, query.repo_id.as_deref())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        commits::{CommitStore, LocalCommitStore},
        storage::InMemoryCatalogStore,
    };
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;
    use tower::util::ServiceExt;

    #[derive(Debug, Deserialize)]
    struct TreeEntryResponse {
        name: String,
        path: String,
        kind: String,
    }

    #[derive(Debug, Deserialize)]
    struct TreeResponse {
        repo_id: String,
        path: String,
        entries: Vec<TreeEntryResponse>,
    }

    #[derive(Debug, Deserialize)]
    struct BlobResponse {
        repo_id: String,
        path: String,
        content: String,
        size_bytes: u64,
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct SearchResultResponse {
        repo_id: String,
        path: String,
        line_number: usize,
        line: String,
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct SearchResponse {
        query: String,
        repo_id: Option<String>,
        results: Vec<SearchResultResponse>,
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct CommitSummaryResponse {
        id: String,
        short_id: String,
        summary: String,
        author_name: String,
        authored_at: String,
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct CommitListResponse {
        repo_id: String,
        commits: Vec<CommitSummaryResponse>,
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct CommitDetailDataResponse {
        id: String,
        short_id: String,
        summary: String,
        author_name: String,
        authored_at: String,
        body: String,
        parents: Vec<String>,
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct CommitDetailResponse {
        repo_id: String,
        commit: CommitDetailDataResponse,
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct CommitDiffFileResponse {
        path: String,
        change_type: String,
        old_path: Option<String>,
        additions: usize,
        deletions: usize,
        patch: Option<String>,
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct CommitDiffResponse {
        repo_id: String,
        commit_id: String,
        files: Vec<CommitDiffFileResponse>,
    }

    fn test_app() -> Router {
        build_router(
            AppConfig::default(),
            Arc::new(InMemoryCatalogStore::seeded()),
            build_browse_store(),
            build_commit_store(),
            build_search_store(),
        )
    }

    async fn read_json<T: serde::de::DeserializeOwned>(response: axum::response::Response) -> T {
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn healthz_returns_ok() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn config_endpoint_hides_database_url_value() {
        let app = build_router(
            AppConfig {
                service_name: "sourcebot-api".into(),
                bind_addr: "127.0.0.1:3000".into(),
                database_url: Some("postgres://secret@localhost/sourcebot".into()),
            },
            Arc::new(InMemoryCatalogStore::seeded()),
            build_browse_store(),
            build_commit_store(),
            build_search_store(),
        );

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/config")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn repo_list_returns_seeded_repositories() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/repos")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn repo_detail_returns_not_found_for_unknown_repo() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/repos/missing")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn repo_detail_returns_seeded_repository() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/repos/repo_sourcebot_rewrite")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn repo_tree_returns_root_directory_entries() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/repos/repo_sourcebot_rewrite/tree")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let payload: TreeResponse = read_json(response).await;
        assert_eq!(payload.repo_id, "repo_sourcebot_rewrite");
        assert_eq!(payload.path, "");
        assert!(payload.entries.iter().any(|entry| {
            entry.name == "Cargo.toml" && entry.path == "Cargo.toml" && entry.kind == "file"
        }));
        assert!(payload
            .entries
            .iter()
            .any(|entry| entry.name == "crates" && entry.path == "crates" && entry.kind == "dir"));
    }

    #[tokio::test]
    async fn repo_blob_returns_file_contents() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/repos/repo_sourcebot_rewrite/blob?path=Cargo.toml")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let payload: BlobResponse = read_json(response).await;
        assert_eq!(payload.repo_id, "repo_sourcebot_rewrite");
        assert_eq!(payload.path, "Cargo.toml");
        assert!(payload.content.contains("[workspace]"));
        assert!(payload.size_bytes > 0);
    }

    #[tokio::test]
    async fn repo_tree_rejects_parent_directory_traversal() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/repos/repo_sourcebot_rewrite/tree?path=..")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn repo_blob_returns_not_found_for_unknown_repo() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/repos/repo_demo_docs/blob?path=README.md")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn repo_blob_returns_not_found_for_missing_path() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/repos/repo_sourcebot_rewrite/blob?path=definitely-missing-file")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn repo_blob_returns_not_found_for_directory_path() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/repos/repo_sourcebot_rewrite/blob?path=crates")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn repo_commits_returns_real_git_history() {
        let expected = LocalCommitStore::seeded()
            .list_commits("repo_sourcebot_rewrite", 2)
            .unwrap()
            .unwrap();

        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/repos/repo_sourcebot_rewrite/commits?limit=2")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let payload: CommitListResponse = read_json(response).await;
        assert_eq!(
            serde_json::to_value(&payload).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
        assert_eq!(payload.repo_id, "repo_sourcebot_rewrite");
        assert_eq!(payload.commits.len(), 2);
        assert_eq!(payload.commits[0].author_name, "Hermes Agent");
        assert_eq!(payload.commits[0].id.len(), 40);
        assert!(payload.commits[0].authored_at.ends_with('Z'));
    }

    #[tokio::test]
    async fn repo_commit_detail_returns_real_git_commit() {
        let commit_id = LocalCommitStore::seeded()
            .list_commits("repo_sourcebot_rewrite", 2)
            .unwrap()
            .unwrap()
            .commits
            .into_iter()
            .nth(1)
            .expect("seeded repository should expose at least two commits")
            .short_id;
        let expected = LocalCommitStore::seeded()
            .get_commit("repo_sourcebot_rewrite", &commit_id)
            .unwrap()
            .unwrap();

        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/v1/repos/repo_sourcebot_rewrite/commits/{commit_id}"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let payload: CommitDetailResponse = read_json(response).await;
        assert_eq!(
            serde_json::to_value(&payload).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
        assert_eq!(payload.repo_id, "repo_sourcebot_rewrite");
        assert_eq!(payload.commit.author_name, "Hermes Agent");
        assert_eq!(payload.commit.body, "");
        assert_eq!(payload.commit.id.len(), 40);
        assert!(payload.commit.authored_at.ends_with('Z'));
    }

    #[tokio::test]
    async fn repo_commits_returns_empty_list_for_supported_repo_without_local_history() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/repos/repo_demo_docs/commits")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let payload: CommitListResponse = read_json(response).await;
        assert_eq!(payload.repo_id, "repo_demo_docs");
        assert!(payload.commits.is_empty());
    }

    #[tokio::test]
    async fn repo_commit_detail_returns_not_found_for_repo_without_local_history() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/repos/repo_demo_docs/commits/556fb45")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn repo_commit_detail_rejects_revision_ranges() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/repos/repo_sourcebot_rewrite/commits/HEAD~1..HEAD")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn repo_commit_diff_returns_real_git_files() {
        let commit_id = LocalCommitStore::seeded()
            .list_commits("repo_sourcebot_rewrite", 1)
            .unwrap()
            .unwrap()
            .commits
            .into_iter()
            .next()
            .expect("seeded repository should expose at least one commit")
            .short_id;
        let expected = LocalCommitStore::seeded()
            .get_commit_diff("repo_sourcebot_rewrite", &commit_id)
            .unwrap()
            .unwrap();

        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/v1/repos/repo_sourcebot_rewrite/commits/{commit_id}/diff"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let payload: CommitDiffResponse = read_json(response).await;
        assert_eq!(
            serde_json::to_value(&payload).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
        assert_eq!(payload.repo_id, "repo_sourcebot_rewrite");
        assert_eq!(payload.commit_id.len(), 40);
        assert!(!payload.files.is_empty());
        assert!(payload.files.iter().all(|file| !file.path.is_empty()));
        assert!(payload.files.iter().any(|file| file.patch.is_some()));
    }

    #[tokio::test]
    async fn repo_commit_diff_returns_not_found_for_repo_without_local_history() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/repos/repo_demo_docs/commits/fe7f21f/diff")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn repo_commit_diff_rejects_revision_ranges() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/repos/repo_sourcebot_rewrite/commits/HEAD~1..HEAD/diff")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn search_returns_bad_request_for_empty_query() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/search?q=&repo_id=repo_sourcebot_rewrite")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn search_returns_matches_for_seeded_local_repository() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/search?q=build_router&repo_id=repo_sourcebot_rewrite")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let payload: SearchResponse = read_json(response).await;
        assert_eq!(payload.query, "build_router");
        assert_eq!(payload.repo_id.as_deref(), Some("repo_sourcebot_rewrite"));
        assert!(!payload.results.is_empty());
        assert!(payload.results.iter().any(|result| {
            result.repo_id == "repo_sourcebot_rewrite"
                && result.path == "crates/api/src/main.rs"
                && result.line.contains("build_router")
                && result.line_number > 0
        }));
    }
}
