use leptos::prelude::*;
use serde_json::Value;

use super::super::api::{post_add_job, RefreshSignals};
use super::super::browser::show_toast;
use super::super::filters::{
    datetime_local_to_utc, optional_csv, optional_i16, optional_string, textarea_value,
};
use super::super::types::{AddJobRequest, AdminClientConfig, JobKeyModeRequest, Modal};
use super::form::{ModalButtons, TextInput};

#[component]
pub(super) fn AddJobModal(
    config: AdminClientConfig,
    modal: RwSignal<Option<Modal>>,
    refresh: RefreshSignals,
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
                    show_toast(refresh.toast, format!("Invalid JSON: {error}"));
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
                request,
                refresh,
                modal,
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
