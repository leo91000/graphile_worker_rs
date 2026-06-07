use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use super::super::browser::show_toast;
use super::super::types::{
    AddJobRequest, AdminClientConfig, JobActionRequest, JobActionResponse, MaintenanceRequest,
    MessageResponse, Modal, RemoveJobByKeyRequest,
};
use super::http::api_post;
use super::refresh::{refresh_data, RefreshSignals};

pub(super) fn post_add_job(
    config: AdminClientConfig,
    request: AddJobRequest,
    signals: RefreshSignals,
    modal: RwSignal<Option<Modal>>,
) {
    spawn_local(async move {
        match api_post::<_, JobActionResponse>(
            "/api/jobs",
            &request,
            &config,
            &signals.token.get_untracked(),
        )
        .await
        {
            Ok(response) => {
                show_toast(signals.toast, response.message);
                modal.set(None);
                refresh_data(config, signals);
            }
            Err(error) => show_toast(signals.toast, error),
        }
    });
}

pub(super) fn post_job_action(
    config: AdminClientConfig,
    request: JobActionRequest,
    signals: RefreshSignals,
    modal: Option<RwSignal<Option<Modal>>>,
) {
    spawn_local(async move {
        match api_post::<_, JobActionResponse>(
            "/api/jobs/action",
            &request,
            &config,
            &signals.token.get_untracked(),
        )
        .await
        {
            Ok(response) => {
                show_toast(signals.toast, response.message);
                if let Some(modal) = modal {
                    modal.set(None);
                }
                refresh_data(config, signals);
            }
            Err(error) => show_toast(signals.toast, error),
        }
    });
}

pub(super) fn post_remove_key(
    config: AdminClientConfig,
    request: RemoveJobByKeyRequest,
    signals: RefreshSignals,
    modal: RwSignal<Option<Modal>>,
) {
    spawn_local(async move {
        match api_post::<_, MessageResponse>(
            "/api/jobs/remove-by-key",
            &request,
            &config,
            &signals.token.get_untracked(),
        )
        .await
        {
            Ok(response) => {
                show_toast(signals.toast, response.message);
                modal.set(None);
                refresh_data(config, signals);
            }
            Err(error) => show_toast(signals.toast, error),
        }
    });
}

pub(super) fn post_maintenance(
    config: AdminClientConfig,
    request: MaintenanceRequest,
    signals: RefreshSignals,
) {
    spawn_local(async move {
        match api_post::<_, MessageResponse>(
            "/api/maintenance",
            &request,
            &config,
            &signals.token.get_untracked(),
        )
        .await
        {
            Ok(response) => {
                show_toast(signals.toast, response.message);
                refresh_data(config, signals);
            }
            Err(error) => show_toast(signals.toast, error),
        }
    });
}
