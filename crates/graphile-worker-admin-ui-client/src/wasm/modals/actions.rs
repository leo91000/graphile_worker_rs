use leptos::prelude::*;

use super::super::api::{post_job_action, post_remove_key, RefreshSignals};
use super::super::filters::{datetime_local_to_utc, optional_i16, optional_string, textarea_value};
use super::super::types::{
    AdminClientConfig, JobAction, JobActionRequest, Modal, RemoveJobByKeyRequest,
};
use super::form::{ModalButtons, TextInput};

#[component]
pub(super) fn FailJobsModal(
    config: AdminClientConfig,
    modal: RwSignal<Option<Modal>>,
    refresh: RefreshSignals,
) -> impl IntoView {
    let reason = RwSignal::new("Marked failed from Graphile Worker admin UI".to_string());
    view! {
        <form class="grid gap-3" on:submit=move |event| {
            event.prevent_default();
            post_job_action(
                config.clone(),
                JobActionRequest {
                    action: JobAction::Fail,
                    ids: refresh.selected_jobs.get_untracked(),
                    reason: optional_string(reason.get_untracked()),
                    run_at: None,
                    priority: None,
                    attempts: None,
                    max_attempts: None,
                },
                refresh,
                Some(modal),
            );
        }>
            <p class="gw-muted text-sm">{move || format!("This permanently fails {} selected job(s).", refresh.selected_jobs.get().len())}</p>
            <label class="grid gap-1 text-sm" for="fail-reason">"Reason"
                <textarea id="fail-reason" name="reason" class="gw-textarea" prop:value=move || reason.get() on:input=move |event| reason.set(textarea_value(&event))></textarea>
            </label>
            <ModalButtons modal=modal danger=true submit_label="Fail jobs" submit_icon="i-lucide-ban" />
        </form>
    }
}

#[component]
pub(super) fn RescheduleModal(
    config: AdminClientConfig,
    modal: RwSignal<Option<Modal>>,
    refresh: RefreshSignals,
) -> impl IntoView {
    let run_at = RwSignal::new(String::new());
    let priority = RwSignal::new(String::new());
    let attempts = RwSignal::new(String::new());
    let max_attempts = RwSignal::new(String::new());
    view! {
        <form class="grid gap-3" on:submit=move |event| {
            event.prevent_default();
            post_job_action(
                config.clone(),
                JobActionRequest {
                    action: JobAction::Reschedule,
                    ids: refresh.selected_jobs.get_untracked(),
                    reason: None,
                    run_at: datetime_local_to_utc(&run_at.get_untracked()),
                    priority: optional_i16(priority.get_untracked()),
                    attempts: optional_i16(attempts.get_untracked()),
                    max_attempts: optional_i16(max_attempts.get_untracked()),
                },
                refresh,
                Some(modal),
            );
        }>
            <div class="grid gap-3 md:grid-cols-2">
                <TextInput label="Run at" value=run_at input_type="datetime-local" required=false />
                <TextInput label="Priority" value=priority input_type="number" required=false />
                <TextInput label="Attempts" value=attempts input_type="number" required=false />
                <TextInput label="Max attempts" value=max_attempts input_type="number" required=false />
            </div>
            <ModalButtons modal=modal danger=false submit_label="Reschedule" submit_icon="i-lucide-calendar-clock" />
        </form>
    }
}

#[component]
pub(super) fn RemoveKeyModal(
    config: AdminClientConfig,
    modal: RwSignal<Option<Modal>>,
    refresh: RefreshSignals,
) -> impl IntoView {
    let key = RwSignal::new(String::new());
    view! {
        <form class="grid gap-3" on:submit=move |event| {
            event.prevent_default();
            post_remove_key(
                config.clone(),
                RemoveJobByKeyRequest { key: key.get_untracked() },
                refresh,
                modal,
            );
        }>
            <TextInput label="Job key" value=key required=true />
            <ModalButtons modal=modal danger=true submit_label="Remove" submit_icon="i-tabler-key-off" />
        </form>
    }
}
