use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::extract::State;
use axum::http::header::{AUTHORIZATION, WWW_AUTHENTICATE};
use axum::http::{Request, StatusCode};
use axum::Json;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use graphile_worker::{Schema, WorkerUtils};
use graphile_worker_admin_api::defaults::default_limit;
use graphile_worker_admin_api::jobs::JobState;
use graphile_worker_admin_api::queries::error::AdminQueryError;
use graphile_worker_admin_api::queries::jobs::filters::apply_job_filters;
use sqlx::{Postgres, QueryBuilder};

use super::auth::{
    authorize_basic, generate_secret, AdminAuthConfig, AdminAuthSummary, PublicAuthMode,
};
use super::error::{AdminUiError, ApiError};
use super::middleware::unauthorized_response;
use super::routes::jobs::add::add_job;
use super::state::{AdminServerConfig, AppState};
use super::types::{AddJobRequest, JobKeyModeRequest, ListJobsParams};
use super::view::{render_admin_html, AdminUiRenderConfig};

fn lazy_pool() -> sqlx::PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://postgres:postgres@localhost/postgres")
        .unwrap()
}

fn admin_config(listen_addr: SocketAddr, auth: AdminAuthConfig) -> AdminServerConfig {
    let pool = lazy_pool();
    let database: graphile_worker::Database = pool.clone().into();
    AdminServerConfig {
        pool,
        utils: WorkerUtils::new(database, "graphile_worker".to_string()),
        schema: Schema::new("graphile_worker"),
        schema_name: "graphile_worker".to_string(),
        listen_addr,
        auth,
        read_only: false,
    }
}

mod auth;
mod errors;
mod jobs;
mod render;
mod state;
