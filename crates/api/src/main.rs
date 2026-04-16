mod ask;
mod auth;
mod browse;
mod commits;
mod storage;

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use ask::{build_ask_thread_store, AskCompletionRequest, AskCompletionResponse, DynAskThreadStore};
use auth::{
    build_bootstrap_store, build_local_session_store, build_organization_store, DynBootstrapStore,
    DynLocalSessionStore, DynOrganizationStore,
};
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use browse::{build_browse_store, BlobResponse, DynBrowseStore, ReferenceMatch, TreeResponse};
use commits::{
    build_commit_store, CommitDetailResponse, CommitDiffResponse, CommitListResponse,
    DynCommitStore,
};
use serde::{Deserialize, Serialize};
use sourcebot_config::{AppConfig, PublicAppConfig};
use sourcebot_core::{build_llm_provider, visible_repo_ids_for_user, LlmProviderConfig};
use sourcebot_models::{
    AskMessage, AskMessageRole, AskThread, AskThreadVisibility, BootstrapState, BootstrapStatus,
    LocalSession, RepositoryDetail, RepositorySummary,
};
use sourcebot_search::{
    build_search_store, extract_symbols, DynSearchStore, SearchResponse, SymbolKind,
};
use std::collections::HashSet;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use storage::{build_catalog_store, DynCatalogStore};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tracing::info;

#[derive(Clone)]
struct AppState {
    config: AppConfig,
    catalog: DynCatalogStore,
    bootstrap: DynBootstrapStore,
    local_sessions: DynLocalSessionStore,
    organization_store: DynOrganizationStore,
    browse: DynBrowseStore,
    commits: DynCommitStore,
    search: DynSearchStore,
    ask_threads: DynAskThreadStore,
}

const DEFAULT_ASK_USER_ID: &str = "local_user";
const LOCAL_BOOTSTRAP_ADMIN_USER_ID: &str = "local_user_bootstrap_admin";
static NEXT_ASK_ENTITY_ID: AtomicU64 = AtomicU64::new(1);

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
    let bootstrap = build_bootstrap_store(config.bootstrap_state_path.clone());
    let local_sessions = build_local_session_store(config.local_session_state_path.clone());
    let browse = build_browse_store();
    let commits = build_commit_store();
    let search = build_search_store();
    let ask_threads = build_ask_thread_store();

    let app = build_router(
        config,
        catalog,
        bootstrap,
        local_sessions,
        browse,
        commits,
        search,
        ask_threads,
    );

    info!(%addr, service = %service_name, "starting sourcebot api");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn build_app_state(
    config: AppConfig,
    catalog: DynCatalogStore,
    bootstrap: DynBootstrapStore,
    local_sessions: DynLocalSessionStore,
    browse: DynBrowseStore,
    commits: DynCommitStore,
    search: DynSearchStore,
    ask_threads: DynAskThreadStore,
) -> AppState {
    let organization_store = build_organization_store(config.organization_state_path.clone());

    AppState {
        config,
        catalog,
        bootstrap,
        local_sessions,
        organization_store,
        browse,
        commits,
        search,
        ask_threads,
    }
}

fn build_router(
    config: AppConfig,
    catalog: DynCatalogStore,
    bootstrap: DynBootstrapStore,
    local_sessions: DynLocalSessionStore,
    browse: DynBrowseStore,
    commits: DynCommitStore,
    search: DynSearchStore,
    ask_threads: DynAskThreadStore,
) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route(
            "/api/v1/auth/bootstrap",
            get(get_bootstrap_status).post(create_bootstrap_admin),
        )
        .route("/api/v1/auth/login", post(login_local_admin))
        .route("/api/v1/auth/me", get(get_authenticated_local_admin))
        .route("/api/v1/auth/logout", post(logout_local_admin))
        .route("/api/v1/auth/revoke", post(revoke_local_admin_session))
        .route("/api/v1/config", get(public_config))
        .route("/api/v1/repos", get(list_repositories))
        .route("/api/v1/repos/{repo_id}", get(get_repository_detail))
        .route("/api/v1/repos/{repo_id}/tree", get(get_repository_tree))
        .route("/api/v1/repos/{repo_id}/blob", get(get_repository_blob))
        .route(
            "/api/v1/repos/{repo_id}/definitions",
            get(get_repository_definitions),
        )
        .route(
            "/api/v1/repos/{repo_id}/references",
            get(get_repository_references),
        )
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
        .route("/api/v1/ask/completions", post(create_ask_completion))
        .with_state(build_app_state(
            config,
            catalog,
            bootstrap,
            local_sessions,
            browse,
            commits,
            search,
            ask_threads,
        ))
}

fn next_ask_entity_id(prefix: &str) -> String {
    let sequence = NEXT_ASK_ENTITY_ID.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}_{sequence}")
}

fn current_timestamp() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("current UTC time should format as RFC3339")
}

fn ask_thread_title_from_prompt(prompt: &str) -> String {
    const MAX_TITLE_CHARS: usize = 80;

    prompt.chars().take(MAX_TITLE_CHARS).collect()
}

#[derive(Debug, serde::Deserialize, Default)]
struct BrowseQuery {
    #[serde(default)]
    path: String,
    revision: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct SearchQuery {
    #[serde(default)]
    q: String,
    repo_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BootstrapCreateBody {
    email: String,
    name: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct LoginBody {
    email: String,
    password: String,
}

#[derive(Debug, Deserialize, Default)]
struct RevokeLocalSessionBody {
    #[serde(default)]
    session_id: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct LoginResponse {
    session_id: String,
    session_secret: String,
    user_id: String,
    created_at: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct AuthMeResponse {
    user_id: String,
    email: String,
    name: String,
    session_id: String,
    created_at: String,
}

#[derive(Debug, Deserialize, Default)]
struct DefinitionsQuery {
    path: Option<String>,
    symbol: Option<String>,
    revision: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ReferencesQuery {
    path: Option<String>,
    symbol: Option<String>,
    revision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct DefinitionRangeResponse {
    start_line: usize,
    end_line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct DefinitionResponse {
    path: String,
    name: String,
    kind: SymbolKind,
    range: DefinitionRangeResponse,
    browse_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status")]
enum DefinitionsResponse {
    #[serde(rename = "supported")]
    Supported {
        repo_id: String,
        path: String,
        revision: Option<String>,
        symbol: String,
        definitions: Vec<DefinitionResponse>,
    },
    #[serde(rename = "unsupported")]
    Unsupported {
        repo_id: String,
        path: String,
        revision: Option<String>,
        symbol: String,
        capability: String,
        definitions: Vec<DefinitionResponse>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ReferenceResponse {
    path: String,
    line_number: usize,
    line: String,
    browse_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status")]
enum ReferencesResponse {
    #[serde(rename = "supported")]
    Supported {
        repo_id: String,
        path: String,
        revision: Option<String>,
        symbol: String,
        references: Vec<ReferenceResponse>,
    },
    #[serde(rename = "unsupported")]
    Unsupported {
        repo_id: String,
        path: String,
        revision: Option<String>,
        symbol: String,
        capability: String,
        references: Vec<ReferenceResponse>,
    },
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

async fn get_bootstrap_status(
    State(state): State<AppState>,
) -> Result<Json<BootstrapStatus>, StatusCode> {
    let status = state
        .bootstrap
        .bootstrap_status()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(status))
}

async fn create_bootstrap_admin(
    State(state): State<AppState>,
    Json(payload): Json<BootstrapCreateBody>,
) -> Result<(StatusCode, Json<BootstrapStatus>), StatusCode> {
    let email = payload.email.trim();
    let name = payload.name.trim();
    if email.is_empty() || name.is_empty() || payload.password.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let status = state
        .bootstrap
        .bootstrap_status()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !status.bootstrap_required {
        return Err(StatusCode::CONFLICT);
    }

    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
        .hash_password(payload.password.as_bytes(), &salt)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .to_string();
    let bootstrap_state = BootstrapState {
        initialized_at: current_timestamp(),
        admin_email: email.to_string(),
        admin_name: name.to_string(),
        password_hash,
    };

    state
        .bootstrap
        .initialize_bootstrap(bootstrap_state)
        .await
        .map_err(map_bootstrap_initialize_error)?;

    Ok((
        StatusCode::CREATED,
        Json(BootstrapStatus {
            bootstrap_required: false,
        }),
    ))
}

fn map_bootstrap_initialize_error(error: anyhow::Error) -> StatusCode {
    if error
        .downcast_ref::<std::io::Error>()
        .is_some_and(|io_error| io_error.kind() == ErrorKind::AlreadyExists)
    {
        StatusCode::CONFLICT
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

async fn login_local_admin(
    State(state): State<AppState>,
    Json(payload): Json<LoginBody>,
) -> Result<(StatusCode, Json<LoginResponse>), StatusCode> {
    let email = payload.email.trim();
    let password = payload.password;
    if email.is_empty() || password.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let bootstrap_state = state
        .bootstrap
        .bootstrap_state()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::CONFLICT)?;
    if bootstrap_state.admin_email != email {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let password_hash = PasswordHash::new(&bootstrap_state.password_hash)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Argon2::default()
        .verify_password(password.as_bytes(), &password_hash)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let session_id = format!(
        "local_session_{}",
        SaltString::generate(&mut OsRng)
            .to_string()
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .collect::<String>()
    );
    let session_secret = SaltString::generate(&mut OsRng).to_string();
    let secret_hash = Argon2::default()
        .hash_password(session_secret.as_bytes(), &SaltString::generate(&mut OsRng))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .to_string();
    let created_at = current_timestamp();

    state
        .local_sessions
        .store_local_session(LocalSession {
            id: session_id.clone(),
            user_id: LOCAL_BOOTSTRAP_ADMIN_USER_ID.to_string(),
            secret_hash,
            created_at: created_at.clone(),
        })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((
        StatusCode::CREATED,
        Json(LoginResponse {
            session_id,
            session_secret,
            user_id: LOCAL_BOOTSTRAP_ADMIN_USER_ID.to_string(),
            created_at,
        }),
    ))
}

fn parse_local_session_bearer_token(headers: &HeaderMap) -> Result<(String, String), StatusCode> {
    let authorization = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let token = authorization
        .strip_prefix("Bearer ")
        .filter(|value| !value.is_empty())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let (session_id, session_secret) = token.split_once(':').ok_or(StatusCode::UNAUTHORIZED)?;
    if session_id.is_empty() || session_secret.is_empty() || session_secret.contains(':') {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok((session_id.to_string(), session_secret.to_string()))
}

async fn authenticate_local_session_record(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<LocalSession, StatusCode> {
    let (session_id, session_secret) = parse_local_session_bearer_token(headers)?;
    let session = state
        .local_sessions
        .local_session(&session_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if !local_session_record_is_well_formed(&session, &session_id) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let secret_hash =
        PasswordHash::new(&session.secret_hash).map_err(|_| StatusCode::UNAUTHORIZED)?;
    Argon2::default()
        .verify_password(session_secret.as_bytes(), &secret_hash)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    Ok(session)
}

async fn authenticate_local_session(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(BootstrapState, LocalSession), StatusCode> {
    let session = authenticate_local_session_record(state, headers).await?;
    if session.user_id != LOCAL_BOOTSTRAP_ADMIN_USER_ID {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let bootstrap_state = state
        .bootstrap
        .bootstrap_state()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if bootstrap_state.admin_email.trim().is_empty() || bootstrap_state.admin_name.trim().is_empty()
    {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok((bootstrap_state, session))
}

async fn get_authenticated_local_admin(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AuthMeResponse>, StatusCode> {
    let (bootstrap_state, session) = authenticate_local_session(&state, &headers).await?;

    Ok(Json(AuthMeResponse {
        user_id: LOCAL_BOOTSTRAP_ADMIN_USER_ID.to_string(),
        email: bootstrap_state.admin_email,
        name: bootstrap_state.admin_name,
        session_id: session.id,
        created_at: session.created_at,
    }))
}

async fn logout_local_admin(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let (_, session) = authenticate_local_session(&state, &headers).await?;
    let deleted = state
        .local_sessions
        .delete_local_session(&session.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !deleted {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(StatusCode::NO_CONTENT)
}

fn local_session_record_is_well_formed(session: &LocalSession, expected_session_id: &str) -> bool {
    if session.id != expected_session_id
        || session.id.trim().is_empty()
        || session.user_id.trim().is_empty()
        || session.created_at.trim().is_empty()
    {
        return false;
    }

    PasswordHash::new(&session.secret_hash).is_ok()
}

async fn revoke_local_admin_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RevokeLocalSessionBody>,
) -> Result<StatusCode, StatusCode> {
    let (_, authenticated_session) = authenticate_local_session(&state, &headers).await?;
    let target_session_id = payload.session_id.trim();
    if target_session_id.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let target_session = state
        .local_sessions
        .local_session(target_session_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if !local_session_record_is_well_formed(&target_session, target_session_id)
        || target_session.user_id != authenticated_session.user_id
        || target_session.user_id != LOCAL_BOOTSTRAP_ADMIN_USER_ID
    {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let deleted = state
        .local_sessions
        .delete_local_session(target_session_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !deleted {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn list_repositories(
    State(state): State<AppState>,
) -> Result<Json<Vec<RepositorySummary>>, StatusCode> {
    let repositories = state
        .catalog
        .list_repositories()
        .await
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
        .await
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
    let revision = query
        .revision
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let blob = state
        .browse
        .get_blob_at_revision(&repo_id, &query.path, revision)
        .map_err(map_browse_error_to_status)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(blob))
}

async fn get_repository_definitions(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
    Query(query): Query<DefinitionsQuery>,
) -> Result<Json<DefinitionsResponse>, StatusCode> {
    let path = query
        .path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(StatusCode::BAD_REQUEST)?;
    let symbol = query
        .symbol
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(StatusCode::BAD_REQUEST)?;
    let requested_revision = query
        .revision
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let effective_revision = requested_revision.as_deref().unwrap_or("HEAD");
    let response_revision = Some(effective_revision.to_string());

    let blob = state
        .browse
        .get_blob_at_revision(&repo_id, path, Some(effective_revision))
        .map_err(map_browse_error_to_status)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let response = match extract_symbols(path, &blob.content) {
        sourcebot_search::SymbolExtraction::Supported { symbols } => {
            let definitions = symbols
                .into_iter()
                .filter(|candidate| candidate.name == symbol)
                .map(|candidate| DefinitionResponse {
                    browse_url: build_definition_browse_url(
                        &repo_id,
                        &candidate.path,
                        Some(effective_revision),
                        candidate.range.start_line,
                    ),
                    path: candidate.path,
                    name: candidate.name,
                    kind: candidate.kind,
                    range: DefinitionRangeResponse {
                        start_line: candidate.range.start_line,
                        end_line: candidate.range.end_line,
                    },
                })
                .collect();

            DefinitionsResponse::Supported {
                repo_id,
                path: path.to_string(),
                revision: response_revision,
                symbol: symbol.to_string(),
                definitions,
            }
        }
        sourcebot_search::SymbolExtraction::Unsupported { capability, .. } => {
            DefinitionsResponse::Unsupported {
                repo_id,
                path: path.to_string(),
                revision: response_revision,
                symbol: symbol.to_string(),
                capability,
                definitions: Vec::new(),
            }
        }
    };

    Ok(Json(response))
}

async fn get_repository_references(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
    Query(query): Query<ReferencesQuery>,
) -> Result<Json<ReferencesResponse>, StatusCode> {
    let path = query
        .path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(StatusCode::BAD_REQUEST)?;
    let symbol = query
        .symbol
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(StatusCode::BAD_REQUEST)?;
    let requested_revision = query
        .revision
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let effective_revision = requested_revision.as_deref().unwrap_or("HEAD");
    let response_revision = Some(effective_revision.to_string());

    let blob = state
        .browse
        .get_blob_at_revision(&repo_id, path, Some(effective_revision))
        .map_err(map_browse_error_to_status)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let response = match extract_symbols(path, &blob.content) {
        sourcebot_search::SymbolExtraction::Supported { .. } => {
            let references = state
                .browse
                .find_text_references_at_revision(&repo_id, symbol, effective_revision)
                .map_err(map_browse_error_to_status)?
                .ok_or(StatusCode::NOT_FOUND)?;
            let references =
                build_reference_responses(&repo_id, Some(effective_revision), references);

            ReferencesResponse::Supported {
                repo_id,
                path: path.to_string(),
                revision: response_revision.clone(),
                symbol: symbol.to_string(),
                references,
            }
        }
        sourcebot_search::SymbolExtraction::Unsupported { capability, .. } => {
            ReferencesResponse::Unsupported {
                repo_id,
                path: path.to_string(),
                revision: response_revision,
                symbol: symbol.to_string(),
                capability,
                references: Vec::new(),
            }
        }
    };

    Ok(Json(response))
}

fn build_definition_browse_url(
    repo_id: &str,
    path: &str,
    revision: Option<&str>,
    start_line: usize,
) -> String {
    build_blob_browse_url(repo_id, path, revision, start_line)
}

fn build_reference_browse_url(
    repo_id: &str,
    path: &str,
    revision: Option<&str>,
    line_number: usize,
) -> String {
    build_blob_browse_url(repo_id, path, revision, line_number)
}

fn build_blob_browse_url(
    repo_id: &str,
    path: &str,
    revision: Option<&str>,
    line_number: usize,
) -> String {
    let encoded_path = encode_query_value(path);
    let revision_suffix = revision
        .map(|revision| format!("&revision={}", encode_query_value(revision)))
        .unwrap_or_default();
    format!("/api/v1/repos/{repo_id}/blob?path={encoded_path}{revision_suffix}#L{line_number}")
}

fn build_reference_responses(
    repo_id: &str,
    revision: Option<&str>,
    references: Vec<ReferenceMatch>,
) -> Vec<ReferenceResponse> {
    let mut references = references
        .into_iter()
        .map(|reference| ReferenceResponse {
            browse_url: build_reference_browse_url(
                repo_id,
                &reference.path,
                revision,
                reference.line_number,
            ),
            path: reference.path,
            line_number: reference.line_number,
            line: reference.line,
        })
        .collect::<Vec<_>>();

    references.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.line_number.cmp(&right.line_number))
            .then(left.line.cmp(&right.line))
            .then(left.browse_url.cmp(&right.browse_url))
    });
    references.dedup();
    references
}

fn encode_query_value(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{:02X}", byte)),
        }
    }
    encoded
}

fn map_browse_error_to_status(error: anyhow::Error) -> StatusCode {
    if error.to_string().contains("invalid relative path") {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::NOT_FOUND
    }
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

async fn visible_repo_ids_for_request(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<HashSet<String>, StatusCode> {
    let session = authenticate_local_session_record(state, headers).await?;
    let organization_state = state
        .organization_store
        .organization_state()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(
        visible_repo_ids_for_user(&organization_state, &session.user_id)
            .into_iter()
            .collect(),
    )
}

async fn search_repository_contents(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, StatusCode> {
    let visible_repo_ids = visible_repo_ids_for_request(&state, &headers).await?;

    if query.q.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let requested_repo_id = query
        .repo_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(repo_id) = requested_repo_id {
        if !visible_repo_ids.contains(repo_id) {
            return Err(StatusCode::NOT_FOUND);
        }
    }

    let mut response = state
        .search
        .search(&query.q, requested_repo_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    response
        .results
        .retain(|result| visible_repo_ids.contains(&result.repo_id));

    Ok(Json(response))
}

async fn create_ask_completion(
    State(state): State<AppState>,
    Json(request): Json<AskCompletionRequest>,
) -> Result<Json<AskCompletionResponse>, StatusCode> {
    let repo_ids = state
        .catalog
        .list_repositories()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .map(|repository| repository.id)
        .collect::<Vec<_>>();
    let request = request.into_core_request(&repo_ids)?;

    let provider = build_llm_provider(LlmProviderConfig {
        provider: state
            .config
            .llm_provider
            .clone()
            .unwrap_or_else(|| "disabled".into()),
        model: state.config.llm_model.clone(),
        api_base: state.config.llm_api_base.clone(),
        api_key: state.config.llm_api_key.clone(),
    });
    let response = provider
        .complete(&request)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(thread_id) = request.thread_id.as_deref() {
        let timestamp = current_timestamp();
        state
            .ask_threads
            .append_message_for_user(
                DEFAULT_ASK_USER_ID,
                thread_id,
                AskMessage {
                    id: next_ask_entity_id("msg"),
                    role: AskMessageRole::User,
                    content: request.prompt.clone(),
                    citations: Vec::new(),
                },
                &timestamp,
            )
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;
        state
            .ask_threads
            .append_message_for_user(
                DEFAULT_ASK_USER_ID,
                thread_id,
                AskMessage {
                    id: next_ask_entity_id("msg"),
                    role: AskMessageRole::Assistant,
                    content: response.answer.clone(),
                    citations: response.citations.clone(),
                },
                &timestamp,
            )
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;
    } else {
        let timestamp = current_timestamp();
        state
            .ask_threads
            .create_thread(AskThread {
                id: next_ask_entity_id("thread"),
                session_id: next_ask_entity_id("session"),
                user_id: DEFAULT_ASK_USER_ID.into(),
                title: ask_thread_title_from_prompt(&request.prompt),
                repo_scope: request.repo_scope.clone(),
                visibility: AskThreadVisibility::Private,
                created_at: timestamp.clone(),
                updated_at: timestamp,
                messages: vec![
                    AskMessage {
                        id: next_ask_entity_id("msg"),
                        role: AskMessageRole::User,
                        content: request.prompt.clone(),
                        citations: Vec::new(),
                    },
                    AskMessage {
                        id: next_ask_entity_id("msg"),
                        role: AskMessageRole::Assistant,
                        content: response.answer.clone(),
                        citations: response.citations.clone(),
                    },
                ],
            })
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(Json(response.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ask::InMemoryAskThreadStore,
        commits::{CommitStore, LocalCommitStore},
        storage::InMemoryCatalogStore,
    };
    use async_trait::async_trait;
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use serde::{Deserialize, Serialize};
    use sourcebot_core::{AskThreadStore, BootstrapStore};
    use sourcebot_models::{
        LocalAccount, LocalSessionState, Organization, OrganizationInvite, OrganizationMembership,
        OrganizationRole, OrganizationState, RepositoryPermissionBinding,
    };
    use sourcebot_search::build_search_store;
    use std::sync::Arc;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };
    use time::{format_description::well_known::Rfc3339, OffsetDateTime};
    use tower::ServiceExt;

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

    #[derive(Debug, Deserialize)]
    struct ReferenceResponse {
        path: String,
        line_number: usize,
        line: String,
        browse_url: String,
    }

    #[derive(Debug, Deserialize)]
    #[serde(tag = "status")]
    enum ReferencesResponse {
        #[serde(rename = "supported")]
        Supported {
            repo_id: String,
            path: String,
            revision: Option<String>,
            symbol: String,
            references: Vec<ReferenceResponse>,
        },
        #[serde(rename = "unsupported")]
        Unsupported {
            repo_id: String,
            path: String,
            revision: Option<String>,
            symbol: String,
            capability: String,
            references: Vec<ReferenceResponse>,
        },
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

    #[derive(Debug, Deserialize)]
    struct AskCompletionResponseBody {
        provider: String,
        model: Option<String>,
        answer: String,
    }

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct BootstrapStatusResponse {
        bootstrap_required: bool,
    }

    #[derive(Debug, Serialize)]
    struct BootstrapCreateRequest {
        email: String,
        name: String,
        password: String,
    }

    #[derive(Debug, Serialize)]
    struct LoginRequest {
        email: String,
        password: String,
    }

    #[derive(Debug, Serialize)]
    struct RevokeRequest {
        session_id: String,
    }

    #[derive(Debug, Deserialize)]
    struct LoginResponseBody {
        session_id: String,
        session_secret: String,
        user_id: String,
        created_at: String,
    }

    #[derive(Debug, Deserialize)]
    struct AuthMeResponseBody {
        user_id: String,
        email: String,
        name: String,
        session_id: String,
        created_at: String,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct BootstrapStateResponse {
        initialized_at: String,
        admin_email: String,
        admin_name: String,
        password_hash: String,
    }

    #[derive(Debug, Serialize)]
    struct AskCompletionRequest {
        prompt: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        system_prompt: Option<String>,
        repo_scope: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        thread_id: Option<String>,
    }

    fn test_app() -> Router {
        test_app_with_config(AppConfig::default())
    }

    fn unique_test_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("sourcebot-bootstrap-{name}-{nanos}.json"))
    }

    fn test_app_with_config(config: AppConfig) -> Router {
        let bootstrap_state_path = config.bootstrap_state_path.clone();
        let local_session_state_path = config.local_session_state_path.clone();
        build_router(
            config,
            Arc::new(InMemoryCatalogStore::seeded()),
            build_bootstrap_store(bootstrap_state_path),
            build_local_session_store(local_session_state_path),
            build_browse_store(),
            build_commit_store(),
            build_search_store(),
            build_ask_thread_store(),
        )
    }

    #[tokio::test]
    async fn build_app_state_constructs_file_backed_organization_store_from_configured_path() {
        let configured_organization_state_path = unique_test_path("app-state-organizations-config");
        let injected_organization_state_path = unique_test_path("app-state-organizations-injected");
        let expected_state = OrganizationState {
            organizations: vec![Organization {
                id: "org_acme".into(),
                slug: "acme".into(),
                name: "Acme".into(),
            }],
            memberships: vec![OrganizationMembership {
                organization_id: "org_acme".into(),
                user_id: "user_admin".into(),
                role: OrganizationRole::Admin,
                joined_at: "2026-04-21T00:00:00Z".into(),
            }],
            accounts: vec![LocalAccount {
                id: "user_admin".into(),
                email: "admin@example.com".into(),
                name: "Admin User".into(),
                created_at: "2026-04-20T23:55:00Z".into(),
            }],
            invites: vec![OrganizationInvite {
                id: "invite_reviewer".into(),
                organization_id: "org_acme".into(),
                email: "reviewer@example.com".into(),
                role: OrganizationRole::Viewer,
                invited_by_user_id: "user_admin".into(),
                created_at: "2026-04-21T00:05:00Z".into(),
                expires_at: "2026-04-28T00:05:00Z".into(),
                accepted_by_user_id: None,
                accepted_at: None,
            }],
            repo_permissions: vec![RepositoryPermissionBinding {
                organization_id: "org_acme".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                synced_at: "2026-04-21T00:06:00Z".into(),
            }],
        };
        let mismatched_state = OrganizationState {
            organizations: vec![Organization {
                id: "org_other".into(),
                slug: "other".into(),
                name: "Other".into(),
            }],
            memberships: vec![],
            accounts: vec![],
            invites: vec![],
            repo_permissions: vec![RepositoryPermissionBinding {
                organization_id: "org_other".into(),
                repository_id: "repo_other".into(),
                synced_at: "2026-04-21T00:07:00Z".into(),
            }],
        };
        fs::write(
            &configured_organization_state_path,
            serde_json::to_vec(&expected_state).unwrap(),
        )
        .unwrap();
        fs::write(
            &injected_organization_state_path,
            serde_json::to_vec(&mismatched_state).unwrap(),
        )
        .unwrap();

        let config = AppConfig {
            organization_state_path: configured_organization_state_path.display().to_string(),
            ..AppConfig::default()
        };
        let state = build_app_state(
            config,
            Arc::new(InMemoryCatalogStore::seeded()),
            build_bootstrap_store(unique_test_path("app-state-bootstrap")),
            build_local_session_store(unique_test_path("app-state-local-sessions")),
            build_browse_store(),
            build_commit_store(),
            build_search_store(),
            build_ask_thread_store(),
        );

        let persisted_state = state.organization_store.organization_state().await.unwrap();
        assert_eq!(persisted_state, expected_state);
        assert_ne!(persisted_state, mismatched_state);

        fs::remove_file(configured_organization_state_path).unwrap();
        fs::remove_file(injected_organization_state_path).unwrap();
    }

    async fn read_json<T: serde::de::DeserializeOwned>(response: axum::response::Response) -> T {
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    fn write_organization_state_fixture(path: &PathBuf, user_id: &str, repo_ids: &[&str]) {
        let state = OrganizationState {
            organizations: vec![Organization {
                id: "org_acme".into(),
                slug: "acme".into(),
                name: "Acme".into(),
            }],
            memberships: vec![OrganizationMembership {
                organization_id: "org_acme".into(),
                user_id: user_id.into(),
                role: OrganizationRole::Viewer,
                joined_at: "2026-04-21T00:00:00Z".into(),
            }],
            accounts: vec![LocalAccount {
                id: user_id.into(),
                email: "admin@example.com".into(),
                name: "Admin User".into(),
                created_at: "2026-04-20T23:55:00Z".into(),
            }],
            repo_permissions: repo_ids
                .iter()
                .map(|repo_id| RepositoryPermissionBinding {
                    organization_id: "org_acme".into(),
                    repository_id: (*repo_id).into(),
                    synced_at: "2026-04-21T00:06:00Z".into(),
                })
                .collect(),
            ..OrganizationState::default()
        };

        fs::write(path, serde_json::to_vec(&state).unwrap()).unwrap();
    }

    async fn bootstrap_and_login(app: &Router) -> String {
        let bootstrap_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/bootstrap")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&BootstrapCreateRequest {
                            email: "admin@example.com".into(),
                            name: "Admin User".into(),
                            password: "hunter2".into(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(bootstrap_response.status(), StatusCode::CREATED);

        let login_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&LoginRequest {
                            email: "admin@example.com".into(),
                            password: "hunter2".into(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(login_response.status(), StatusCode::CREATED);

        let payload: LoginResponseBody = read_json(login_response).await;
        format!("Bearer {}:{}", payload.session_id, payload.session_secret)
    }

    async fn seed_local_session(state_path: &str, user_id: &str) -> String {
        let session_id = format!("seeded_session_{user_id}");
        let session_secret = format!("secret_for_{user_id}");
        let secret_hash = Argon2::default()
            .hash_password(session_secret.as_bytes(), &SaltString::generate(&mut OsRng))
            .unwrap()
            .to_string();
        build_local_session_store(state_path.to_string())
            .store_local_session(LocalSession {
                id: session_id.clone(),
                user_id: user_id.into(),
                secret_hash,
                created_at: "2026-04-21T00:07:00Z".into(),
            })
            .await
            .unwrap();

        format!("Bearer {session_id}:{session_secret}")
    }

    #[derive(Debug)]
    struct AlreadyInitializedBootstrapStore;

    #[async_trait]
    impl BootstrapStore for AlreadyInitializedBootstrapStore {
        async fn bootstrap_status(&self) -> anyhow::Result<BootstrapStatus> {
            Ok(BootstrapStatus {
                bootstrap_required: true,
            })
        }

        async fn bootstrap_state(&self) -> anyhow::Result<Option<BootstrapState>> {
            Ok(None)
        }

        async fn initialize_bootstrap(&self, _state: BootstrapState) -> anyhow::Result<()> {
            Err(
                std::io::Error::new(ErrorKind::AlreadyExists, "bootstrap already initialized")
                    .into(),
            )
        }
    }

    #[tokio::test]
    async fn ask_completions_returns_provider_response_for_known_repo_scope() {
        let app = test_app_with_config(AppConfig {
            llm_provider: Some("stub".into()),
            llm_model: Some("stub-model".into()),
            ..AppConfig::default()
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/ask/completions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&AskCompletionRequest {
                            prompt: " where is build_router implemented? ".into(),
                            system_prompt: Some("answer briefly".into()),
                            repo_scope: vec![" repo_sourcebot_rewrite ".into()],
                            thread_id: None,
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let payload: AskCompletionResponseBody = read_json(response).await;
        assert_eq!(payload.provider, "stub");
        assert_eq!(payload.model.as_deref(), Some("stub-model"));
        assert!(payload
            .answer
            .contains("where is build_router implemented?"));
    }

    #[tokio::test]
    async fn ask_completions_persists_new_repo_scoped_threads() {
        let ask_threads = Arc::new(InMemoryAskThreadStore::new());
        let config = AppConfig {
            llm_provider: Some("stub".into()),
            llm_model: Some("stub-model".into()),
            ..AppConfig::default()
        };
        let app = build_router(
            config.clone(),
            Arc::new(InMemoryCatalogStore::seeded()),
            build_bootstrap_store(config.bootstrap_state_path.clone()),
            build_local_session_store(config.local_session_state_path.clone()),
            build_browse_store(),
            build_commit_store(),
            build_search_store(),
            ask_threads.clone(),
        );

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/ask/completions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&AskCompletionRequest {
                            prompt: "where is build_router implemented?".into(),
                            system_prompt: Some("answer briefly".into()),
                            repo_scope: vec!["repo_sourcebot_rewrite".into()],
                            thread_id: None,
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let threads = ask_threads
            .list_threads_for_user(DEFAULT_ASK_USER_ID)
            .await
            .unwrap();
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].title, "where is build_router implemented?");
        assert_eq!(threads[0].repo_scope, vec!["repo_sourcebot_rewrite"]);

        let thread = ask_threads
            .get_thread_for_user(DEFAULT_ASK_USER_ID, &threads[0].id)
            .await
            .unwrap()
            .expect("new ask completion should create a thread");
        assert_eq!(thread.created_at, thread.updated_at);
        assert!(OffsetDateTime::parse(&thread.created_at, &Rfc3339).is_ok());
        assert_eq!(thread.messages.len(), 2);
        assert_eq!(thread.messages[0].role, AskMessageRole::User);
        assert_eq!(
            thread.messages[0].content,
            "where is build_router implemented?"
        );
        assert_eq!(thread.messages[1].role, AskMessageRole::Assistant);
        assert!(thread.messages[1]
            .content
            .contains("where is build_router implemented?"));
    }

    #[tokio::test]
    async fn ask_completions_appends_to_existing_repo_scoped_thread_when_thread_id_is_supplied() {
        let ask_threads = Arc::new(InMemoryAskThreadStore::new());
        let existing_thread = AskThread {
            id: "thread_existing".into(),
            session_id: "session_existing".into(),
            user_id: DEFAULT_ASK_USER_ID.into(),
            title: "existing thread".into(),
            repo_scope: vec!["repo_sourcebot_rewrite".into()],
            visibility: AskThreadVisibility::Private,
            created_at: "2026-04-16T08:00:00Z".into(),
            updated_at: "2026-04-16T08:00:00Z".into(),
            messages: vec![
                AskMessage {
                    id: "msg_existing_user".into(),
                    role: AskMessageRole::User,
                    content: "original prompt".into(),
                    citations: Vec::new(),
                },
                AskMessage {
                    id: "msg_existing_assistant".into(),
                    role: AskMessageRole::Assistant,
                    content: "original answer".into(),
                    citations: Vec::new(),
                },
            ],
        };
        ask_threads
            .create_thread(existing_thread.clone())
            .await
            .unwrap();

        let config = AppConfig {
            llm_provider: Some("stub".into()),
            llm_model: Some("stub-model".into()),
            ..AppConfig::default()
        };
        let app = build_router(
            config.clone(),
            Arc::new(InMemoryCatalogStore::seeded()),
            build_bootstrap_store(config.bootstrap_state_path.clone()),
            build_local_session_store(config.local_session_state_path.clone()),
            build_browse_store(),
            build_commit_store(),
            build_search_store(),
            ask_threads.clone(),
        );

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/ask/completions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&AskCompletionRequest {
                            prompt: "where is build_router implemented?".into(),
                            system_prompt: Some("answer briefly".into()),
                            repo_scope: vec!["repo_sourcebot_rewrite".into()],
                            thread_id: Some(existing_thread.id.clone()),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let threads = ask_threads
            .list_threads_for_user(DEFAULT_ASK_USER_ID)
            .await
            .unwrap();
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].id, existing_thread.id);

        let thread = ask_threads
            .get_thread_for_user(DEFAULT_ASK_USER_ID, &existing_thread.id)
            .await
            .unwrap()
            .expect("existing thread should remain addressable");
        assert_eq!(thread.messages.len(), 4);
        assert_eq!(thread.messages[0].id, "msg_existing_user");
        assert_eq!(thread.messages[0].content, "original prompt");
        assert_eq!(thread.messages[1].id, "msg_existing_assistant");
        assert_eq!(thread.messages[1].content, "original answer");
        assert_eq!(thread.messages[2].role, AskMessageRole::User);
        assert_eq!(
            thread.messages[2].content,
            "where is build_router implemented?"
        );
        assert!(thread.messages[2].citations.is_empty());
        assert_eq!(thread.messages[3].role, AskMessageRole::Assistant);
        assert!(thread.messages[3]
            .content
            .contains("where is build_router implemented?"));
        assert!(thread.messages[3].citations.is_empty());
        assert_ne!(thread.updated_at, "2026-04-16T08:00:00Z");
        assert!(OffsetDateTime::parse(&thread.updated_at, &Rfc3339).is_ok());
    }

    #[tokio::test]
    async fn ask_completions_returns_bad_request_for_empty_repo_scope() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/ask/completions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&AskCompletionRequest {
                            prompt: "where is build_router implemented?".into(),
                            system_prompt: None,
                            repo_scope: vec!["   ".into()],
                            thread_id: None,
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn ask_completions_returns_bad_request_for_unknown_repo_scope() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/ask/completions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&AskCompletionRequest {
                            prompt: "where is build_router implemented?".into(),
                            system_prompt: None,
                            repo_scope: vec![
                                "repo_sourcebot_rewrite".into(),
                                "repo_missing".into(),
                            ],
                            thread_id: None,
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn ask_completions_returns_bad_request_for_empty_prompt() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/ask/completions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&AskCompletionRequest {
                            prompt: "   ".into(),
                            system_prompt: None,
                            repo_scope: vec!["repo_sourcebot_rewrite".into()],
                            thread_id: None,
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
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
    async fn bootstrap_status_returns_required_when_state_file_is_missing() {
        let app = test_app_with_config(AppConfig {
            bootstrap_state_path: unique_test_path("missing").display().to_string(),
            ..AppConfig::default()
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/bootstrap")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            read_json::<BootstrapStatusResponse>(response).await,
            BootstrapStatusResponse {
                bootstrap_required: true,
            }
        );
    }

    #[tokio::test]
    async fn bootstrap_status_returns_not_required_after_initialization_state_exists() {
        let bootstrap_state_path = unique_test_path("present");
        fs::write(
            &bootstrap_state_path,
            serde_json::to_vec(&BootstrapStateResponse {
                initialized_at: "2026-04-16T17:00:00Z".into(),
                admin_email: "admin@example.com".into(),
                admin_name: "Admin User".into(),
                password_hash: "$argon2id$example".into(),
            })
            .unwrap(),
        )
        .unwrap();
        let app = test_app_with_config(AppConfig {
            bootstrap_state_path: bootstrap_state_path.display().to_string(),
            ..AppConfig::default()
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/bootstrap")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            read_json::<BootstrapStatusResponse>(response).await,
            BootstrapStatusResponse {
                bootstrap_required: false,
            }
        );

        fs::remove_file(bootstrap_state_path).unwrap();
    }

    #[tokio::test]
    async fn bootstrap_create_persists_admin_state_and_closes_bootstrap() {
        let bootstrap_state_path = unique_test_path("create");
        let app = test_app_with_config(AppConfig {
            bootstrap_state_path: bootstrap_state_path.display().to_string(),
            ..AppConfig::default()
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/bootstrap")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&BootstrapCreateRequest {
                            email: "admin@example.com".into(),
                            name: "Admin User".into(),
                            password: "correct horse battery staple".into(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(
            read_json::<BootstrapStatusResponse>(response).await,
            BootstrapStatusResponse {
                bootstrap_required: false,
            }
        );

        let persisted: BootstrapStateResponse =
            serde_json::from_slice(&fs::read(&bootstrap_state_path).unwrap()).unwrap();
        assert_eq!(persisted.admin_email, "admin@example.com");
        assert_eq!(persisted.admin_name, "Admin User");
        assert_ne!(persisted.password_hash, "correct horse battery staple");
        assert!(persisted.password_hash.starts_with("$argon2"));
        assert!(OffsetDateTime::parse(&persisted.initialized_at, &Rfc3339).is_ok());

        let status_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/bootstrap")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status_response.status(), StatusCode::OK);
        assert_eq!(
            read_json::<BootstrapStatusResponse>(status_response).await,
            BootstrapStatusResponse {
                bootstrap_required: false,
            }
        );

        fs::remove_file(bootstrap_state_path).unwrap();
    }

    #[tokio::test]
    async fn bootstrap_create_returns_conflict_on_second_post_after_initialization() {
        let bootstrap_state_path = unique_test_path("conflict");
        let app = test_app_with_config(AppConfig {
            bootstrap_state_path: bootstrap_state_path.display().to_string(),
            ..AppConfig::default()
        });

        let first_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/bootstrap")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&BootstrapCreateRequest {
                            email: "admin@example.com".into(),
                            name: "Admin User".into(),
                            password: "correct horse battery staple".into(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(first_response.status(), StatusCode::CREATED);

        let second_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/bootstrap")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&BootstrapCreateRequest {
                            email: "admin@example.com".into(),
                            name: "Admin User".into(),
                            password: "correct horse battery staple".into(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(second_response.status(), StatusCode::CONFLICT);

        fs::remove_file(bootstrap_state_path).unwrap();
    }

    #[tokio::test]
    async fn bootstrap_create_returns_conflict_when_initialize_races_with_existing_state() {
        let app = build_router(
            AppConfig::default(),
            Arc::new(InMemoryCatalogStore::seeded()),
            Arc::new(AlreadyInitializedBootstrapStore),
            build_local_session_store(unique_test_path("already-initialized-sessions")),
            build_browse_store(),
            build_commit_store(),
            build_search_store(),
            build_ask_thread_store(),
        );

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/bootstrap")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&BootstrapCreateRequest {
                            email: "admin@example.com".into(),
                            name: "Admin User".into(),
                            password: "correct horse battery staple".into(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn login_returns_created_session_and_persists_hashed_secret_for_bootstrap_admin() {
        let bootstrap_state_path = unique_test_path("login-bootstrap");
        let local_session_state_path = unique_test_path("login-sessions");
        let password = "correct horse battery staple";
        let password_hash = Argon2::default()
            .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
            .unwrap()
            .to_string();
        fs::write(
            &bootstrap_state_path,
            serde_json::to_vec(&BootstrapStateResponse {
                initialized_at: "2026-04-16T17:00:00Z".into(),
                admin_email: "admin@example.com".into(),
                admin_name: "Admin User".into(),
                password_hash,
            })
            .unwrap(),
        )
        .unwrap();

        let app = test_app_with_config(AppConfig {
            bootstrap_state_path: bootstrap_state_path.display().to_string(),
            local_session_state_path: local_session_state_path.display().to_string(),
            ..AppConfig::default()
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&LoginRequest {
                            email: " admin@example.com ".into(),
                            password: password.into(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        let payload: LoginResponseBody = read_json(response).await;
        assert!(payload.session_id.starts_with("local_session_"));
        assert!(payload.session_id.len() > "local_session_".len() + 12);
        assert!(!payload.session_secret.is_empty());
        assert_eq!(payload.user_id, "local_user_bootstrap_admin");
        assert!(OffsetDateTime::parse(&payload.created_at, &Rfc3339).is_ok());

        let persisted: sourcebot_models::LocalSessionState =
            serde_json::from_slice(&fs::read(&local_session_state_path).unwrap()).unwrap();
        assert_eq!(persisted.sessions.len(), 1);
        let session = &persisted.sessions[0];
        assert_eq!(session.id, payload.session_id);
        assert_eq!(session.user_id, payload.user_id);
        assert_eq!(session.created_at, payload.created_at);
        assert_ne!(session.secret_hash, payload.session_secret);
        let persisted_secret_hash = PasswordHash::new(&session.secret_hash).unwrap();
        assert!(Argon2::default()
            .verify_password(payload.session_secret.as_bytes(), &persisted_secret_hash)
            .is_ok());

        fs::remove_file(bootstrap_state_path).unwrap();
        fs::remove_file(local_session_state_path).unwrap();
    }

    #[tokio::test]
    async fn login_preserves_password_whitespace_when_verifying_bootstrap_admin() {
        let bootstrap_state_path = unique_test_path("login-whitespace-bootstrap");
        let local_session_state_path = unique_test_path("login-whitespace-sessions");
        let password = "  correct horse battery staple  ";
        let password_hash = Argon2::default()
            .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
            .unwrap()
            .to_string();
        fs::write(
            &bootstrap_state_path,
            serde_json::to_vec(&BootstrapStateResponse {
                initialized_at: "2026-04-16T17:00:00Z".into(),
                admin_email: "admin@example.com".into(),
                admin_name: "Admin User".into(),
                password_hash,
            })
            .unwrap(),
        )
        .unwrap();

        let app = test_app_with_config(AppConfig {
            bootstrap_state_path: bootstrap_state_path.display().to_string(),
            local_session_state_path: local_session_state_path.display().to_string(),
            ..AppConfig::default()
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&LoginRequest {
                            email: "admin@example.com".into(),
                            password: password.into(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        fs::remove_file(bootstrap_state_path).unwrap();
        fs::remove_file(local_session_state_path).unwrap();
    }

    #[tokio::test]
    async fn login_rejects_invalid_password_without_creating_session() {
        let bootstrap_state_path = unique_test_path("login-invalid-password-bootstrap");
        let local_session_state_path = unique_test_path("login-invalid-password-sessions");
        let password_hash = Argon2::default()
            .hash_password(
                b"correct horse battery staple",
                &SaltString::generate(&mut OsRng),
            )
            .unwrap()
            .to_string();
        fs::write(
            &bootstrap_state_path,
            serde_json::to_vec(&BootstrapStateResponse {
                initialized_at: "2026-04-16T17:00:00Z".into(),
                admin_email: "admin@example.com".into(),
                admin_name: "Admin User".into(),
                password_hash,
            })
            .unwrap(),
        )
        .unwrap();

        let app = test_app_with_config(AppConfig {
            bootstrap_state_path: bootstrap_state_path.display().to_string(),
            local_session_state_path: local_session_state_path.display().to_string(),
            ..AppConfig::default()
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&LoginRequest {
                            email: "admin@example.com".into(),
                            password: "wrong password".into(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(!local_session_state_path.is_file());

        fs::remove_file(bootstrap_state_path).unwrap();
    }

    #[tokio::test]
    async fn login_returns_conflict_when_bootstrap_is_still_required() {
        let local_session_state_path = unique_test_path("login-bootstrap-required-sessions");
        let app = test_app_with_config(AppConfig {
            bootstrap_state_path: unique_test_path("login-bootstrap-required-bootstrap")
                .display()
                .to_string(),
            local_session_state_path: local_session_state_path.display().to_string(),
            ..AppConfig::default()
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&LoginRequest {
                            email: "admin@example.com".into(),
                            password: "correct horse battery staple".into(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert!(!local_session_state_path.is_file());
    }

    #[tokio::test]
    async fn auth_me_returns_bootstrap_admin_for_valid_bearer_local_session() {
        let bootstrap_state_path = unique_test_path("auth-me-bootstrap");
        let local_session_state_path = unique_test_path("auth-me-sessions");
        let password = "correct horse battery staple";
        let password_hash = Argon2::default()
            .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
            .unwrap()
            .to_string();
        fs::write(
            &bootstrap_state_path,
            serde_json::to_vec(&BootstrapStateResponse {
                initialized_at: "2026-04-16T17:00:00Z".into(),
                admin_email: "admin@example.com".into(),
                admin_name: "Admin User".into(),
                password_hash,
            })
            .unwrap(),
        )
        .unwrap();

        let app = test_app_with_config(AppConfig {
            bootstrap_state_path: bootstrap_state_path.display().to_string(),
            local_session_state_path: local_session_state_path.display().to_string(),
            ..AppConfig::default()
        });

        let login_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&LoginRequest {
                            email: "admin@example.com".into(),
                            password: password.into(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(login_response.status(), StatusCode::CREATED);
        let login_payload: LoginResponseBody = read_json(login_response).await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/me")
                    .header(
                        "authorization",
                        format!(
                            "Bearer {}:{}",
                            login_payload.session_id, login_payload.session_secret
                        ),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let payload: AuthMeResponseBody = read_json(response).await;
        assert_eq!(payload.user_id, "local_user_bootstrap_admin");
        assert_eq!(payload.email, "admin@example.com");
        assert_eq!(payload.name, "Admin User");
        assert_eq!(payload.session_id, login_payload.session_id);
        assert!(OffsetDateTime::parse(&payload.created_at, &Rfc3339).is_ok());

        fs::remove_file(bootstrap_state_path).unwrap();
        fs::remove_file(local_session_state_path).unwrap();
    }

    #[tokio::test]
    async fn auth_me_returns_401_without_authorization_header() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/me")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn auth_me_returns_401_for_malformed_bearer_token() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/me")
                    .header("authorization", "Bearer not-a-session-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn auth_me_returns_401_for_invalid_session_secret() {
        let bootstrap_state_path = unique_test_path("auth-me-invalid-secret-bootstrap");
        let local_session_state_path = unique_test_path("auth-me-invalid-secret-sessions");
        let password = "correct horse battery staple";
        let password_hash = Argon2::default()
            .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
            .unwrap()
            .to_string();
        fs::write(
            &bootstrap_state_path,
            serde_json::to_vec(&BootstrapStateResponse {
                initialized_at: "2026-04-16T17:00:00Z".into(),
                admin_email: "admin@example.com".into(),
                admin_name: "Admin User".into(),
                password_hash,
            })
            .unwrap(),
        )
        .unwrap();

        let app = test_app_with_config(AppConfig {
            bootstrap_state_path: bootstrap_state_path.display().to_string(),
            local_session_state_path: local_session_state_path.display().to_string(),
            ..AppConfig::default()
        });

        let login_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&LoginRequest {
                            email: "admin@example.com".into(),
                            password: password.into(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(login_response.status(), StatusCode::CREATED);
        let login_payload: LoginResponseBody = read_json(login_response).await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/me")
                    .header(
                        "authorization",
                        format!("Bearer {}:wrong-secret", login_payload.session_id),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        fs::remove_file(bootstrap_state_path).unwrap();
        fs::remove_file(local_session_state_path).unwrap();
    }

    #[tokio::test]
    async fn auth_me_returns_401_when_bootstrap_state_is_missing() {
        let bootstrap_state_path = unique_test_path("auth-me-missing-bootstrap");
        let local_session_state_path = unique_test_path("auth-me-missing-bootstrap-sessions");
        let session_secret = "session-secret";
        let secret_hash = Argon2::default()
            .hash_password(session_secret.as_bytes(), &SaltString::generate(&mut OsRng))
            .unwrap()
            .to_string();
        fs::write(
            &local_session_state_path,
            serde_json::to_vec(&sourcebot_models::LocalSessionState {
                sessions: vec![LocalSession {
                    id: "local_session_test".into(),
                    user_id: LOCAL_BOOTSTRAP_ADMIN_USER_ID.into(),
                    secret_hash,
                    created_at: "2026-04-16T18:00:00Z".into(),
                }],
            })
            .unwrap(),
        )
        .unwrap();

        let app = test_app_with_config(AppConfig {
            bootstrap_state_path: bootstrap_state_path.display().to_string(),
            local_session_state_path: local_session_state_path.display().to_string(),
            ..AppConfig::default()
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/me")
                    .header(
                        "authorization",
                        format!("Bearer local_session_test:{session_secret}"),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        fs::remove_file(local_session_state_path).unwrap();
    }

    #[tokio::test]
    async fn logout_revokes_only_the_current_bearer_session() {
        let bootstrap_state_path = unique_test_path("logout-bootstrap");
        let local_session_state_path = unique_test_path("logout-sessions");
        let password = "correct horse battery staple";
        let password_hash = Argon2::default()
            .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
            .unwrap()
            .to_string();
        fs::write(
            &bootstrap_state_path,
            serde_json::to_vec(&BootstrapStateResponse {
                initialized_at: "2026-04-16T17:00:00Z".into(),
                admin_email: "admin@example.com".into(),
                admin_name: "Admin User".into(),
                password_hash,
            })
            .unwrap(),
        )
        .unwrap();

        let app = test_app_with_config(AppConfig {
            bootstrap_state_path: bootstrap_state_path.display().to_string(),
            local_session_state_path: local_session_state_path.display().to_string(),
            ..AppConfig::default()
        });

        let first_login_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&LoginRequest {
                            email: "admin@example.com".into(),
                            password: password.into(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(first_login_response.status(), StatusCode::CREATED);
        let first_login_payload: LoginResponseBody = read_json(first_login_response).await;

        let second_login_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&LoginRequest {
                            email: "admin@example.com".into(),
                            password: password.into(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(second_login_response.status(), StatusCode::CREATED);
        let second_login_payload: LoginResponseBody = read_json(second_login_response).await;

        let logout_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/logout")
                    .header(
                        "authorization",
                        format!(
                            "Bearer {}:{}",
                            first_login_payload.session_id, first_login_payload.session_secret
                        ),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(logout_response.status(), StatusCode::NO_CONTENT);
        let logout_body = to_bytes(logout_response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(logout_body.is_empty());

        let revoked_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/me")
                    .header(
                        "authorization",
                        format!(
                            "Bearer {}:{}",
                            first_login_payload.session_id, first_login_payload.session_secret
                        ),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(revoked_response.status(), StatusCode::UNAUTHORIZED);

        let retained_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/me")
                    .header(
                        "authorization",
                        format!(
                            "Bearer {}:{}",
                            second_login_payload.session_id, second_login_payload.session_secret
                        ),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(retained_response.status(), StatusCode::OK);

        fs::remove_file(bootstrap_state_path).unwrap();
        fs::remove_file(local_session_state_path).unwrap();
    }

    #[tokio::test]
    async fn logout_returns_401_for_missing_or_malformed_authorization() {
        let missing_auth_response = test_app()
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/logout")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing_auth_response.status(), StatusCode::UNAUTHORIZED);

        let malformed_auth_response = test_app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/logout")
                    .header("authorization", "Bearer not-a-session-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(malformed_auth_response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn revoke_explicitly_targets_requested_local_session_and_fails_closed() {
        let bootstrap_state_path = unique_test_path("revoke-bootstrap");
        let local_session_state_path = unique_test_path("revoke-sessions");
        let password = "correct horse battery staple";
        let password_hash = Argon2::default()
            .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
            .unwrap()
            .to_string();
        fs::write(
            &bootstrap_state_path,
            serde_json::to_vec(&BootstrapStateResponse {
                initialized_at: "2026-04-16T17:00:00Z".into(),
                admin_email: "admin@example.com".into(),
                admin_name: "Admin User".into(),
                password_hash,
            })
            .unwrap(),
        )
        .unwrap();

        let app = test_app_with_config(AppConfig {
            bootstrap_state_path: bootstrap_state_path.display().to_string(),
            local_session_state_path: local_session_state_path.display().to_string(),
            ..AppConfig::default()
        });

        let auth_login_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&LoginRequest {
                            email: "admin@example.com".into(),
                            password: password.into(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(auth_login_response.status(), StatusCode::CREATED);
        let auth_login_payload: LoginResponseBody = read_json(auth_login_response).await;

        let target_login_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&LoginRequest {
                            email: "admin@example.com".into(),
                            password: password.into(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(target_login_response.status(), StatusCode::CREATED);
        let target_login_payload: LoginResponseBody = read_json(target_login_response).await;

        let missing_payload_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/revoke")
                    .header(
                        "authorization",
                        format!(
                            "Bearer {}:{}",
                            auth_login_payload.session_id, auth_login_payload.session_secret
                        ),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(b"{}".to_vec()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing_payload_response.status(), StatusCode::BAD_REQUEST);

        let blank_payload_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/revoke")
                    .header(
                        "authorization",
                        format!(
                            "Bearer {}:{}",
                            auth_login_payload.session_id, auth_login_payload.session_secret
                        ),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&RevokeRequest {
                            session_id: "   ".into(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(blank_payload_response.status(), StatusCode::BAD_REQUEST);

        let success_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/revoke")
                    .header(
                        "authorization",
                        format!(
                            "Bearer {}:{}",
                            auth_login_payload.session_id, auth_login_payload.session_secret
                        ),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&RevokeRequest {
                            session_id: format!("  {}  ", target_login_payload.session_id),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(success_response.status(), StatusCode::NO_CONTENT);
        assert!(to_bytes(success_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .is_empty());

        let target_me_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/me")
                    .header(
                        "authorization",
                        format!(
                            "Bearer {}:{}",
                            target_login_payload.session_id, target_login_payload.session_secret
                        ),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(target_me_response.status(), StatusCode::UNAUTHORIZED);

        let auth_me_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/me")
                    .header(
                        "authorization",
                        format!(
                            "Bearer {}:{}",
                            auth_login_payload.session_id, auth_login_payload.session_secret
                        ),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(auth_me_response.status(), StatusCode::OK);

        let missing_target_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/revoke")
                    .header(
                        "authorization",
                        format!(
                            "Bearer {}:{}",
                            auth_login_payload.session_id, auth_login_payload.session_secret
                        ),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&RevokeRequest {
                            session_id: "missing-session".into(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing_target_response.status(), StatusCode::UNAUTHORIZED);

        let mut persisted_state: LocalSessionState =
            serde_json::from_slice(&fs::read(&local_session_state_path).unwrap()).unwrap();
        persisted_state.sessions.push(LocalSession {
            id: "foreign-session".into(),
            user_id: "another-user".into(),
            secret_hash: "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHQ$ZXhhbXBsZWhhc2g".into(),
            created_at: "2026-04-16T17:05:00Z".into(),
        });
        persisted_state.sessions.push(LocalSession {
            id: "malformed-session".into(),
            user_id: LOCAL_BOOTSTRAP_ADMIN_USER_ID.into(),
            secret_hash: "not-a-password-hash".into(),
            created_at: "2026-04-16T17:06:00Z".into(),
        });
        fs::write(
            &local_session_state_path,
            serde_json::to_vec(&persisted_state).unwrap(),
        )
        .unwrap();

        let foreign_target_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/revoke")
                    .header(
                        "authorization",
                        format!(
                            "Bearer {}:{}",
                            auth_login_payload.session_id, auth_login_payload.session_secret
                        ),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&RevokeRequest {
                            session_id: "foreign-session".into(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(foreign_target_response.status(), StatusCode::UNAUTHORIZED);

        let malformed_target_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/revoke")
                    .header(
                        "authorization",
                        format!(
                            "Bearer {}:{}",
                            auth_login_payload.session_id, auth_login_payload.session_secret
                        ),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&RevokeRequest {
                            session_id: "malformed-session".into(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(malformed_target_response.status(), StatusCode::UNAUTHORIZED);

        let already_revoked_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/revoke")
                    .header(
                        "authorization",
                        format!(
                            "Bearer {}:{}",
                            auth_login_payload.session_id, auth_login_payload.session_secret
                        ),
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&RevokeRequest {
                            session_id: target_login_payload.session_id,
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(already_revoked_response.status(), StatusCode::UNAUTHORIZED);

        fs::remove_file(bootstrap_state_path).unwrap();
        fs::remove_file(local_session_state_path).unwrap();
    }

    #[tokio::test]
    async fn config_endpoint_hides_database_url_value() {
        let app = build_router(
            AppConfig {
                service_name: "sourcebot-api".into(),
                bind_addr: "127.0.0.1:3000".into(),
                database_url: Some("postgres://secret@localhost/sourcebot".into()),
                bootstrap_state_path: unique_test_path("config").display().to_string(),
                local_session_state_path: unique_test_path("config-local-sessions")
                    .display()
                    .to_string(),
                organization_state_path: unique_test_path("config-organizations")
                    .display()
                    .to_string(),
                llm_provider: Some("stub".into()),
                llm_model: Some("stub-model".into()),
                llm_api_base: Some("https://llm.invalid".into()),
                llm_api_key: Some("super-secret".into()),
            },
            Arc::new(InMemoryCatalogStore::seeded()),
            build_bootstrap_store(unique_test_path("config-store")),
            build_local_session_store(unique_test_path("config-local-sessions")),
            build_browse_store(),
            build_commit_store(),
            build_search_store(),
            build_ask_thread_store(),
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
        let payload: PublicAppConfig = read_json(response).await;
        assert_eq!(payload.service_name, "sourcebot-api");
        assert_eq!(payload.llm_provider.as_deref(), Some("stub"));
        assert_eq!(payload.llm_model.as_deref(), Some("stub-model"));
        assert!(payload.has_database_url);
        assert!(payload.has_llm_api_key);
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
    async fn repo_blob_returns_requested_revision_contents() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri(
                        "/api/v1/repos/repo_sourcebot_rewrite/blob?path=crates/api/src/main.rs&revision=3864b25",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let payload: BlobResponse = read_json(response).await;
        assert_eq!(payload.path, "crates/api/src/main.rs");
        assert!(!payload
            .content
            .contains("async fn get_repository_references("));
    }

    #[tokio::test]
    async fn repo_blob_rejects_parent_directory_traversal_with_bad_request() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/repos/repo_sourcebot_rewrite/blob?path=..")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
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
    async fn search_requires_an_authenticated_session() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/search?q=build_router&repo_id=repo_sourcebot_rewrite")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn search_requires_authentication_before_validating_query_shape() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/search?q=&repo_id=repo_sourcebot_rewrite")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn search_returns_bad_request_for_empty_query() {
        let organization_state_path = unique_test_path("search-empty-query-orgs");
        write_organization_state_fixture(
            &organization_state_path,
            LOCAL_BOOTSTRAP_ADMIN_USER_ID,
            &["repo_sourcebot_rewrite"],
        );
        let app = test_app_with_config(AppConfig {
            organization_state_path: organization_state_path.display().to_string(),
            bootstrap_state_path: unique_test_path("search-empty-query-bootstrap")
                .display()
                .to_string(),
            local_session_state_path: unique_test_path("search-empty-query-sessions")
                .display()
                .to_string(),
            ..AppConfig::default()
        });
        let authorization = bootstrap_and_login(&app).await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/search?q=&repo_id=repo_sourcebot_rewrite")
                    .header(header::AUTHORIZATION, authorization)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn search_returns_matches_only_for_visible_repositories() {
        let organization_state_path = unique_test_path("search-visible-repos-orgs");
        write_organization_state_fixture(
            &organization_state_path,
            LOCAL_BOOTSTRAP_ADMIN_USER_ID,
            &["repo_sourcebot_rewrite"],
        );
        let app = test_app_with_config(AppConfig {
            organization_state_path: organization_state_path.display().to_string(),
            bootstrap_state_path: unique_test_path("search-visible-repos-bootstrap")
                .display()
                .to_string(),
            local_session_state_path: unique_test_path("search-visible-repos-sessions")
                .display()
                .to_string(),
            ..AppConfig::default()
        });
        let authorization = bootstrap_and_login(&app).await;

        let visible_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/search?q=build_router&repo_id=repo_sourcebot_rewrite")
                    .header(header::AUTHORIZATION, authorization.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(visible_response.status(), StatusCode::OK);

        let payload: SearchResponse = read_json(visible_response).await;
        assert_eq!(payload.query, "build_router");
        assert_eq!(payload.repo_id.as_deref(), Some("repo_sourcebot_rewrite"));
        assert!(!payload.results.is_empty());
        assert!(payload
            .results
            .iter()
            .all(|result| result.repo_id == "repo_sourcebot_rewrite"));
        assert!(payload.results.iter().any(|result| {
            result.repo_id == "repo_sourcebot_rewrite"
                && result.path == "crates/api/src/main.rs"
                && result.line.contains("build_router")
                && result.line_number > 0
        }));

        let hidden_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/search?q=build_router&repo_id=repo_not_visible")
                    .header(header::AUTHORIZATION, authorization)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(hidden_response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn search_allows_non_bootstrap_local_sessions_with_visible_repo_permissions() {
        let organization_state_path = unique_test_path("search-non-bootstrap-orgs");
        let local_session_state_path = unique_test_path("search-non-bootstrap-sessions");
        let user_id = "user_viewer";
        write_organization_state_fixture(
            &organization_state_path,
            user_id,
            &["repo_sourcebot_rewrite"],
        );
        let authorization =
            seed_local_session(&local_session_state_path.display().to_string(), user_id).await;
        let app = test_app_with_config(AppConfig {
            organization_state_path: organization_state_path.display().to_string(),
            bootstrap_state_path: unique_test_path("search-non-bootstrap-bootstrap")
                .display()
                .to_string(),
            local_session_state_path: local_session_state_path.display().to_string(),
            ..AppConfig::default()
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/search?q=build_router&repo_id=repo_sourcebot_rewrite")
                    .header(header::AUTHORIZATION, authorization)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let payload: SearchResponse = read_json(response).await;
        assert!(!payload.results.is_empty());
        assert!(payload
            .results
            .iter()
            .all(|result| result.repo_id == "repo_sourcebot_rewrite"));
    }

    #[tokio::test]
    async fn definitions_returns_bad_request_for_missing_required_query_parameters() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/repos/repo_sourcebot_rewrite/definitions?path=crates/api/src/main.rs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn definitions_returns_not_found_for_unknown_repo_or_path() {
        let unknown_repo = test_app()
            .oneshot(
                Request::builder()
                    .uri(
                        "/api/v1/repos/missing/definitions?path=crates/api/src/main.rs&symbol=build_router",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unknown_repo.status(), StatusCode::NOT_FOUND);

        let unknown_path = test_app()
            .oneshot(
                Request::builder()
                    .uri(
                        "/api/v1/repos/repo_sourcebot_rewrite/definitions?path=missing.rs&symbol=build_router",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unknown_path.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn definitions_returns_supported_rust_definitions_and_echoes_revision() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri(
                        "/api/v1/repos/repo_sourcebot_rewrite/definitions?path=crates/api/src/main.rs&symbol=build_router&revision=HEAD",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let payload: DefinitionsResponse = read_json(response).await;
        match payload {
            DefinitionsResponse::Supported {
                repo_id,
                path,
                revision,
                symbol,
                definitions,
            } => {
                assert_eq!(repo_id, "repo_sourcebot_rewrite");
                assert_eq!(path, "crates/api/src/main.rs");
                assert_eq!(revision.as_deref(), Some("HEAD"));
                assert_eq!(symbol, "build_router");
                assert!(!definitions.is_empty());
                assert_eq!(definitions[0].name, "build_router");
                assert_eq!(definitions[0].path, "crates/api/src/main.rs");
                assert!(definitions[0].range.start_line > 0);
                assert!(definitions[0].range.end_line >= definitions[0].range.start_line);
                assert_eq!(
                    definitions[0].browse_url,
                    format!(
                        "/api/v1/repos/repo_sourcebot_rewrite/blob?path=crates%2Fapi%2Fsrc%2Fmain.rs&revision=HEAD#L{}",
                        definitions[0].range.start_line
                    )
                );
            }
            DefinitionsResponse::Unsupported { .. } => {
                panic!("expected supported definitions response")
            }
        }
    }

    #[tokio::test]
    async fn definitions_default_revision_resolves_to_head_in_response_and_browse_url() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri(
                        "/api/v1/repos/repo_sourcebot_rewrite/definitions?path=crates/api/src/main.rs&symbol=build_router",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let payload: DefinitionsResponse = read_json(response).await;
        match payload {
            DefinitionsResponse::Supported {
                revision,
                definitions,
                ..
            } => {
                assert_eq!(revision.as_deref(), Some("HEAD"));
                assert!(!definitions.is_empty());
                assert!(definitions
                    .iter()
                    .all(|definition| definition.browse_url.contains("revision=HEAD")));
            }
            DefinitionsResponse::Unsupported { .. } => {
                panic!("expected supported definitions response")
            }
        }
    }

    #[tokio::test]
    async fn definitions_reject_parent_directory_traversal_with_bad_request() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri(
                        "/api/v1/repos/repo_sourcebot_rewrite/definitions?path=../README.md&symbol=sourcebot",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn definitions_use_requested_revision_for_lookup() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri(
                        "/api/v1/repos/repo_sourcebot_rewrite/definitions?path=crates/api/src/main.rs&symbol=get_repository_references&revision=3864b25",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let payload: DefinitionsResponse = read_json(response).await;
        match payload {
            DefinitionsResponse::Supported {
                revision,
                definitions,
                ..
            } => {
                assert_eq!(revision.as_deref(), Some("3864b25"));
                assert!(definitions.is_empty());
            }
            DefinitionsResponse::Unsupported { .. } => {
                panic!("expected supported definitions response")
            }
        }
    }

    #[test]
    fn definition_browse_url_encodes_path_and_revision_query_values() {
        assert_eq!(
            build_definition_browse_url(
                "repo_sourcebot_rewrite",
                "dir/hello world?#.rs",
                Some("feature/test branch"),
                42,
            ),
            "/api/v1/repos/repo_sourcebot_rewrite/blob?path=dir%2Fhello%20world%3F%23.rs&revision=feature%2Ftest%20branch#L42"
        );
    }
    #[tokio::test]
    async fn definitions_returns_unsupported_capability_for_non_rust_files() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri(
                        "/api/v1/repos/repo_sourcebot_rewrite/definitions?path=README.md&symbol=sourcebot",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let payload: DefinitionsResponse = read_json(response).await;
        match payload {
            DefinitionsResponse::Unsupported {
                repo_id,
                path,
                revision,
                symbol,
                capability,
                definitions,
            } => {
                assert_eq!(repo_id, "repo_sourcebot_rewrite");
                assert_eq!(path, "README.md");
                assert_eq!(revision.as_deref(), Some("HEAD"));
                assert_eq!(symbol, "sourcebot");
                assert_eq!(
                    capability,
                    "symbol extraction is not supported for .md files"
                );
                assert!(definitions.is_empty());
            }
            DefinitionsResponse::Supported { .. } => {
                panic!("expected unsupported definitions response")
            }
        }
    }

    #[tokio::test]
    async fn references_returns_bad_request_for_missing_required_query_parameters() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/repos/repo_sourcebot_rewrite/references?path=crates/api/src/main.rs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn references_returns_not_found_for_unknown_repo_or_path() {
        let unknown_repo = test_app()
            .oneshot(
                Request::builder()
                    .uri(
                        "/api/v1/repos/missing/references?path=crates/api/src/main.rs&symbol=build_router",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unknown_repo.status(), StatusCode::NOT_FOUND);

        let unknown_path = test_app()
            .oneshot(
                Request::builder()
                    .uri(
                        "/api/v1/repos/repo_sourcebot_rewrite/references?path=missing.rs&symbol=build_router",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unknown_path.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn references_returns_unsupported_capability_for_non_rust_files() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri(
                        "/api/v1/repos/repo_sourcebot_rewrite/references?path=README.md&symbol=sourcebot",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let payload: ReferencesResponse = read_json(response).await;
        match payload {
            ReferencesResponse::Unsupported {
                repo_id,
                path,
                revision,
                symbol,
                capability,
                references,
            } => {
                assert_eq!(repo_id, "repo_sourcebot_rewrite");
                assert_eq!(path, "README.md");
                assert_eq!(revision.as_deref(), Some("HEAD"));
                assert_eq!(symbol, "sourcebot");
                assert_eq!(
                    capability,
                    "symbol extraction is not supported for .md files"
                );
                assert!(references.is_empty());
            }
            ReferencesResponse::Supported { .. } => {
                panic!("expected unsupported references response")
            }
        }
    }

    #[tokio::test]
    async fn references_return_supported_rust_hits_with_browse_urls_ordering_and_deduplication() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri(
                        "/api/v1/repos/repo_sourcebot_rewrite/references?path=crates/api/src/main.rs&symbol=build_router&revision=HEAD",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let payload: ReferencesResponse = read_json(response).await;
        match payload {
            ReferencesResponse::Supported {
                repo_id,
                path,
                revision,
                symbol,
                references,
            } => {
                assert_eq!(repo_id, "repo_sourcebot_rewrite");
                assert_eq!(path, "crates/api/src/main.rs");
                assert_eq!(revision.as_deref(), Some("HEAD"));
                assert_eq!(symbol, "build_router");
                assert!(!references.is_empty());
                assert!(references.iter().any(|reference| {
                    reference.path == "crates/api/src/main.rs"
                        && reference.line.contains("build_router")
                        && reference.line_number > 0
                        && reference.browse_url.starts_with(
                            "/api/v1/repos/repo_sourcebot_rewrite/blob?path=crates%2Fapi%2Fsrc%2Fmain.rs&revision=HEAD#L",
                        )
                }));

                let mut sorted = references
                    .iter()
                    .map(|reference| {
                        (
                            reference.path.clone(),
                            reference.line_number,
                            reference.line.clone(),
                            reference.browse_url.clone(),
                        )
                    })
                    .collect::<Vec<_>>();
                let mut deduped = sorted.clone();
                sorted.sort();
                deduped.sort();
                deduped.dedup();
                assert_eq!(
                    references
                        .iter()
                        .map(|reference| (
                            reference.path.clone(),
                            reference.line_number,
                            reference.line.clone(),
                            reference.browse_url.clone(),
                        ))
                        .collect::<Vec<_>>(),
                    sorted
                );
                assert_eq!(sorted, deduped);
            }
            ReferencesResponse::Unsupported { .. } => {
                panic!("expected supported references response")
            }
        }
    }

    #[tokio::test]
    async fn references_default_revision_resolves_to_head_in_response_and_browse_urls() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri(
                        "/api/v1/repos/repo_sourcebot_rewrite/references?path=crates/api/src/main.rs&symbol=build_router",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let payload: ReferencesResponse = read_json(response).await;
        match payload {
            ReferencesResponse::Supported {
                revision,
                references,
                ..
            } => {
                assert_eq!(revision.as_deref(), Some("HEAD"));
                assert!(!references.is_empty());
                assert!(references
                    .iter()
                    .all(|reference| reference.browse_url.contains("revision=HEAD")));
            }
            ReferencesResponse::Unsupported { .. } => {
                panic!("expected supported references response")
            }
        }
    }

    #[tokio::test]
    async fn references_use_requested_revision_for_lookup() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri(
                        "/api/v1/repos/repo_sourcebot_rewrite/references?path=crates/api/src/main.rs&symbol=get_repository_references&revision=3864b25",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let payload: ReferencesResponse = read_json(response).await;
        match payload {
            ReferencesResponse::Supported {
                revision,
                references,
                ..
            } => {
                assert_eq!(revision.as_deref(), Some("3864b25"));
                assert!(references.is_empty());
            }
            ReferencesResponse::Unsupported { .. } => {
                panic!("expected supported references response")
            }
        }
    }

    #[tokio::test]
    async fn references_reject_parent_directory_traversal_with_bad_request() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri(
                        "/api/v1/repos/repo_sourcebot_rewrite/references?path=../README.md&symbol=sourcebot",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn reference_browse_url_encodes_path_and_revision_query_values() {
        assert_eq!(
            build_reference_browse_url(
                "repo_sourcebot_rewrite",
                "dir/hello world?#.rs",
                Some("feature/test branch"),
                42,
            ),
            "/api/v1/repos/repo_sourcebot_rewrite/blob?path=dir%2Fhello%20world%3F%23.rs&revision=feature%2Ftest%20branch#L42"
        );
    }
}
