use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use super::super::browser::show_toast;
use super::super::types::{AdminClientConfig, ListJobsResponse, ListedJob, OverviewResponse};
use super::http::api_get;

#[derive(Clone, Copy)]
pub(super) struct RefreshSignals {
    pub(super) token: RwSignal<String>,
    pub(super) limit: RwSignal<i64>,
    pub(super) overview: RwSignal<OverviewResponse>,
    pub(super) jobs: RwSignal<Vec<ListedJob>>,
    pub(super) selected_jobs: RwSignal<Vec<i64>>,
    pub(super) toast: RwSignal<Option<String>>,
    pub(super) refreshing: RwSignal<bool>,
    pub(super) refresh_pending: RwSignal<bool>,
}

pub(super) fn refresh_data(config: AdminClientConfig, signals: RefreshSignals) {
    if signals.refreshing.get_untracked() {
        signals.refresh_pending.set(true);
        return;
    }
    signals.refreshing.set(true);
    spawn_local(async move {
        loop {
            signals.refresh_pending.set(false);
            let token = signals.token.get_untracked();
            let limit_value = signals.limit.get_untracked();
            let jobs_path = format!("/api/jobs?state=all&limit={limit_value}");
            let overview_request = api_get::<OverviewResponse>("/api/overview", &config, &token);
            let jobs_request = api_get::<ListJobsResponse>(&jobs_path, &config, &token);
            let (overview_result, jobs_result) = futures::join!(overview_request, jobs_request);
            match (overview_result, jobs_result) {
                (Ok(next_overview), Ok(next_jobs)) => {
                    let next_ids = next_jobs.jobs.iter().map(|job| job.id).collect::<Vec<_>>();
                    signals.overview.set(next_overview);
                    signals.jobs.set(next_jobs.jobs);
                    signals
                        .selected_jobs
                        .update(|selected| selected.retain(|id| next_ids.contains(id)));
                }
                (Err(error), _) | (_, Err(error)) => show_toast(signals.toast, error),
            }
            if !signals.refresh_pending.get_untracked() {
                break;
            }
        }
        signals.refreshing.set(false);
    });
}
