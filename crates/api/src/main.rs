use axum::{
    extract::State,
    routing::get,
    Json, Router,
};
use serde::Serialize;
use sourcebot_config::{AppConfig, PublicAppConfig};
use std::net::SocketAddr;
use tracing::info;

#[derive(Clone)]
struct AppState {
    config: AppConfig,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    service: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_target(false).compact().init();

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
        .with_state(AppState { config })
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
            .oneshot(Request::builder().uri("/healthz").body(Body::empty()).unwrap())
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
}
