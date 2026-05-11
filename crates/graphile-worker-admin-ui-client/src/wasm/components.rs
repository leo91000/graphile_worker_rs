use leptos::prelude::*;
use web_sys::ClipboardEvent;

use super::api::post_maintenance;
use super::browser::{copy_to_clipboard, show_toast};
use super::filters::{
    format_date, job_state, normalize_filter_paste, short, state_color, state_label,
    stringify_value,
};
use super::types::{
    AdminClientConfig, JobState, JobStats, ListedJob, MaintenanceAction, MaintenanceRequest, Modal,
    OverviewResponse,
};

#[component]
pub(super) fn Overview(
    stats: impl Fn() -> JobStats + Copy + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <section id="overview" class="grid gap-3 md:grid-cols-5">
            <StatCard label="Total" icon="i-lucide-database" value=move || stats().total.to_string() />
            <StatCard label="Ready" icon="i-lucide-play text-emerald-500" value=move || stats().ready.to_string() />
            <StatCard label="Scheduled" icon="i-lucide-clock text-amber-500" value=move || stats().scheduled.to_string() />
            <StatCard label="Locked" icon="i-lucide-lock text-cyan-500" value=move || stats().locked.to_string() />
            <StatCard label="Failed" icon="i-lucide-circle-alert text-rose-500" value=move || stats().failed.to_string() />
        </section>
    }
}

#[component]
pub(super) fn StatCard(
    label: &'static str,
    icon: &'static str,
    value: impl Fn() -> String + Copy + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <div class="gw-panel p-4">
            <div class="flex items-center justify-between">
                <span class="gw-muted text-sm">{label}</span>
                <span class=format!("{icon} h-4 w-4 gw-muted")></span>
            </div>
            <strong class="mt-2 block text-2xl">{move || value()}</strong>
        </div>
    }
}

#[component]
pub(super) fn StateTab(
    label: &'static str,
    state: JobState,
    active_state: RwSignal<JobState>,
) -> impl IntoView {
    view! {
        <button
            class="gw-tab"
            aria-selected=move || (active_state.get() == state).to_string()
            type="button"
            on:click=move |_| active_state.set(state)
        >
            {label}
        </button>
    }
}

#[component]
pub(super) fn ColumnFilter(
    name: &'static str,
    placeholder: &'static str,
    value: RwSignal<String>,
) -> impl IntoView {
    view! {
        <input
            class="gw-input column-filter"
            name=name
            type="search"
            placeholder=placeholder
            prop:value=move || value.get()
            on:input=move |event| value.set(event_target_value(&event))
            on:paste=move |event: ClipboardEvent| {
                let pasted = event
                    .clipboard_data()
                    .and_then(|data| data.get_data("text").ok())
                    .unwrap_or_default();
                if pasted.contains('\n') || pasted.contains('\r') || pasted.contains('\t') || pasted.contains(',') {
                    event.prevent_default();
                    value.set(normalize_filter_paste(&pasted));
                }
            }
        />
    }
}

#[component]
pub(super) fn JobRow(
    job: ListedJob,
    selected_jobs: RwSignal<Vec<i64>>,
    modal: RwSignal<Option<Modal>>,
    toast: RwSignal<Option<String>>,
) -> impl IntoView {
    let id = job.id;
    let row_state = job_state(&job);
    let queue = job.queue_name.clone();
    let key = job.key.clone();
    let payload = stringify_value(&job.payload);
    let last_error = job.last_error.clone().unwrap_or_default();
    let job_for_details = job.clone();

    view! {
        <tr>
            <td>
                <input
                    class="h-4 w-4"
                    type="checkbox"
                    name="selected_jobs"
                    aria-label=format!("Select job {id}")
                    prop:checked=move || selected_jobs.get().contains(&id)
                    on:change=move |event| {
                        selected_jobs.update(|selected| {
                            if event_target_checked(&event) {
                                if !selected.contains(&id) {
                                    selected.push(id);
                                }
                            } else {
                                selected.retain(|candidate| *candidate != id);
                            }
                        });
                    }
                />
            </td>
            <td class="font-mono">{id}</td>
            <td><button class="font-medium hover:underline" type="button" on:click={
                let task = job.task_identifier.clone();
                move |_| copy_to_clipboard(task.clone(), "Copied task_identifier", toast)
            }>{job.task_identifier.clone()}</button></td>
            <td>{queue.clone().map(|queue| view! {
                <button class="hover:underline" type="button" on:click={
                    let queue = queue.clone();
                    move |_| copy_to_clipboard(queue.clone(), "Copied queue_name", toast)
                }>{queue.clone()}</button>
            }.into_any()).unwrap_or_else(|| view! { <span class="gw-muted">"default"</span> }.into_any())}</td>
            <td><span class=format!("gw-pill {}", state_color(row_state))>{state_label(row_state)}</span></td>
            <td class="whitespace-nowrap">{format_date(job.run_at)}</td>
            <td>{format!("{}/{}", job.attempts, job.max_attempts)}</td>
            <td>{job.priority}</td>
            <td>{key.clone().map(|key| view! {
                <button class="font-mono text-xs hover:underline" type="button" on:click={
                    let key = key.clone();
                    move |_| copy_to_clipboard(key.clone(), "Copied key", toast)
                }>{short(&key, 28)}</button>
            }.into_any()).unwrap_or_else(|| ().into_any())}</td>
            <td><button class="max-w-72 text-left font-mono text-xs hover:underline" type="button" on:click={
                let payload = payload.clone();
                move |_| copy_to_clipboard(payload.clone(), "Copied payload", toast)
            }>{short(&payload, 90)}</button></td>
            <td><button class="max-w-56 text-left text-xs text-rose-600 hover:underline dark:text-rose-300" type="button" on:click={
                let last_error = last_error.clone();
                move |_| copy_to_clipboard(last_error.clone(), "Copied last_error", toast)
            }>{short(&last_error, 70)}</button></td>
            <td>
                <button class="gw-btn h-8 px-2" type="button" title="View details" on:click=move |_| modal.set(Some(Modal::JobDetails(job_for_details.clone())))>
                    <span class="i-lucide-eye h-4 w-4"></span>
                </button>
            </td>
        </tr>
    }
}

#[component]
pub(super) fn QueuesPanel(
    overview: RwSignal<OverviewResponse>,
    queue_filter: RwSignal<String>,
) -> impl IntoView {
    view! {
        <div id="queues" class="gw-panel">
            <div class="flex items-center gap-2 border-b p-3" style="border-color: rgb(var(--border));">
                <span class="i-lucide-git-branch h-4 w-4"></span>
                <h3 class="font-semibold">"Queues"</h3>
            </div>
            <div class="divide-y" style="border-color: rgb(var(--border));">
                {move || if overview.get().queues.is_empty() {
                    view! { <div class="p-4 gw-muted">"No queues yet."</div> }.into_any()
                } else {
                    ().into_any()
                }}
                <For
                    each=move || overview.get().queues
                    key=|queue| queue.id
                    children=move |queue| {
                        let queue_name = queue.queue_name.clone();
                        view! {
                            <div class="flex items-center justify-between gap-3 p-3">
                                <div class="min-w-0">
                                    <button class="truncate font-medium hover:underline" type="button" on:click=move |_| {
                                        queue_filter.set(queue_name.clone());
                                        if let Some(window) = web_sys::window() {
                                            let _ = window.location().set_hash("jobs");
                                        }
                                    }>{queue.queue_name.clone()}</button>
                                    <p class="gw-muted text-xs">{queue.locked_by.clone().map(|worker| format!("locked by {worker}")).unwrap_or_else(|| "unlocked".to_string())}</p>
                                </div>
                                <div class="flex gap-2">
                                    <span class="gw-pill">{format!("{} ready", queue.ready_count)}</span>
                                    <span class="gw-pill">{format!("{} total", queue.job_count)}</span>
                                </div>
                            </div>
                        }
                    }
                />
            </div>
        </div>
    }
}

#[component]
pub(super) fn WorkersPanel(
    config: AdminClientConfig,
    token: RwSignal<String>,
    overview: RwSignal<OverviewResponse>,
    jobs: RwSignal<Vec<ListedJob>>,
    selected_jobs: RwSignal<Vec<i64>>,
    selected_workers: RwSignal<Vec<String>>,
    limit: RwSignal<i64>,
    toast: RwSignal<Option<String>>,
    refreshing: RwSignal<bool>,
    refresh_pending: RwSignal<bool>,
) -> impl IntoView {
    view! {
        <div id="workers" class="gw-panel">
            <div class="flex items-center justify-between gap-2 border-b p-3" style="border-color: rgb(var(--border));">
                <div class="flex items-center gap-2">
                    <span class="i-lucide-hard-drive h-4 w-4"></span>
                    <h3 class="font-semibold">"Workers"</h3>
                </div>
                <button class="gw-btn" type="button" disabled=config.read_only on:click={
                    let config = config.clone();
                    move |_| {
                        let worker_ids = selected_workers.get_untracked();
                        if worker_ids.is_empty() {
                            show_toast(toast, "Select at least one worker");
                            return;
                        }
                        post_maintenance(
                            config.clone(),
                            token,
                            MaintenanceRequest {
                                action: MaintenanceAction::ForceUnlock,
                                cleanup_tasks: Vec::new(),
                                worker_ids,
                            },
                            overview,
                            jobs,
                            selected_jobs,
                            limit,
                            toast,
                            refreshing,
                            refresh_pending,
                        );
                    }
                }>
                    <span class="i-lucide-unlock h-4 w-4"></span>"Force unlock"
                </button>
            </div>
            <div class="divide-y" style="border-color: rgb(var(--border));">
                {move || if overview.get().workers.is_empty() {
                    view! { <div class="p-4 gw-muted">"No active locks."</div> }.into_any()
                } else {
                    ().into_any()
                }}
                <For
                    each=move || overview.get().workers
                    key=|worker| worker.worker_id.clone()
                    children=move |worker| {
                        let worker_id = worker.worker_id.clone();
                        let worker_id_for_checked = worker_id.clone();
                        let worker_id_for_change = worker_id.clone();
                        view! {
                            <label class="flex cursor-pointer items-center justify-between gap-3 p-3">
                                <span class="min-w-0">
                                    <span class="block truncate font-mono text-sm">{worker.worker_id.clone()}</span>
                                    <span class="gw-muted text-xs">{format!("{} jobs · {} queues", worker.locked_jobs, worker.locked_queues)}</span>
                                </span>
                                <input
                                    class="h-4 w-4"
                                    type="checkbox"
                                    name="selected_workers"
                                    aria-label=format!("Select worker {}", worker.worker_id)
                                    prop:checked=move || selected_workers.get().contains(&worker_id_for_checked)
                                    on:change=move |event| {
                                        selected_workers.update(|selected| {
                                            if event_target_checked(&event) {
                                                if !selected.contains(&worker_id_for_change) {
                                                    selected.push(worker_id_for_change.clone());
                                                }
                                            } else {
                                                selected.retain(|candidate| candidate != &worker_id_for_change);
                                            }
                                        });
                                    }
                                />
                            </label>
                        }
                    }
                />
            </div>
        </div>
    }
}
