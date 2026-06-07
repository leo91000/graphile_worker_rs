use std::sync::Arc;

use axum::middleware;
use axum::routing::{get, post};
use axum::Router;

use super::assets::{css, favicon, js, wasm, wasm_bindgen_js};
use super::auth::AdminAuthConfig;
use super::error::AdminUiError;
use super::middleware::{require_api_auth, require_page_auth, security_headers};
use super::routes::{jobs, maintenance, overview, page};
use super::state::{AdminServerConfig, AppState};

pub fn build_router(config: AdminServerConfig) -> Result<Router, AdminUiError> {
    let state = Arc::new(AppState::from_config(config)?);

    let api = Router::new()
        .route("/api/session", get(page::session))
        .route("/api/overview", get(overview::overview))
        .route("/api/jobs", get(jobs::list::jobs).post(jobs::add::add_job))
        .route("/api/jobs/{id}", get(jobs::list::job))
        .route("/api/jobs/action", post(jobs::actions::job_action))
        .route(
            "/api/jobs/remove-by-key",
            post(jobs::actions::remove_job_by_key),
        )
        .route("/api/maintenance", post(maintenance::maintenance))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_api_auth,
        ));

    let public = Router::new()
        .route("/", get(page::index))
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
        Ok(app.layer(middleware::from_fn_with_state(state, require_page_auth)))
    } else {
        Ok(app)
    }
}

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
