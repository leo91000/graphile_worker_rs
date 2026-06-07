use std::sync::Arc;

use axum::extract::State;
use axum::response::Html;
use axum::Json;

use super::super::auth::CSRF_HEADER;
use super::super::state::AppState;
use super::super::types::SessionResponse;
use super::super::view::{render_admin_html, AdminUiRenderConfig};

pub(crate) async fn index(State(state): State<Arc<AppState>>) -> Html<String> {
    let auth = state.auth.summary();
    Html(render_admin_html(&AdminUiRenderConfig {
        csrf_token: state.csrf_token.clone(),
        schema: state.schema_name.clone(),
        read_only: state.read_only,
        auth,
    }))
}

pub(crate) async fn session(State(state): State<Arc<AppState>>) -> Json<SessionResponse> {
    Json(SessionResponse {
        schema: state.schema_name.clone(),
        read_only: state.read_only,
        csrf_header: CSRF_HEADER.to_string(),
        auth: state.auth.summary(),
    })
}
