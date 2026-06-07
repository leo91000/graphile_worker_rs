use leptos::prelude::*;

use super::super::browser::copy_to_clipboard;
use super::super::filters::{job_state, state_label};
use super::super::types::ListedJob;

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
