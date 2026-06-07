use leptos::prelude::*;
use web_sys::ClipboardEvent;

use super::super::browser::copy_to_clipboard;
use super::super::filters::{
    format_date, job_state, normalize_filter_paste, short, state_color, state_label,
    stringify_value,
};
use super::super::types::{JobState, ListedJob, Modal};

#[component]
pub(in crate::wasm) fn StateTab(
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
pub(in crate::wasm) fn ColumnFilter(
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
pub(in crate::wasm) fn JobRow(
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
