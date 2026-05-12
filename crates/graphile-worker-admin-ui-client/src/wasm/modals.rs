use leptos::prelude::*;
use serde_json::Value;

use super::api::{post_add_job, post_job_action, post_remove_key};
use super::browser::{copy_to_clipboard, show_toast};
use super::filters::{
    datetime_local_to_utc, job_state, modal_title, optional_csv, optional_i16, optional_string,
    state_label, textarea_value,
};
use super::types::{
    AddJobRequest, AdminClientConfig, JobAction, JobActionRequest, JobKeyModeRequest, ListedJob,
    Modal, OverviewResponse, RemoveJobByKeyRequest,
};

#[component]
pub(super) fn ModalView(
    modal: RwSignal<Option<Modal>>,
    config: AdminClientConfig,
    token: RwSignal<String>,
    overview: RwSignal<OverviewResponse>,
    jobs: RwSignal<Vec<ListedJob>>,
    selected_jobs: RwSignal<Vec<i64>>,
    limit: RwSignal<i64>,
    toast: RwSignal<Option<String>>,
    refreshing: RwSignal<bool>,
    refresh_pending: RwSignal<bool>,
) -> impl IntoView {
    view! {
        <div class="gw-modal" data-open=move || modal.get().is_some().to_string() role="dialog" aria-modal="true">
            <div class="gw-dialog">
                <div class="mb-4 flex items-center justify-between gap-3">
                    <h3 class="text-lg font-semibold">{move || modal_title(&modal.get())}</h3>
                    <button class="gw-btn" type="button" aria-label="Close" on:click=move |_| modal.set(None)>
                        <span class="i-lucide-x h-4 w-4"></span>
                    </button>
                </div>
                {move || match modal.get() {
                    Some(Modal::AddJob) => view! {
                        <AddJobModal
                            config=config.clone()
                            token=token
                            overview=overview
                            jobs=jobs
                            selected_jobs=selected_jobs
                            limit=limit
                            modal=modal
                            toast=toast
                            refreshing=refreshing
                            refresh_pending=refresh_pending
                        />
                    }.into_any(),
                    Some(Modal::FailJobs) => view! {
                        <FailJobsModal
                            config=config.clone()
                            token=token
                            overview=overview
                            jobs=jobs
                            selected_jobs=selected_jobs
                            limit=limit
                            modal=modal
                            toast=toast
                            refreshing=refreshing
                            refresh_pending=refresh_pending
                        />
                    }.into_any(),
                    Some(Modal::Reschedule) => view! {
                        <RescheduleModal
                            config=config.clone()
                            token=token
                            overview=overview
                            jobs=jobs
                            selected_jobs=selected_jobs
                            limit=limit
                            modal=modal
                            toast=toast
                            refreshing=refreshing
                            refresh_pending=refresh_pending
                        />
                    }.into_any(),
                    Some(Modal::RemoveKey) => view! {
                        <RemoveKeyModal
                            config=config.clone()
                            token=token
                            overview=overview
                            jobs=jobs
                            selected_jobs=selected_jobs
                            limit=limit
                            modal=modal
                            toast=toast
                            refreshing=refreshing
                            refresh_pending=refresh_pending
                        />
                    }.into_any(),
                    Some(Modal::JobDetails(job)) => view! { <JobDetailsModal job=job toast=toast /> }.into_any(),
                    None => ().into_any(),
                }}
            </div>
        </div>
    }
}

#[component]
pub(super) fn AddJobModal(
    config: AdminClientConfig,
    token: RwSignal<String>,
    overview: RwSignal<OverviewResponse>,
    jobs: RwSignal<Vec<ListedJob>>,
    selected_jobs: RwSignal<Vec<i64>>,
    limit: RwSignal<i64>,
    modal: RwSignal<Option<Modal>>,
    toast: RwSignal<Option<String>>,
    refreshing: RwSignal<bool>,
    refresh_pending: RwSignal<bool>,
) -> impl IntoView {
    let identifier = RwSignal::new(String::new());
    let queue = RwSignal::new(String::new());
    let key = RwSignal::new(String::new());
    let key_mode = RwSignal::new(String::new());
    let priority = RwSignal::new(String::new());
    let max_attempts = RwSignal::new(String::new());
    let run_at = RwSignal::new(String::new());
    let payload = RwSignal::new("{\"hello\":\"world\"}".to_string());
    let flags = RwSignal::new(String::new());

    view! {
        <form class="grid gap-3" on:submit=move |event| {
            event.prevent_default();
            let payload_value = match serde_json::from_str::<Value>(&payload.get_untracked()) {
                Ok(payload) => payload,
                Err(error) => {
                    show_toast(toast, format!("Invalid JSON: {error}"));
                    return;
                }
            };
            let request = AddJobRequest {
                identifier: identifier.get_untracked(),
                payload: payload_value,
                queue: optional_string(queue.get_untracked()),
                run_at: datetime_local_to_utc(&run_at.get_untracked()),
                max_attempts: optional_i16(max_attempts.get_untracked()),
                key: optional_string(key.get_untracked()),
                job_key_mode: match key_mode.get_untracked().as_str() {
                    "replace" => Some(JobKeyModeRequest::Replace),
                    "preserve-run-at" => Some(JobKeyModeRequest::PreserveRunAt),
                    "unsafe-dedupe" => Some(JobKeyModeRequest::UnsafeDedupe),
                    _ => None,
                },
                priority: optional_i16(priority.get_untracked()),
                flags: optional_csv(flags.get_untracked()),
            };
            post_add_job(
                config.clone(),
                token,
                request,
                overview,
                jobs,
                selected_jobs,
                limit,
                modal,
                toast,
                refreshing,
                refresh_pending,
            );
        }>
            <div class="grid gap-3 md:grid-cols-2">
                <TextInput label="Task identifier" value=identifier required=true />
                <TextInput label="Queue" value=queue required=false />
                <TextInput label="Key" value=key required=false />
                <label class="grid gap-1 text-sm" for="key-mode">"Key mode"
                    <select id="key-mode" name="job_key_mode" class="gw-input" prop:value=move || key_mode.get() on:change=move |event| key_mode.set(event_target_value(&event))>
                        <option value="">"Default"</option>
                        <option value="replace">"Replace"</option>
                        <option value="preserve-run-at">"Preserve run_at"</option>
                        <option value="unsafe-dedupe">"Unsafe dedupe"</option>
                    </select>
                </label>
                <TextInput label="Priority" value=priority input_type="number" required=false />
                <TextInput label="Max attempts" value=max_attempts input_type="number" required=false />
                <TextInput label="Run at" value=run_at input_type="datetime-local" required=false class="grid gap-1 text-sm md:col-span-2" />
            </div>
            <label class="grid gap-1 text-sm" for="job-payload">"Payload JSON"
                <textarea id="job-payload" name="payload" class="gw-textarea font-mono" prop:value=move || payload.get() on:input=move |event| payload.set(textarea_value(&event))></textarea>
            </label>
            <TextInput label="Flags, comma-separated" value=flags required=false />
            <ModalButtons modal=modal danger=false submit_label="Add job" submit_icon="i-lucide-plus" />
        </form>
    }
}

#[component]
pub(super) fn FailJobsModal(
    config: AdminClientConfig,
    token: RwSignal<String>,
    overview: RwSignal<OverviewResponse>,
    jobs: RwSignal<Vec<ListedJob>>,
    selected_jobs: RwSignal<Vec<i64>>,
    limit: RwSignal<i64>,
    modal: RwSignal<Option<Modal>>,
    toast: RwSignal<Option<String>>,
    refreshing: RwSignal<bool>,
    refresh_pending: RwSignal<bool>,
) -> impl IntoView {
    let reason = RwSignal::new("Marked failed from Graphile Worker admin UI".to_string());
    view! {
        <form class="grid gap-3" on:submit=move |event| {
            event.prevent_default();
            post_job_action(
                config.clone(),
                token,
                JobActionRequest {
                    action: JobAction::Fail,
                    ids: selected_jobs.get_untracked(),
                    reason: optional_string(reason.get_untracked()),
                    run_at: None,
                    priority: None,
                    attempts: None,
                    max_attempts: None,
                },
                overview,
                jobs,
                selected_jobs,
                limit,
                Some(modal),
                toast,
                refreshing,
                refresh_pending,
            );
        }>
            <p class="gw-muted text-sm">{move || format!("This permanently fails {} selected job(s).", selected_jobs.get().len())}</p>
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
    token: RwSignal<String>,
    overview: RwSignal<OverviewResponse>,
    jobs: RwSignal<Vec<ListedJob>>,
    selected_jobs: RwSignal<Vec<i64>>,
    limit: RwSignal<i64>,
    modal: RwSignal<Option<Modal>>,
    toast: RwSignal<Option<String>>,
    refreshing: RwSignal<bool>,
    refresh_pending: RwSignal<bool>,
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
                token,
                JobActionRequest {
                    action: JobAction::Reschedule,
                    ids: selected_jobs.get_untracked(),
                    reason: None,
                    run_at: datetime_local_to_utc(&run_at.get_untracked()),
                    priority: optional_i16(priority.get_untracked()),
                    attempts: optional_i16(attempts.get_untracked()),
                    max_attempts: optional_i16(max_attempts.get_untracked()),
                },
                overview,
                jobs,
                selected_jobs,
                limit,
                Some(modal),
                toast,
                refreshing,
                refresh_pending,
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
    token: RwSignal<String>,
    overview: RwSignal<OverviewResponse>,
    jobs: RwSignal<Vec<ListedJob>>,
    selected_jobs: RwSignal<Vec<i64>>,
    limit: RwSignal<i64>,
    modal: RwSignal<Option<Modal>>,
    toast: RwSignal<Option<String>>,
    refreshing: RwSignal<bool>,
    refresh_pending: RwSignal<bool>,
) -> impl IntoView {
    let key = RwSignal::new(String::new());
    view! {
        <form class="grid gap-3" on:submit=move |event| {
            event.prevent_default();
            post_remove_key(
                config.clone(),
                token,
                RemoveJobByKeyRequest { key: key.get_untracked() },
                overview,
                jobs,
                selected_jobs,
                limit,
                modal,
                toast,
                refreshing,
                refresh_pending,
            );
        }>
            <TextInput label="Job key" value=key required=true />
            <ModalButtons modal=modal danger=true submit_label="Remove" submit_icon="i-tabler-key-off" />
        </form>
    }
}

#[component]
pub(super) fn JobDetailsModal(job: ListedJob, toast: RwSignal<Option<String>>) -> impl IntoView {
    let row_state = state_label(job_state(&job));
    let job_json = serde_json::to_string_pretty(&job).unwrap_or_default();
    view! {
        <div class="grid gap-3">
            <div class="grid gap-3 md:grid-cols-3">
                <div class="gw-panel p-3"><span class="gw-muted text-xs">"Task"</span><strong class="block">{job.task_identifier.clone()}</strong></div>
                <div class="gw-panel p-3"><span class="gw-muted text-xs">"Queue"</span><strong class="block">{job.queue_name.clone().unwrap_or_else(|| "default".to_string())}</strong></div>
                <div class="gw-panel p-3"><span class="gw-muted text-xs">"State"</span><strong class="block">{row_state}</strong></div>
            </div>
            <label class="grid gap-1 text-sm" for="job-detail-payload">"Payload"
                <textarea id="job-detail-payload" name="job_detail_payload" class="gw-textarea font-mono" readonly>{serde_json::to_string_pretty(&job.payload).unwrap_or_default()}</textarea>
            </label>
            <label class="grid gap-1 text-sm" for="job-detail-last-error">"Last error"
                <textarea id="job-detail-last-error" name="job_detail_last_error" class="gw-textarea font-mono" readonly>{job.last_error.clone().unwrap_or_default()}</textarea>
            </label>
            <div class="flex justify-end gap-2">
                <button class="gw-btn" type="button" on:click=move |_| copy_to_clipboard(job_json.clone(), "Copied job JSON", toast)>
                    <span class="i-lucide-copy h-4 w-4"></span>"Copy JSON"
                </button>
            </div>
        </div>
    }
}

#[component]
pub(super) fn TextInput(
    label: &'static str,
    value: RwSignal<String>,
    #[prop(default = "text")] input_type: &'static str,
    #[prop(default = false)] required: bool,
    #[prop(default = "grid gap-1 text-sm")] class: &'static str,
) -> impl IntoView {
    let id = format!(
        "admin-{}",
        label
            .to_ascii_lowercase()
            .replace(|character: char| !character.is_ascii_alphanumeric(), "-")
    );
    let name = id.trim_start_matches("admin-").replace('-', "_");
    let input_id = id.clone();
    view! {
        <label class=class for=id.clone()>{label}
            <input
                id=input_id
                name=name
                class="gw-input"
                type=input_type
                required=required
                prop:value=move || value.get()
                on:input=move |event| value.set(event_target_value(&event))
            />
        </label>
    }
}

#[component]
pub(super) fn ModalButtons(
    modal: RwSignal<Option<Modal>>,
    danger: bool,
    submit_label: &'static str,
    submit_icon: &'static str,
) -> impl IntoView {
    view! {
        <div class="flex justify-end gap-2">
            <button class="gw-btn" type="button" on:click=move |_| modal.set(None)>"Cancel"</button>
            <button class=if danger { "gw-btn gw-btn-danger" } else { "gw-btn gw-btn-primary" } type="submit">
                <span class=format!("{submit_icon} h-4 w-4")></span>
                {submit_label}
            </button>
        </div>
    }
}
