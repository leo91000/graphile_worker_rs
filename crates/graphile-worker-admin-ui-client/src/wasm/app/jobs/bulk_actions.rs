use leptos::prelude::*;

use crate::wasm::api::{post_job_action, RefreshSignals};
use crate::wasm::types::{AdminClientConfig, JobAction, JobActionRequest, Modal};

#[component]
pub(super) fn JobBulkActions(
    config: AdminClientConfig,
    refresh: RefreshSignals,
    selected_count: Memo<usize>,
    modal: RwSignal<Option<Modal>>,
) -> impl IntoView {
    let selected_jobs = refresh.selected_jobs;

    view! {
        <div class="flex flex-wrap items-center gap-2 border-b p-3" style="border-color: rgb(var(--border));">
            <span class="gw-pill">{move || format!("{} selected", selected_count.get())}</span>
            <button class="gw-btn" type="button" disabled=move || selected_count.get() == 0 || config.read_only on:click={
                let config = config.clone();
                move |_| post_job_action(
                    config.clone(),
                    JobActionRequest {
                        action: JobAction::Complete,
                        ids: selected_jobs.get_untracked(),
                        reason: None,
                        run_at: None,
                        priority: None,
                        attempts: None,
                        max_attempts: None,
                    },
                    refresh,
                    None,
                )
            }>
                <span class="i-lucide-check h-4 w-4"></span>"Complete"
            </button>
            <button class="gw-btn" type="button" disabled=move || selected_count.get() == 0 || config.read_only on:click={
                let config = config.clone();
                move |_| post_job_action(
                    config.clone(),
                    JobActionRequest {
                        action: JobAction::RunNow,
                        ids: selected_jobs.get_untracked(),
                        reason: None,
                        run_at: None,
                        priority: None,
                        attempts: None,
                        max_attempts: None,
                    },
                    refresh,
                    None,
                )
            }>
                <span class="i-lucide-play h-4 w-4"></span>"Run now"
            </button>
            <button class="gw-btn" type="button" disabled=move || selected_count.get() == 0 || config.read_only on:click=move |_| modal.set(Some(Modal::Reschedule))>
                <span class="i-lucide-calendar-clock h-4 w-4"></span>"Reschedule"
            </button>
            <button class="gw-btn gw-btn-danger" type="button" disabled=move || selected_count.get() == 0 || config.read_only on:click=move |_| modal.set(Some(Modal::FailJobs))>
                <span class="i-lucide-ban h-4 w-4"></span>"Fail"
            </button>
            <button class="gw-btn" type="button" disabled=config.read_only on:click=move |_| modal.set(Some(Modal::RemoveKey))>
                <span class="i-tabler-key-off h-4 w-4"></span>"Remove by key"
            </button>
        </div>
    }
}
