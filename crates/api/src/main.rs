use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::Serialize;
use sourcebot_config::{AppConfig, PublicAppConfig};
use sourcebot_models::{
    seed_connections, seed_repositories, Connection, Repository, RepositoryDetail,
    RepositorySummary,
};
use std::net::SocketAddr;
use tracing::info;

#[derive(Clone)]
struct AppState {
    config: AppConfig,
    repositories: Vec<Repository>,
    connections: Vec<Connection>,
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

    let app = build_router(config);

    info!(%addr, service = %service_name, "starting sourcebot api");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn build_router(config: AppConfig) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/v1/config", get(public_config))
        .route("/api/v1/repos", get(list_repositories))
        .route("/api/v1/repos/{repo_id}", get(get_repository_detail))
        .with_state(AppState {
            config,
            repositories: seed_repositories(),
            connections: seed_connections(),
        })
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

async fn list_repositories(State(state): State<AppState>) -> Json<Vec<RepositorySummary>> {
    Json(state.repositories.iter().map(Repository::summary).collect())
}

async fn get_repository_detail(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
) -> Result<Json<RepositoryDetail>, StatusCode> {
    let repository = state
        .repositories
        .iter()
        .find(|repo| repo.id == repo_id)
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)?;

    let connection = state
        .connections
        .iter()
        .find(|conn| conn.id == repository.connection_id)
        .cloned()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(RepositoryDetail {
        repository,
        connection,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn healthz_returns_ok() {
        let app = build_router(AppConfig::default());

        let response = app
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
        let app = build_router(AppConfig {
            service_name: "sourcebot-api".into(),
            bind_addr: "127.0.0.1:3000".into(),
            database_url: Some("postgres://secret@localhost/sourcebot".into()),
        });

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
        let app = build_router(AppConfig::default());

        let response = app
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
        let app = build_router(AppConfig::default());

        let response = app
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
}
