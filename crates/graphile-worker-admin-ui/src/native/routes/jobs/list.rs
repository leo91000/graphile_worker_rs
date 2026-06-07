use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use graphile_worker_admin_api::queries::jobs::{get_job, list_jobs, ListJobsQueryOptions};

use super::super::super::error::ApiError;
use super::super::super::state::AppState;
use super::super::super::types::{ListJobsParams, ListJobsResponse, ListedJob};

pub(crate) async fn jobs(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListJobsParams>,
) -> Result<Json<ListJobsResponse>, ApiError> {
    let jobs = list_jobs(
        &state.pool,
        &state.schema,
        &params,
        ListJobsQueryOptions {
            max_limit: Some(500),
        },
    )
    .await?;
    Ok(Json(ListJobsResponse { jobs }))
}

pub(crate) async fn job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<ListedJob>, ApiError> {
    let job = get_job(&state.pool, &state.schema, id).await?;
    Ok(Json(job))
}
