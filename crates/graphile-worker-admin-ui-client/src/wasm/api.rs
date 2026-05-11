use gloo_net::http::{Request, RequestBuilder};
use leptos::prelude::*;
use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen_futures::spawn_local;
use web_sys::RequestCredentials;

use super::browser::show_toast;
use super::types::{
    AddJobRequest, AdminClientConfig, ErrorResponse, JobActionRequest, JobActionResponse,
    ListJobsResponse, ListedJob, MaintenanceRequest, MessageResponse, Modal, OverviewResponse,
    RemoveJobByKeyRequest,
};

pub(super) fn refresh_data(
    config: AdminClientConfig,
    token: RwSignal<String>,
    limit: RwSignal<i64>,
    overview: RwSignal<OverviewResponse>,
    jobs: RwSignal<Vec<ListedJob>>,
    selected_jobs: RwSignal<Vec<i64>>,
    toast: RwSignal<Option<String>>,
) {
    spawn_local(async move {
        let token = token.get_untracked();
        let limit_value = limit.get_untracked();
        let overview_result = api_get::<OverviewResponse>("/api/overview", &config, &token).await;
        let jobs_result = api_get::<ListJobsResponse>(
            &format!("/api/jobs?state=all&limit={limit_value}"),
            &config,
            &token,
        )
        .await;
        match (overview_result, jobs_result) {
            (Ok(next_overview), Ok(next_jobs)) => {
                let next_ids = next_jobs.jobs.iter().map(|job| job.id).collect::<Vec<_>>();
                overview.set(next_overview);
                jobs.set(next_jobs.jobs);
                selected_jobs.update(|selected| selected.retain(|id| next_ids.contains(id)));
            }
            (Err(error), _) | (_, Err(error)) => show_toast(toast, error),
        }
    });
}

pub(super) fn post_add_job(
    config: AdminClientConfig,
    token: RwSignal<String>,
    request: AddJobRequest,
    overview: RwSignal<OverviewResponse>,
    jobs: RwSignal<Vec<ListedJob>>,
    selected_jobs: RwSignal<Vec<i64>>,
    limit: RwSignal<i64>,
    modal: RwSignal<Option<Modal>>,
    toast: RwSignal<Option<String>>,
) {
    spawn_local(async move {
        match api_post::<_, JobActionResponse>(
            "/api/jobs",
            &request,
            &config,
            &token.get_untracked(),
        )
        .await
        {
            Ok(response) => {
                show_toast(toast, response.message);
                modal.set(None);
                refresh_data(config, token, limit, overview, jobs, selected_jobs, toast);
            }
            Err(error) => show_toast(toast, error),
        }
    });
}

pub(super) fn post_job_action(
    config: AdminClientConfig,
    token: RwSignal<String>,
    request: JobActionRequest,
    overview: RwSignal<OverviewResponse>,
    jobs: RwSignal<Vec<ListedJob>>,
    selected_jobs: RwSignal<Vec<i64>>,
    limit: RwSignal<i64>,
    toast: RwSignal<Option<String>>,
) {
    spawn_local(async move {
        match api_post::<_, JobActionResponse>(
            "/api/jobs/action",
            &request,
            &config,
            &token.get_untracked(),
        )
        .await
        {
            Ok(response) => {
                show_toast(toast, response.message);
                refresh_data(config, token, limit, overview, jobs, selected_jobs, toast);
            }
            Err(error) => show_toast(toast, error),
        }
    });
}

pub(super) fn post_remove_key(
    config: AdminClientConfig,
    token: RwSignal<String>,
    request: RemoveJobByKeyRequest,
    overview: RwSignal<OverviewResponse>,
    jobs: RwSignal<Vec<ListedJob>>,
    selected_jobs: RwSignal<Vec<i64>>,
    limit: RwSignal<i64>,
    modal: RwSignal<Option<Modal>>,
    toast: RwSignal<Option<String>>,
) {
    spawn_local(async move {
        match api_post::<_, MessageResponse>(
            "/api/jobs/remove-by-key",
            &request,
            &config,
            &token.get_untracked(),
        )
        .await
        {
            Ok(response) => {
                show_toast(toast, response.message);
                modal.set(None);
                refresh_data(config, token, limit, overview, jobs, selected_jobs, toast);
            }
            Err(error) => show_toast(toast, error),
        }
    });
}

pub(super) fn post_maintenance(
    config: AdminClientConfig,
    token: RwSignal<String>,
    request: MaintenanceRequest,
    overview: RwSignal<OverviewResponse>,
    jobs: RwSignal<Vec<ListedJob>>,
    selected_jobs: RwSignal<Vec<i64>>,
    limit: RwSignal<i64>,
    toast: RwSignal<Option<String>>,
) {
    spawn_local(async move {
        match api_post::<_, MessageResponse>(
            "/api/maintenance",
            &request,
            &config,
            &token.get_untracked(),
        )
        .await
        {
            Ok(response) => {
                show_toast(toast, response.message);
                refresh_data(config, token, limit, overview, jobs, selected_jobs, toast);
            }
            Err(error) => show_toast(toast, error),
        }
    });
}

pub(super) async fn api_get<T>(
    path: &str,
    config: &AdminClientConfig,
    token: &str,
) -> Result<T, String>
where
    T: DeserializeOwned,
{
    let response = with_api_headers(Request::get(&same_origin(path)), config, token, false)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    parse_response(response).await
}

pub(super) async fn api_post<B, T>(
    path: &str,
    body: &B,
    config: &AdminClientConfig,
    token: &str,
) -> Result<T, String>
where
    B: Serialize + ?Sized,
    T: DeserializeOwned,
{
    let request = with_api_headers(Request::post(&same_origin(path)), config, token, true)
        .json(body)
        .map_err(|error| error.to_string())?;
    let response = request.send().await.map_err(|error| error.to_string())?;
    parse_response(response).await
}

pub(super) fn with_api_headers(
    builder: RequestBuilder,
    config: &AdminClientConfig,
    token: &str,
    writes: bool,
) -> RequestBuilder {
    let mut builder = builder
        .credentials(RequestCredentials::SameOrigin)
        .header("Accept", "application/json");

    if writes {
        builder = builder.header(&config.csrf_header, &config.csrf);
    }
    if config.auth_mode == "bearer" && !token.is_empty() {
        builder = builder.header("Authorization", &format!("Bearer {token}"));
    }
    if config.auth_mode == "header" && !token.is_empty() && !config.auth_header.is_empty() {
        builder = builder.header(&config.auth_header, token);
    }
    builder
}

pub(super) async fn parse_response<T>(response: gloo_net::http::Response) -> Result<T, String>
where
    T: DeserializeOwned,
{
    let status = response.status();
    let text = response.text().await.map_err(|error| error.to_string())?;
    if !(200..300).contains(&status) {
        return Err(serde_json::from_str::<ErrorResponse>(&text)
            .map(|error| error.error)
            .unwrap_or_else(|_| format!("{status}: {text}")));
    }
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

pub(super) fn same_origin(path: &str) -> String {
    let Some(window) = web_sys::window() else {
        return path.to_string();
    };
    let location = window.location();
    let protocol = location.protocol().unwrap_or_default();
    let host = location.host().unwrap_or_default();
    format!("{protocol}//{host}{path}")
}
