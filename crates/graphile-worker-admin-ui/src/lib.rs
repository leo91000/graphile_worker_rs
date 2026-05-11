#[cfg(not(target_arch = "wasm32"))]
use std::fmt;
#[cfg(not(target_arch = "wasm32"))]
use std::net::SocketAddr;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use axum::body::Body;
#[cfg(not(target_arch = "wasm32"))]
use axum::extract::{Path, Query, State};
#[cfg(not(target_arch = "wasm32"))]
use axum::http::header::{
    AUTHORIZATION, CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, REFERRER_POLICY,
    WWW_AUTHENTICATE, X_CONTENT_TYPE_OPTIONS,
};
#[cfg(not(target_arch = "wasm32"))]
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode};
#[cfg(not(target_arch = "wasm32"))]
use axum::middleware::{self, Next};
#[cfg(not(target_arch = "wasm32"))]
use axum::response::{Html, IntoResponse, Response};
#[cfg(not(target_arch = "wasm32"))]
use axum::routing::{get, post};
#[cfg(not(target_arch = "wasm32"))]
use axum::{Json, Router};
#[cfg(not(target_arch = "wasm32"))]
use base64::engine::general_purpose::STANDARD;
#[cfg(not(target_arch = "wasm32"))]
use base64::Engine;
use chrono::{DateTime, Utc};
#[cfg(not(target_arch = "wasm32"))]
use graphile_worker::worker_utils::{CleanupTask, RescheduleJobOptions};
#[cfg(not(target_arch = "wasm32"))]
use graphile_worker::{DbJob, Job, JobKeyMode, JobSpec, WorkerUtils};
#[cfg(not(target_arch = "wasm32"))]
use indoc::formatdoc;
#[cfg(not(target_arch = "wasm32"))]
use leptos::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(not(target_arch = "wasm32"))]
use sqlx::{PgPool, Postgres, QueryBuilder};
#[cfg(not(target_arch = "wasm32"))]
use thiserror::Error;

#[cfg(not(target_arch = "wasm32"))]
pub const ADMIN_CSS: &str = include_str!(concat!(env!("OUT_DIR"), "/admin.css"));
#[cfg(not(target_arch = "wasm32"))]
pub const ADMIN_JS: &str = include_str!(concat!(env!("OUT_DIR"), "/admin.js"));
#[cfg(not(target_arch = "wasm32"))]
pub const ADMIN_WASM_BINDGEN_JS: &str = include_str!(concat!(env!("OUT_DIR"), "/admin_ui.js"));
#[cfg(not(target_arch = "wasm32"))]
pub const ADMIN_WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/admin_ui_bg.wasm"));
#[cfg(not(target_arch = "wasm32"))]
pub const ADMIN_FAVICON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect width="64" height="64" rx="12" fill="#0e7490"/><path d="M17 22h30M17 32h30M17 42h30" stroke="#fff" stroke-width="6" stroke-linecap="round"/></svg>"##;

#[cfg(not(target_arch = "wasm32"))]
const CSRF_HEADER: &str = "x-graphile-worker-admin-csrf";

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Error)]
pub enum AdminUiError {
    #[error("admin auth without credentials is only allowed on loopback addresses")]
    InsecureNoAuth,
    #[error("invalid header name `{0}`")]
    InvalidHeaderName(String),
    #[error("failed to bind admin UI listener: {0}")]
    Bind(#[from] std::io::Error),
    #[error("failed to serve admin UI: {0}")]
    Serve(std::io::Error),
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Serialize)]
pub enum PublicAuthMode {
    Basic,
    Bearer,
    Header,
    None,
}

#[cfg(not(target_arch = "wasm32"))]
impl PublicAuthMode {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Basic => "basic",
            Self::Bearer => "bearer",
            Self::Header => "header",
            Self::None => "none",
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub enum AdminAuthConfig {
    Basic {
        username: String,
        password: String,
        generated_password: bool,
    },
    Bearer {
        token: String,
        generated_token: bool,
    },
    Header {
        header_name: HeaderName,
        token: String,
        generated_token: bool,
    },
    None,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Serialize)]
pub struct AdminAuthSummary {
    pub mode: PublicAuthMode,
    pub username: Option<String>,
    pub header_name: Option<String>,
    pub generated_secret: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl AdminAuthConfig {
    pub fn basic(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self::Basic {
            username: username.into(),
            password: password.into(),
            generated_password: false,
        }
    }

    pub fn basic_with_random_password(username: impl Into<String>) -> Self {
        Self::Basic {
            username: username.into(),
            password: generate_secret(),
            generated_password: true,
        }
    }

    pub fn bearer(token: impl Into<String>, generated_token: bool) -> Self {
        Self::Bearer {
            token: token.into(),
            generated_token,
        }
    }

    pub fn header(
        header_name: impl AsRef<str>,
        token: impl Into<String>,
        generated_token: bool,
    ) -> Result<Self, AdminUiError> {
        let header_name = HeaderName::from_bytes(header_name.as_ref().as_bytes())
            .map_err(|_| AdminUiError::InvalidHeaderName(header_name.as_ref().to_string()))?;
        Ok(Self::Header {
            header_name,
            token: token.into(),
            generated_token,
        })
    }

    pub fn summary(&self) -> AdminAuthSummary {
        match self {
            Self::Basic {
                username,
                generated_password,
                ..
            } => AdminAuthSummary {
                mode: PublicAuthMode::Basic,
                username: Some(username.clone()),
                header_name: None,
                generated_secret: *generated_password,
            },
            Self::Bearer {
                generated_token, ..
            } => AdminAuthSummary {
                mode: PublicAuthMode::Bearer,
                username: None,
                header_name: None,
                generated_secret: *generated_token,
            },
            Self::Header {
                header_name,
                generated_token,
                ..
            } => AdminAuthSummary {
                mode: PublicAuthMode::Header,
                username: None,
                header_name: Some(header_name.as_str().to_string()),
                generated_secret: *generated_token,
            },
            Self::None => AdminAuthSummary {
                mode: PublicAuthMode::None,
                username: None,
                header_name: None,
                generated_secret: false,
            },
        }
    }

    pub fn secret_for_display(&self) -> Option<&str> {
        match self {
            Self::Basic { password, .. } => Some(password),
            Self::Bearer { token, .. } | Self::Header { token, .. } => Some(token),
            Self::None => None,
        }
    }

    fn is_authorized(&self, headers: &HeaderMap) -> bool {
        match self {
            Self::None => true,
            Self::Basic {
                username, password, ..
            } => authorize_basic(headers, username, password),
            Self::Bearer { token, .. } => authorize_bearer(headers, token),
            Self::Header {
                header_name, token, ..
            } => headers
                .get(header_name)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| constant_time_eq(value.as_bytes(), token.as_bytes())),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
pub struct AdminServerConfig {
    pub pool: PgPool,
    pub utils: WorkerUtils,
    pub escaped_schema: String,
    pub schema: String,
    pub listen_addr: SocketAddr,
    pub auth: AdminAuthConfig,
    pub read_only: bool,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
struct AppState {
    pool: PgPool,
    utils: WorkerUtils,
    escaped_schema: String,
    schema: String,
    auth: AdminAuthConfig,
    csrf_token: String,
    read_only: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl AppState {
    fn from_config(config: AdminServerConfig) -> Result<Self, AdminUiError> {
        if matches!(config.auth, AdminAuthConfig::None) && !config.listen_addr.ip().is_loopback() {
            return Err(AdminUiError::InsecureNoAuth);
        }

        Ok(Self {
            pool: config.pool,
            utils: config.utils,
            escaped_schema: config.escaped_schema,
            schema: config.schema,
            auth: config.auth,
            csrf_token: generate_secret(),
            read_only: config.read_only,
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn generate_secret() -> String {
    let mut bytes = [0_u8; 24];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn build_router(config: AdminServerConfig) -> Result<Router, AdminUiError> {
    let state = Arc::new(AppState::from_config(config)?);

    let api = Router::new()
        .route("/api/session", get(session))
        .route("/api/overview", get(overview))
        .route("/api/jobs", get(jobs).post(add_job))
        .route("/api/jobs/{id}", get(job))
        .route("/api/jobs/action", post(job_action))
        .route("/api/jobs/remove-by-key", post(remove_job_by_key))
        .route("/api/maintenance", post(maintenance))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_api_auth,
        ));

    let public = Router::new()
        .route("/", get(index))
        .route("/assets/admin.css", get(css))
        .route("/assets/admin.js", get(js))
        .route("/assets/admin_ui.js", get(wasm_bindgen_js))
        .route("/assets/admin_ui_bg.wasm", get(wasm))
        .route("/favicon.ico", get(favicon));

    let app = public
        .merge(api)
        .with_state(state.clone())
        .layer(middleware::from_fn(security_headers));

    if matches!(state.auth, AdminAuthConfig::Basic { .. }) {
        Ok(app.layer(middleware::from_fn_with_state(
            state,
            require_basic_page_auth,
        )))
    } else {
        Ok(app)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn serve(config: AdminServerConfig) -> Result<(), AdminUiError> {
    let addr = config.listen_addr;
    let router = build_router(config)?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(
        address = %listener.local_addr()?,
        "Graphile Worker admin UI listening"
    );
    axum::serve(listener, router)
        .await
        .map_err(AdminUiError::Serve)
}

#[cfg(not(target_arch = "wasm32"))]
async fn index(State(state): State<Arc<AppState>>) -> Html<String> {
    let auth = state.auth.summary();
    Html(render_admin_html(&AdminUiRenderConfig {
        csrf_token: state.csrf_token.clone(),
        schema: state.schema.clone(),
        read_only: state.read_only,
        auth,
    }))
}

#[cfg(not(target_arch = "wasm32"))]
async fn css() -> impl IntoResponse {
    (
        [(
            CONTENT_TYPE,
            HeaderValue::from_static("text/css; charset=utf-8"),
        )],
        ADMIN_CSS,
    )
}

#[cfg(not(target_arch = "wasm32"))]
async fn js() -> impl IntoResponse {
    (
        [(
            CONTENT_TYPE,
            HeaderValue::from_static("application/javascript; charset=utf-8"),
        )],
        ADMIN_JS,
    )
}

#[cfg(not(target_arch = "wasm32"))]
async fn wasm_bindgen_js() -> impl IntoResponse {
    (
        [(
            CONTENT_TYPE,
            HeaderValue::from_static("application/javascript; charset=utf-8"),
        )],
        ADMIN_WASM_BINDGEN_JS,
    )
}

#[cfg(not(target_arch = "wasm32"))]
async fn wasm() -> impl IntoResponse {
    (
        [(CONTENT_TYPE, HeaderValue::from_static("application/wasm"))],
        ADMIN_WASM,
    )
}

#[cfg(not(target_arch = "wasm32"))]
async fn favicon() -> impl IntoResponse {
    (
        [(CONTENT_TYPE, HeaderValue::from_static("image/svg+xml"))],
        ADMIN_FAVICON,
    )
}

#[cfg(not(target_arch = "wasm32"))]
async fn session(State(state): State<Arc<AppState>>) -> Json<SessionResponse> {
    Json(SessionResponse {
        schema: state.schema.clone(),
        read_only: state.read_only,
        csrf_header: CSRF_HEADER.to_string(),
        auth: state.auth.summary(),
    })
}

#[cfg(not(target_arch = "wasm32"))]
async fn overview(State(state): State<Arc<AppState>>) -> Result<Json<OverviewResponse>, ApiError> {
    let stats = get_stats(&state.pool, &state.escaped_schema).await?;
    let queues = list_queues(&state.pool, &state.escaped_schema).await?;
    let workers = list_locked_workers(&state.pool, &state.escaped_schema).await?;
    Ok(Json(OverviewResponse {
        stats,
        queues,
        workers,
    }))
}

#[cfg(not(target_arch = "wasm32"))]
async fn jobs(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListJobsParams>,
) -> Result<Json<ListJobsResponse>, ApiError> {
    let jobs = list_jobs(&state.pool, &state.escaped_schema, &params).await?;
    Ok(Json(ListJobsResponse { jobs }))
}

#[cfg(not(target_arch = "wasm32"))]
async fn job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<ListedJob>, ApiError> {
    let job = get_job(&state.pool, &state.escaped_schema, id).await?;
    Ok(Json(job))
}

#[cfg(not(target_arch = "wasm32"))]
async fn add_job(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AddJobRequest>,
) -> Result<Json<JobActionResponse>, ApiError> {
    ensure_write_allowed(&state)?;
    if request.job_key_mode.is_some()
        && request
            .key
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
    {
        return Err(ApiError::bad_request(
            "key is required when job_key_mode is set",
        ));
    }

    let spec = JobSpec {
        queue_name: request.queue,
        run_at: request.run_at,
        max_attempts: request.max_attempts,
        job_key: request.key,
        job_key_mode: request.job_key_mode.map(Into::into),
        priority: request.priority,
        flags: request.flags.filter(|flags| !flags.is_empty()),
    };

    let job = state
        .utils
        .add_raw_job(&request.identifier, request.payload, spec)
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(JobActionResponse {
        message: format!("Added job {}", job.id()),
        jobs: vec![DbJobOutput::from_job(&job)],
    }))
}

#[cfg(not(target_arch = "wasm32"))]
async fn job_action(
    State(state): State<Arc<AppState>>,
    Json(request): Json<JobActionRequest>,
) -> Result<Json<JobActionResponse>, ApiError> {
    ensure_write_allowed(&state)?;
    if request.ids.is_empty() {
        return Err(ApiError::bad_request("select at least one job"));
    }

    match request.action {
        JobAction::Complete => {
            let jobs = state
                .utils
                .complete_jobs(&request.ids)
                .await
                .map_err(ApiError::internal)?;
            Ok(action_response("Completed", &jobs))
        }
        JobAction::Fail => {
            let reason = request
                .reason
                .as_deref()
                .filter(|reason| !reason.trim().is_empty())
                .unwrap_or("Marked failed from Graphile Worker admin UI");
            let jobs = state
                .utils
                .permanently_fail_jobs(&request.ids, reason)
                .await
                .map_err(ApiError::internal)?;
            Ok(action_response("Failed", &jobs))
        }
        JobAction::RunNow => {
            let jobs = state
                .utils
                .reschedule_jobs(
                    &request.ids,
                    RescheduleJobOptions {
                        run_at: Some(Utc::now()),
                        priority: request.priority,
                        attempts: request.attempts,
                        max_attempts: request.max_attempts,
                    },
                )
                .await
                .map_err(ApiError::internal)?;
            Ok(action_response("Rescheduled", &jobs))
        }
        JobAction::Reschedule => {
            if request.run_at.is_none()
                && request.priority.is_none()
                && request.attempts.is_none()
                && request.max_attempts.is_none()
            {
                return Err(ApiError::bad_request(
                    "provide run_at, priority, attempts, or max_attempts",
                ));
            }

            let jobs = state
                .utils
                .reschedule_jobs(
                    &request.ids,
                    RescheduleJobOptions {
                        run_at: request.run_at,
                        priority: request.priority,
                        attempts: request.attempts,
                        max_attempts: request.max_attempts,
                    },
                )
                .await
                .map_err(ApiError::internal)?;
            Ok(action_response("Rescheduled", &jobs))
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn remove_job_by_key(
    State(state): State<Arc<AppState>>,
    Json(request): Json<RemoveJobByKeyRequest>,
) -> Result<Json<MessageResponse>, ApiError> {
    ensure_write_allowed(&state)?;
    if request.key.trim().is_empty() {
        return Err(ApiError::bad_request("key is required"));
    }

    state
        .utils
        .remove_job(&request.key)
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(MessageResponse {
        message: format!("Removed job with key `{}`", request.key),
    }))
}

#[cfg(not(target_arch = "wasm32"))]
async fn maintenance(
    State(state): State<Arc<AppState>>,
    Json(request): Json<MaintenanceRequest>,
) -> Result<Json<MessageResponse>, ApiError> {
    ensure_write_allowed(&state)?;
    match request.action {
        MaintenanceAction::Migrate => {
            state.utils.migrate().await.map_err(ApiError::internal)?;
            Ok(Json(MessageResponse {
                message: "Migrations complete".to_string(),
            }))
        }
        MaintenanceAction::Cleanup => {
            let tasks = if request.cleanup_tasks.is_empty() {
                CleanupTaskName::all()
            } else {
                request.cleanup_tasks
            };
            let cleanup_tasks: Vec<_> = tasks.into_iter().map(Into::into).collect();
            state
                .utils
                .cleanup(&cleanup_tasks)
                .await
                .map_err(ApiError::internal)?;
            Ok(Json(MessageResponse {
                message: "Cleanup complete".to_string(),
            }))
        }
        MaintenanceAction::ForceUnlock => {
            if request.worker_ids.is_empty() {
                return Err(ApiError::bad_request("provide at least one worker id"));
            }
            let worker_ids: Vec<_> = request.worker_ids.iter().map(String::as_str).collect();
            state
                .utils
                .force_unlock_workers(&worker_ids)
                .await
                .map_err(ApiError::internal)?;
            Ok(Json(MessageResponse {
                message: format!("Unlocked {} worker id(s)", worker_ids.len()),
            }))
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn require_basic_page_auth(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if state.auth.is_authorized(request.headers()) {
        next.run(request).await
    } else {
        unauthorized_response(&state.auth)
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn require_api_auth(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if !state.auth.is_authorized(request.headers()) {
        return unauthorized_response(&state.auth);
    }

    if is_write_method(request.method()) && !csrf_matches(request.headers(), &state.csrf_token) {
        return json_error_response(StatusCode::FORBIDDEN, "missing or invalid admin CSRF token");
    }

    next.run(request).await
}

#[cfg(not(target_arch = "wasm32"))]
async fn security_headers(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );
    headers.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    headers.insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'",
        ),
    );
    response
}

#[cfg(not(target_arch = "wasm32"))]
fn unauthorized_response(auth: &AdminAuthConfig) -> Response {
    let mut response = json_error_response(StatusCode::UNAUTHORIZED, "unauthorized");
    if matches!(auth, AdminAuthConfig::Basic { .. }) {
        response.headers_mut().insert(
            WWW_AUTHENTICATE,
            HeaderValue::from_static("Basic realm=\"Graphile Worker Admin\", charset=\"UTF-8\""),
        );
    }
    response
}

#[cfg(not(target_arch = "wasm32"))]
fn json_error_response(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(ErrorResponse {
            error: message.to_string(),
        }),
    )
        .into_response()
}

#[cfg(not(target_arch = "wasm32"))]
fn authorize_basic(headers: &HeaderMap, expected_user: &str, expected_password: &str) -> bool {
    let Some(header) = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let Some(encoded) = header.strip_prefix("Basic ") else {
        return false;
    };
    let Ok(decoded) = STANDARD.decode(encoded) else {
        return false;
    };
    let Ok(credentials) = String::from_utf8(decoded) else {
        return false;
    };
    let Some((user, password)) = credentials.split_once(':') else {
        return false;
    };

    constant_time_eq(user.as_bytes(), expected_user.as_bytes())
        & constant_time_eq(password.as_bytes(), expected_password.as_bytes())
}

#[cfg(not(target_arch = "wasm32"))]
fn authorize_bearer(headers: &HeaderMap, expected_token: &str) -> bool {
    let Some(header) = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let Some(token) = header.strip_prefix("Bearer ") else {
        return false;
    };
    constant_time_eq(token.as_bytes(), expected_token.as_bytes())
}

#[cfg(not(target_arch = "wasm32"))]
fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let max_len = left.len().max(right.len());
    let mut diff = left.len() ^ right.len();
    for index in 0..max_len {
        let left_byte = left.get(index).copied().unwrap_or_default();
        let right_byte = right.get(index).copied().unwrap_or_default();
        diff |= usize::from(left_byte ^ right_byte);
    }
    diff == 0
}

#[cfg(not(target_arch = "wasm32"))]
fn csrf_matches(headers: &HeaderMap, expected_token: &str) -> bool {
    headers
        .get(CSRF_HEADER)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| constant_time_eq(value.as_bytes(), expected_token.as_bytes()))
}

#[cfg(not(target_arch = "wasm32"))]
fn is_write_method(method: &Method) -> bool {
    !matches!(method, &Method::GET | &Method::HEAD | &Method::OPTIONS)
}

#[cfg(not(target_arch = "wasm32"))]
fn ensure_write_allowed(state: &AppState) -> Result<(), ApiError> {
    if state.read_only {
        return Err(ApiError::forbidden("admin UI is running in read-only mode"));
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

#[cfg(not(target_arch = "wasm32"))]
impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn internal(error: impl fmt::Display) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: error.to_string(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<sqlx::Error> for ApiError {
    fn from(value: sqlx::Error) -> Self {
        Self::internal(value)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        json_error_response(self.status, &self.message)
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ListJobsParams {
    #[serde(default)]
    state: JobState,
    identifier: Option<String>,
    queue: Option<String>,
    search: Option<String>,
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum JobState {
    #[default]
    All,
    Ready,
    Scheduled,
    Locked,
    Failed,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(sqlx::FromRow))]
struct ListedJob {
    pub(crate) id: i64,
    pub(crate) task_identifier: String,
    pub(crate) queue_name: Option<String>,
    pub(crate) payload: Value,
    pub(crate) priority: i16,
    pub(crate) run_at: DateTime<Utc>,
    pub(crate) attempts: i16,
    pub(crate) max_attempts: i16,
    pub(crate) last_error: Option<String>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) key: Option<String>,
    pub(crate) locked_at: Option<DateTime<Utc>>,
    pub(crate) locked_by: Option<String>,
    pub(crate) revision: i32,
    pub(crate) flags: Option<Value>,
    pub(crate) is_available: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(sqlx::FromRow))]
struct JobStats {
    pub(crate) total: i64,
    pub(crate) ready: i64,
    pub(crate) scheduled: i64,
    pub(crate) locked: i64,
    pub(crate) failed: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(sqlx::FromRow))]
struct QueueRow {
    pub(crate) id: i32,
    pub(crate) queue_name: String,
    pub(crate) locked_at: Option<DateTime<Utc>>,
    pub(crate) locked_by: Option<String>,
    pub(crate) job_count: i64,
    pub(crate) ready_count: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(sqlx::FromRow))]
struct LockedWorkerRow {
    pub(crate) worker_id: String,
    pub(crate) locked_jobs: i64,
    pub(crate) locked_queues: i64,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Deserialize, Serialize)]
struct DbJobOutput {
    id: i64,
    task_id: i32,
    task_identifier: Option<String>,
    job_queue_id: Option<i32>,
    payload: Value,
    priority: i16,
    run_at: DateTime<Utc>,
    attempts: i16,
    max_attempts: i16,
    last_error: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    key: Option<String>,
    revision: i32,
    locked_at: Option<DateTime<Utc>>,
    locked_by: Option<String>,
    flags: Option<Value>,
}

#[cfg(not(target_arch = "wasm32"))]
impl DbJobOutput {
    fn from_db_job(job: &DbJob) -> Self {
        Self {
            id: *job.id(),
            task_id: *job.task_id(),
            task_identifier: None,
            job_queue_id: *job.job_queue_id(),
            payload: job.payload().clone(),
            priority: *job.priority(),
            run_at: *job.run_at(),
            attempts: *job.attempts(),
            max_attempts: *job.max_attempts(),
            last_error: job.last_error().clone(),
            created_at: *job.created_at(),
            updated_at: *job.updated_at(),
            key: job.key().clone(),
            revision: *job.revision(),
            locked_at: *job.locked_at(),
            locked_by: job.locked_by().clone(),
            flags: job.flags().clone(),
        }
    }

    fn from_job(job: &Job) -> Self {
        Self {
            id: *job.id(),
            task_id: *job.task_id(),
            task_identifier: Some(job.task_identifier().clone()),
            job_queue_id: *job.job_queue_id(),
            payload: job.payload().clone(),
            priority: *job.priority(),
            run_at: *job.run_at(),
            attempts: *job.attempts(),
            max_attempts: *job.max_attempts(),
            last_error: job.last_error().clone(),
            created_at: *job.created_at(),
            updated_at: *job.updated_at(),
            key: job.key().clone(),
            revision: *job.revision(),
            locked_at: *job.locked_at(),
            locked_by: job.locked_by().clone(),
            flags: job.flags().clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct AddJobRequest {
    pub(crate) identifier: String,
    #[serde(default = "default_payload")]
    pub(crate) payload: Value,
    pub(crate) queue: Option<String>,
    pub(crate) run_at: Option<DateTime<Utc>>,
    pub(crate) max_attempts: Option<i16>,
    pub(crate) key: Option<String>,
    pub(crate) job_key_mode: Option<JobKeyModeRequest>,
    pub(crate) priority: Option<i16>,
    pub(crate) flags: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum JobKeyModeRequest {
    Replace,
    PreserveRunAt,
    UnsafeDedupe,
}

#[cfg(not(target_arch = "wasm32"))]
impl From<JobKeyModeRequest> for JobKeyMode {
    fn from(value: JobKeyModeRequest) -> Self {
        match value {
            JobKeyModeRequest::Replace => JobKeyMode::Replace,
            JobKeyModeRequest::PreserveRunAt => JobKeyMode::PreserveRunAt,
            JobKeyModeRequest::UnsafeDedupe => JobKeyMode::UnsafeDedupe,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct JobActionRequest {
    pub(crate) action: JobAction,
    pub(crate) ids: Vec<i64>,
    pub(crate) reason: Option<String>,
    pub(crate) run_at: Option<DateTime<Utc>>,
    pub(crate) priority: Option<i16>,
    pub(crate) attempts: Option<i16>,
    pub(crate) max_attempts: Option<i16>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum JobAction {
    Complete,
    Fail,
    RunNow,
    Reschedule,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RemoveJobByKeyRequest {
    pub(crate) key: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct MaintenanceRequest {
    pub(crate) action: MaintenanceAction,
    #[serde(default)]
    pub(crate) cleanup_tasks: Vec<CleanupTaskName>,
    #[serde(default)]
    pub(crate) worker_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum MaintenanceAction {
    Migrate,
    Cleanup,
    ForceUnlock,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum CleanupTaskName {
    DeletePermanentlyFailedJobs,
    GcTaskIdentifiers,
    GcJobQueues,
}

#[cfg(not(target_arch = "wasm32"))]
impl CleanupTaskName {
    fn all() -> Vec<Self> {
        vec![
            Self::DeletePermanentlyFailedJobs,
            Self::GcTaskIdentifiers,
            Self::GcJobQueues,
        ]
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<CleanupTaskName> for CleanupTask {
    fn from(value: CleanupTaskName) -> Self {
        match value {
            CleanupTaskName::DeletePermanentlyFailedJobs => {
                CleanupTask::DeletePermenantlyFailedJobs
            }
            CleanupTaskName::GcTaskIdentifiers => CleanupTask::GcTaskIdentifiers,
            CleanupTaskName::GcJobQueues => CleanupTask::GcJobQueues,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Serialize)]
struct SessionResponse {
    schema: String,
    read_only: bool,
    csrf_header: String,
    auth: AdminAuthSummary,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct OverviewResponse {
    pub(crate) stats: JobStats,
    pub(crate) queues: Vec<QueueRow>,
    pub(crate) workers: Vec<LockedWorkerRow>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ListJobsResponse {
    pub(crate) jobs: Vec<ListedJob>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct JobActionResponse {
    pub(crate) message: String,
    #[cfg(not(target_arch = "wasm32"))]
    jobs: Vec<DbJobOutput>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct MessageResponse {
    pub(crate) message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ErrorResponse {
    pub(crate) error: String,
}

#[cfg(not(target_arch = "wasm32"))]
fn action_response(action: &str, jobs: &[DbJob]) -> Json<JobActionResponse> {
    Json(JobActionResponse {
        message: format!("{action} {} job(s)", jobs.len()),
        jobs: jobs.iter().map(DbJobOutput::from_db_job).collect(),
    })
}

#[cfg(not(target_arch = "wasm32"))]
async fn list_jobs(
    pool: &PgPool,
    escaped_schema: &str,
    args: &ListJobsParams,
) -> Result<Vec<ListedJob>, ApiError> {
    if args.limit < 0 {
        return Err(ApiError::bad_request(
            "limit must be greater than or equal to 0",
        ));
    }
    if args.offset < 0 {
        return Err(ApiError::bad_request(
            "offset must be greater than or equal to 0",
        ));
    }

    let limit = args.limit.min(500);
    let mut query = QueryBuilder::<Postgres>::new(formatdoc!(
        r#"
            select
                jobs.id,
                tasks.identifier as task_identifier,
                job_queues.queue_name,
                jobs.payload,
                jobs.priority,
                jobs.run_at,
                jobs.attempts,
                jobs.max_attempts,
                jobs.last_error,
                jobs.created_at,
                jobs.updated_at,
                jobs.key,
                jobs.locked_at,
                jobs.locked_by,
                jobs.revision,
                jobs.flags,
                jobs.is_available
            from {escaped_schema}._private_jobs as jobs
            inner join {escaped_schema}._private_tasks as tasks on tasks.id = jobs.task_id
            left join {escaped_schema}._private_job_queues as job_queues on job_queues.id = jobs.job_queue_id
            where true
        "#
    ));

    apply_job_filters(&mut query, args);
    query.push(" order by jobs.id asc limit ");
    query.push_bind(limit);
    query.push(" offset ");
    query.push_bind(args.offset);

    query
        .build_query_as()
        .fetch_all(pool)
        .await
        .map_err(Into::into)
}

#[cfg(not(target_arch = "wasm32"))]
async fn get_job(pool: &PgPool, escaped_schema: &str, id: i64) -> Result<ListedJob, ApiError> {
    let mut query = QueryBuilder::<Postgres>::new(formatdoc!(
        r#"
            select
                jobs.id,
                tasks.identifier as task_identifier,
                job_queues.queue_name,
                jobs.payload,
                jobs.priority,
                jobs.run_at,
                jobs.attempts,
                jobs.max_attempts,
                jobs.last_error,
                jobs.created_at,
                jobs.updated_at,
                jobs.key,
                jobs.locked_at,
                jobs.locked_by,
                jobs.revision,
                jobs.flags,
                jobs.is_available
            from {escaped_schema}._private_jobs as jobs
            inner join {escaped_schema}._private_tasks as tasks on tasks.id = jobs.task_id
            left join {escaped_schema}._private_job_queues as job_queues on job_queues.id = jobs.job_queue_id
            where jobs.id =
        "#
    ));
    query.push_bind(id);

    query
        .build_query_as()
        .fetch_one(pool)
        .await
        .map_err(|error| job_lookup_error(id, error))
}

#[cfg(not(target_arch = "wasm32"))]
fn job_lookup_error(id: i64, error: sqlx::Error) -> ApiError {
    match error {
        sqlx::Error::RowNotFound => ApiError::not_found(format!("job {id} not found")),
        error => error.into(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn apply_job_filters<'a>(query: &mut QueryBuilder<'a, Postgres>, args: &'a ListJobsParams) {
    if let Some(identifier) = args.identifier.as_ref().filter(|value| !value.is_empty()) {
        query.push(" and tasks.identifier = ");
        query.push_bind(identifier);
    }

    if let Some(queue) = args.queue.as_ref().filter(|value| !value.is_empty()) {
        query.push(" and job_queues.queue_name = ");
        query.push_bind(queue);
    }

    if let Some(search) = args
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let pattern = format!("%{search}%");
        query.push(" and (jobs.id::text ilike ");
        query.push_bind(pattern.clone());
        query.push(" or tasks.identifier ilike ");
        query.push_bind(pattern.clone());
        query.push(" or coalesce(job_queues.queue_name, '') ilike ");
        query.push_bind(pattern.clone());
        query.push(" or coalesce(jobs.key, '') ilike ");
        query.push_bind(pattern.clone());
        query.push(" or coalesce(jobs.locked_by, '') ilike ");
        query.push_bind(pattern.clone());
        query.push(" or coalesce(jobs.last_error, '') ilike ");
        query.push_bind(pattern.clone());
        query.push(" or jobs.payload::text ilike ");
        query.push_bind(pattern);
        query.push(")");
    }

    match args.state {
        JobState::All => {}
        JobState::Ready => {
            query.push(
                " and jobs.locked_at is null and jobs.attempts < jobs.max_attempts and jobs.run_at <= now()",
            );
        }
        JobState::Scheduled => {
            query.push(
                " and jobs.locked_at is null and jobs.attempts < jobs.max_attempts and jobs.run_at > now()",
            );
        }
        JobState::Locked => {
            query.push(" and jobs.locked_at is not null");
        }
        JobState::Failed => {
            query.push(" and jobs.locked_at is null and jobs.attempts >= jobs.max_attempts");
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn get_stats(pool: &PgPool, escaped_schema: &str) -> Result<JobStats, ApiError> {
    let sql = formatdoc!(
        r#"
            select
                count(*)::bigint as total,
                count(*) filter (
                    where locked_at is null
                    and attempts < max_attempts
                    and run_at <= now()
                )::bigint as ready,
                count(*) filter (
                    where locked_at is null
                    and attempts < max_attempts
                    and run_at > now()
                )::bigint as scheduled,
                count(*) filter (where locked_at is not null)::bigint as locked,
                count(*) filter (
                    where locked_at is null
                    and attempts >= max_attempts
                )::bigint as failed
            from {escaped_schema}._private_jobs
        "#
    );

    sqlx::query_as(&sql)
        .fetch_one(pool)
        .await
        .map_err(Into::into)
}

#[cfg(not(target_arch = "wasm32"))]
async fn list_queues(pool: &PgPool, escaped_schema: &str) -> Result<Vec<QueueRow>, ApiError> {
    let sql = formatdoc!(
        r#"
            select
                job_queues.id,
                job_queues.queue_name,
                job_queues.locked_at,
                job_queues.locked_by,
                count(jobs.*)::bigint as job_count,
                count(jobs.*) filter (
                    where jobs.locked_at is null
                    and jobs.attempts < jobs.max_attempts
                    and jobs.run_at <= now()
                )::bigint as ready_count
            from {escaped_schema}._private_job_queues as job_queues
            left join {escaped_schema}._private_jobs as jobs on jobs.job_queue_id = job_queues.id
            group by job_queues.id, job_queues.queue_name, job_queues.locked_at, job_queues.locked_by
            order by job_queues.queue_name asc
        "#
    );

    sqlx::query_as(&sql)
        .fetch_all(pool)
        .await
        .map_err(Into::into)
}

#[cfg(not(target_arch = "wasm32"))]
async fn list_locked_workers(
    pool: &PgPool,
    escaped_schema: &str,
) -> Result<Vec<LockedWorkerRow>, ApiError> {
    let sql = formatdoc!(
        r#"
            select
                worker_id,
                sum(locked_jobs)::bigint as locked_jobs,
                sum(locked_queues)::bigint as locked_queues
            from (
                select locked_by as worker_id, count(*)::bigint as locked_jobs, 0::bigint as locked_queues
                from {escaped_schema}._private_jobs
                where locked_by is not null
                group by locked_by
                union all
                select locked_by as worker_id, 0::bigint as locked_jobs, count(*)::bigint as locked_queues
                from {escaped_schema}._private_job_queues
                where locked_by is not null
                group by locked_by
            ) as locks
            group by worker_id
            order by worker_id asc
        "#
    );

    sqlx::query_as(&sql)
        .fetch_all(pool)
        .await
        .map_err(Into::into)
}

#[cfg(not(target_arch = "wasm32"))]
fn default_limit() -> i64 {
    100
}

fn default_payload() -> Value {
    serde_json::json!({})
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub struct AdminUiRenderConfig {
    pub csrf_token: String,
    pub schema: String,
    pub read_only: bool,
    pub auth: AdminAuthSummary,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn render_admin_html(config: &AdminUiRenderConfig) -> String {
    let auth_mode = config.auth.mode.as_str();
    let header_name = config.auth.header_name.as_deref().unwrap_or("");
    let read_only = config.read_only.to_string();
    let app = view! {
        <div
            id="gw-admin"
            class="gw-shell"
            data-auth-mode=auth_mode
            data-auth-header=header_name
            data-read-only=read_only
            data-csrf=config.csrf_token.clone()
            data-csrf-header=CSRF_HEADER
            data-schema=config.schema.clone()
        >
            <aside class="gw-sidebar">
                <div class="flex items-center gap-3">
                    <div class="flex h-10 w-10 items-center justify-center rounded-lg bg-cyan-600 text-white">
                        <span class="i-lucide-workflow h-5 w-5"></span>
                    </div>
                    <div>
                        <h1 class="text-base font-semibold">"Graphile Worker"</h1>
                        <p class="gw-muted text-xs">"Admin UI"</p>
                    </div>
                </div>

                <nav class="mt-6 grid gap-1">
                    <a class="gw-tab" href="#jobs" aria-selected="true">
                        <span class="i-lucide-list-checks h-4 w-4"></span>
                        "Jobs"
                    </a>
                    <a class="gw-tab" href="#queues">
                        <span class="i-lucide-git-branch h-4 w-4"></span>
                        "Queues"
                    </a>
                    <a class="gw-tab" href="#workers">
                        <span class="i-lucide-hard-drive h-4 w-4"></span>
                        "Workers"
                    </a>
                    <a class="gw-tab" href="#maintenance">
                        <span class="i-tabler-tool h-4 w-4"></span>
                        "Maintenance"
                    </a>
                </nav>

                <div class="mt-auto grid gap-3 rounded-lg border p-3 text-xs" style="border-color: rgb(var(--border));">
                    <div class="flex items-center justify-between">
                        <span class="gw-muted">"Schema"</span>
                        <span class="font-mono">{config.schema.clone()}</span>
                    </div>
                    <div class="flex items-center justify-between">
                        <span class="gw-muted">"Auth"</span>
                        <span id="auth-mode-label" class="gw-pill">{auth_mode}</span>
                    </div>
                    <div class="flex items-center justify-between">
                        <span class="gw-muted">"Writes"</span>
                        <span class="gw-pill">{if config.read_only { "read only" } else { "enabled" }}</span>
                    </div>
                </div>
            </aside>

            <main class="gw-main">
                <header class="gw-topbar">
                    <div class="min-w-0">
                        <p class="gw-muted text-xs">"PostgreSQL-backed queue control plane"</p>
                        <h2 class="truncate text-lg font-semibold">"Jobs, queues, and workers"</h2>
                    </div>
                    <div class="flex flex-wrap items-center justify-end gap-2">
                        <select id="theme-select" class="gw-input w-32" aria-label="Theme">
                            <option value="system">"System"</option>
                            <option value="light">"Light"</option>
                            <option value="dark">"Dark"</option>
                        </select>
                        <select id="accent-select" class="gw-input w-32" aria-label="Accent">
                            <option value="cyan">"Cyan"</option>
                            <option value="emerald">"Emerald"</option>
                            <option value="violet">"Violet"</option>
                            <option value="amber">"Amber"</option>
                        </select>
                        <button id="density-toggle" class="gw-btn" type="button" title="Toggle density">
                            <span class="i-lucide-align-justify h-4 w-4"></span>
                        </button>
                        <label class="gw-btn cursor-pointer">
                            <input id="auto-refresh" class="h-4 w-4" type="checkbox" />
                            <span class="text-sm">"Auto"</span>
                        </label>
                        <button id="refresh-btn" class="gw-btn gw-btn-primary" type="button">
                            <span class="i-lucide-refresh-cw h-4 w-4"></span>
                            "Refresh"
                        </button>
                    </div>
                </header>

                <div class="gw-scroll">
                    <section id="token-login" class="gw-panel mb-4 hidden p-4">
                        <div class="flex flex-wrap items-end gap-3">
                            <div class="min-w-72 flex-1">
                                <label class="mb-1 block text-sm font-medium" for="auth-token">"API token"</label>
                                <input id="auth-token" class="gw-input w-full" type="password" autocomplete="current-password" />
                            </div>
                            <button id="save-token-btn" class="gw-btn gw-btn-primary" type="button">
                                <span class="i-lucide-key-round h-4 w-4"></span>
                                "Use token"
                            </button>
                            <button id="clear-token-btn" class="gw-btn" type="button">"Clear"</button>
                        </div>
                    </section>

                    <section id="overview" class="grid gap-3 md:grid-cols-5">
                        <div class="gw-panel p-4">
                            <div class="flex items-center justify-between">
                                <span class="gw-muted text-sm">"Total"</span>
                                <span class="i-lucide-database h-4 w-4 gw-muted"></span>
                            </div>
                            <strong id="stat-total" class="mt-2 block text-2xl">"0"</strong>
                        </div>
                        <div class="gw-panel p-4">
                            <div class="flex items-center justify-between">
                                <span class="gw-muted text-sm">"Ready"</span>
                                <span class="i-lucide-play h-4 w-4 text-emerald-500"></span>
                            </div>
                            <strong id="stat-ready" class="mt-2 block text-2xl">"0"</strong>
                        </div>
                        <div class="gw-panel p-4">
                            <div class="flex items-center justify-between">
                                <span class="gw-muted text-sm">"Scheduled"</span>
                                <span class="i-lucide-clock h-4 w-4 text-amber-500"></span>
                            </div>
                            <strong id="stat-scheduled" class="mt-2 block text-2xl">"0"</strong>
                        </div>
                        <div class="gw-panel p-4">
                            <div class="flex items-center justify-between">
                                <span class="gw-muted text-sm">"Locked"</span>
                                <span class="i-lucide-lock h-4 w-4 text-cyan-500"></span>
                            </div>
                            <strong id="stat-locked" class="mt-2 block text-2xl">"0"</strong>
                        </div>
                        <div class="gw-panel p-4">
                            <div class="flex items-center justify-between">
                                <span class="gw-muted text-sm">"Failed"</span>
                                <span class="i-lucide-circle-alert h-4 w-4 text-rose-500"></span>
                            </div>
                            <strong id="stat-failed" class="mt-2 block text-2xl">"0"</strong>
                        </div>
                    </section>

                    <section id="jobs" class="gw-panel mt-4">
                        <div class="flex flex-wrap items-center justify-between gap-3 border-b p-3" style="border-color: rgb(var(--border));">
                            <div class="flex flex-wrap items-center gap-2" role="tablist" aria-label="Job state">
                                <button class="gw-tab job-state-tab" data-state="all" aria-selected="true" type="button">"All"</button>
                                <button class="gw-tab job-state-tab" data-state="ready" type="button">"Ready"</button>
                                <button class="gw-tab job-state-tab" data-state="scheduled" type="button">"Scheduled"</button>
                                <button class="gw-tab job-state-tab" data-state="locked" type="button">"Locked"</button>
                                <button class="gw-tab job-state-tab" data-state="failed" type="button">"Failed"</button>
                            </div>
                            <div class="flex flex-wrap items-center gap-2">
                                <button id="add-job-btn" class="gw-btn gw-btn-primary" type="button">
                                    <span class="i-lucide-plus h-4 w-4"></span>
                                    "Add"
                                </button>
                                <button id="copy-selected-json-btn" class="gw-btn" type="button">
                                    <span class="i-lucide-copy h-4 w-4"></span>
                                    "JSON"
                                </button>
                                <button id="copy-selected-csv-btn" class="gw-btn" type="button">
                                    <span class="i-lucide-clipboard h-4 w-4"></span>
                                    "CSV"
                                </button>
                            </div>
                        </div>

                        <div class="grid gap-2 border-b p-3 lg:grid-cols-[minmax(240px,1fr)_repeat(4,minmax(120px,180px))_110px]" style="border-color: rgb(var(--border));">
                            <input id="global-search" class="gw-input" type="search" placeholder="Search id, task, queue, key, worker, payload..." />
                            <input class="gw-input column-filter" name="task_filter" data-column="task_identifier" type="search" placeholder="Task filter" />
                            <input class="gw-input column-filter" name="queue_filter" data-column="queue_name" type="search" placeholder="Queue filter" />
                            <input class="gw-input column-filter" name="key_filter" data-column="key" type="search" placeholder="Key filter" />
                            <input class="gw-input column-filter" name="worker_filter" data-column="locked_by" type="search" placeholder="Worker filter" />
                            <select id="limit-select" class="gw-input">
                                <option value="50">"50"</option>
                                <option value="100" selected>"100"</option>
                                <option value="250">"250"</option>
                                <option value="500">"500"</option>
                            </select>
                        </div>

                        <div class="flex flex-wrap items-center gap-2 border-b p-3" style="border-color: rgb(var(--border));">
                            <span id="selection-count" class="gw-pill">"0 selected"</span>
                            <button id="complete-selected-btn" class="gw-btn" type="button">
                                <span class="i-lucide-check h-4 w-4"></span>
                                "Complete"
                            </button>
                            <button id="run-now-selected-btn" class="gw-btn" type="button">
                                <span class="i-lucide-play h-4 w-4"></span>
                                "Run now"
                            </button>
                            <button id="reschedule-selected-btn" class="gw-btn" type="button">
                                <span class="i-lucide-calendar-clock h-4 w-4"></span>
                                "Reschedule"
                            </button>
                            <button id="fail-selected-btn" class="gw-btn gw-btn-danger" type="button">
                                <span class="i-lucide-ban h-4 w-4"></span>
                                "Fail"
                            </button>
                            <button id="remove-key-btn" class="gw-btn" type="button">
                                <span class="i-lucide-key-x h-4 w-4"></span>
                                "Remove by key"
                            </button>
                        </div>

                        <div class="max-h-[58vh] overflow-auto">
                            <table class="gw-table">
                                <thead>
                                    <tr>
                                        <th><input id="select-all-jobs" type="checkbox" class="h-4 w-4" /></th>
                                        <th>"ID"</th>
                                        <th>"Task"</th>
                                        <th>"Queue"</th>
                                        <th>"State"</th>
                                        <th>"Run at"</th>
                                        <th>"Attempts"</th>
                                        <th>"Priority"</th>
                                        <th>"Key"</th>
                                        <th>"Payload"</th>
                                        <th>"Error"</th>
                                        <th>"Actions"</th>
                                    </tr>
                                </thead>
                                <tbody id="jobs-tbody">
                                    <tr>
                                        <td colspan="12" class="py-8 text-center gw-muted">"Loading jobs..."</td>
                                    </tr>
                                </tbody>
                            </table>
                        </div>
                    </section>

                    <section id="queues-workers" class="mt-4 grid gap-4 lg:grid-cols-2">
                        <div id="queues" class="gw-panel">
                            <div class="flex items-center gap-2 border-b p-3" style="border-color: rgb(var(--border));">
                                <span class="i-lucide-git-branch h-4 w-4"></span>
                                <h3 class="font-semibold">"Queues"</h3>
                            </div>
                            <div id="queues-list" class="divide-y" style="border-color: rgb(var(--border));"></div>
                        </div>
                        <div id="workers" class="gw-panel">
                            <div class="flex items-center justify-between gap-2 border-b p-3" style="border-color: rgb(var(--border));">
                                <div class="flex items-center gap-2">
                                    <span class="i-lucide-hard-drive h-4 w-4"></span>
                                    <h3 class="font-semibold">"Workers"</h3>
                                </div>
                                <button id="force-unlock-btn" class="gw-btn" type="button">
                                    <span class="i-lucide-unlock h-4 w-4"></span>
                                    "Force unlock"
                                </button>
                            </div>
                            <div id="workers-list" class="divide-y" style="border-color: rgb(var(--border));"></div>
                        </div>
                    </section>

                    <section id="maintenance" class="gw-panel mt-4 p-4">
                        <div class="flex flex-wrap items-center justify-between gap-3">
                            <div>
                                <h3 class="font-semibold">"Maintenance"</h3>
                                <p class="gw-muted text-sm">"Run migrations, cleanup orphaned queue metadata, and recover abandoned locks."</p>
                            </div>
                            <div class="flex flex-wrap gap-2">
                                <button id="migrate-btn" class="gw-btn" type="button">
                                    <span class="i-lucide-database-zap h-4 w-4"></span>
                                    "Migrate"
                                </button>
                                <button id="cleanup-btn" class="gw-btn" type="button">
                                    <span class="i-lucide-sparkles h-4 w-4"></span>
                                    "Cleanup"
                                </button>
                            </div>
                        </div>
                    </section>
                </div>
            </main>

            <div id="modal" class="gw-modal" role="dialog" aria-modal="true" aria-labelledby="modal-title">
                <div class="gw-dialog">
                    <div class="mb-4 flex items-center justify-between gap-3">
                        <h3 id="modal-title" class="text-lg font-semibold"></h3>
                        <button id="modal-close" class="gw-btn" type="button" aria-label="Close">
                            <span class="i-lucide-x h-4 w-4"></span>
                        </button>
                    </div>
                    <div id="modal-body"></div>
                </div>
            </div>

            <div id="toast" class="gw-toast"></div>
        </div>
    };

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="color-scheme" content="light dark">
  <title>Graphile Worker Admin</title>
  <link rel="icon" href="/favicon.ico" type="image/svg+xml">
  <link rel="stylesheet" href="/assets/admin.css">
</head>
<body>
{body}
<script type="module" src="/assets/admin.js"></script>
</body>
</html>"#,
        body = app.to_html()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::Request;
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;

    #[test]
    fn generated_secret_is_hex_and_long_enough() {
        let secret = generate_secret();
        assert_eq!(secret.len(), 48);
        assert!(secret.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn render_includes_embedded_bootstrap_data_and_icons() {
        let html = render_admin_html(&AdminUiRenderConfig {
            csrf_token: "csrf".to_string(),
            schema: "graphile_worker".to_string(),
            read_only: false,
            auth: AdminAuthSummary {
                mode: PublicAuthMode::Basic,
                username: Some("admin".to_string()),
                header_name: None,
                generated_secret: true,
            },
        });

        assert!(html.contains("data-auth-mode=\"basic\""));
        assert!(html.contains("data-csrf=\"csrf\""));
        assert!(html.contains("i-lucide-refresh-cw"));
        assert!(html.contains("i-tabler-tool"));
        assert!(html.contains("/assets/admin.css"));
        assert!(html.contains("/assets/admin.js"));
    }

    #[test]
    fn basic_auth_accepts_correct_credentials() {
        let credentials = STANDARD.encode("admin:secret");
        let request = Request::builder()
            .header(AUTHORIZATION, format!("Basic {credentials}"))
            .body(Body::empty())
            .unwrap();

        assert!(authorize_basic(request.headers(), "admin", "secret"));
        assert!(!authorize_basic(request.headers(), "admin", "wrong"));
    }

    #[test]
    fn bearer_and_header_auth_accept_expected_tokens() {
        let bearer = Request::builder()
            .header(AUTHORIZATION, "Bearer admin-token")
            .body(Body::empty())
            .unwrap();
        assert!(AdminAuthConfig::bearer("admin-token", false).is_authorized(bearer.headers()));
        assert!(!AdminAuthConfig::bearer("other-token", false).is_authorized(bearer.headers()));

        let header = Request::builder()
            .header("x-admin-token", "header-token")
            .body(Body::empty())
            .unwrap();
        let auth = AdminAuthConfig::header("x-admin-token", "header-token", false).unwrap();
        assert!(auth.is_authorized(header.headers()));
    }

    #[tokio::test]
    async fn unauthorized_basic_response_prompts_for_basic_auth() {
        let response = unauthorized_response(&AdminAuthConfig::basic("admin", "secret"));
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(response.headers().contains_key(WWW_AUTHENTICATE));

        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(body.contains("unauthorized"));
    }

    #[tokio::test]
    async fn add_job_rejects_job_key_mode_without_key() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost/postgres")
            .unwrap();
        let database: graphile_worker::Database = pool.clone().into();
        let state = Arc::new(AppState {
            pool,
            utils: WorkerUtils::new(database, "graphile_worker".to_string()),
            escaped_schema: "graphile_worker".to_string(),
            schema: "graphile_worker".to_string(),
            auth: AdminAuthConfig::None,
            csrf_token: "csrf".to_string(),
            read_only: false,
        });

        let error = add_job(
            State(state),
            Json(AddJobRequest {
                identifier: "send_email".to_string(),
                payload: serde_json::json!({}),
                queue: None,
                run_at: None,
                max_attempts: None,
                key: None,
                job_key_mode: Some(JobKeyModeRequest::Replace),
                priority: None,
                flags: None,
            }),
        )
        .await
        .expect_err("request should be rejected before reaching the database");

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert!(error.message.contains("key"));
    }

    #[test]
    fn job_lookup_error_maps_missing_job_to_not_found() {
        let error = job_lookup_error(42, sqlx::Error::RowNotFound);

        assert_eq!(error.status, StatusCode::NOT_FOUND);
        assert!(error.message.contains("42"));
    }

    #[test]
    fn job_filters_ignore_whitespace_only_search() {
        let args = ListJobsParams {
            state: JobState::All,
            identifier: None,
            queue: None,
            search: Some("   ".to_string()),
            limit: default_limit(),
            offset: 0,
        };
        let mut query = QueryBuilder::<Postgres>::new("where true");

        apply_job_filters(&mut query, &args);

        assert_eq!(query.sql(), "where true");
    }
}
