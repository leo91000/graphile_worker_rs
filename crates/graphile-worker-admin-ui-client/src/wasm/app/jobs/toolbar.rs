use leptos::prelude::*;

use crate::wasm::api::RefreshSignals;
use crate::wasm::browser::copy_to_clipboard;
use crate::wasm::components::StateTab;
use crate::wasm::filters::{selected_csv, selected_rows};
use crate::wasm::types::{AdminClientConfig, JobState, Modal};

#[component]
pub(super) fn JobsToolbar(
    config: AdminClientConfig,
    refresh: RefreshSignals,
    selected_count: Memo<usize>,
    active_state: RwSignal<JobState>,
    modal: RwSignal<Option<Modal>>,
) -> impl IntoView {
    let jobs = refresh.jobs;
    let selected_jobs = refresh.selected_jobs;
    let toast = refresh.toast;

    let copy_selected_json = move |_| {
        let rows = selected_rows(jobs, selected_jobs);
        copy_to_clipboard(
            serde_json::to_string_pretty(&rows).unwrap_or_default(),
            "Copied selected JSON",
            toast,
        );
    };
    let copy_selected_csv = move |_| {
        copy_to_clipboard(
            selected_csv(&selected_rows(jobs, selected_jobs)),
            "Copied selected CSV",
            toast,
        );
    };

    view! {
        <div class="flex flex-wrap items-center justify-between gap-3 border-b p-3" style="border-color: rgb(var(--border));">
            <div class="flex flex-wrap items-center gap-2" role="tablist" aria-label="Job state">
                <StateTab label="All" state=JobState::All active_state=active_state />
                <StateTab label="Ready" state=JobState::Ready active_state=active_state />
                <StateTab label="Scheduled" state=JobState::Scheduled active_state=active_state />
                <StateTab label="Locked" state=JobState::Locked active_state=active_state />
                <StateTab label="Failed" state=JobState::Failed active_state=active_state />
            </div>
            <div class="flex flex-wrap items-center gap-2">
                <button class="gw-btn gw-btn-primary" type="button" disabled=config.read_only on:click=move |_| modal.set(Some(Modal::AddJob))>
                    <span class="i-lucide-plus h-4 w-4"></span>"Add"
                </button>
                <button class="gw-btn" type="button" disabled=move || selected_count.get() == 0 on:click=copy_selected_json>
                    <span class="i-lucide-copy h-4 w-4"></span>"JSON"
                </button>
                <button class="gw-btn" type="button" disabled=move || selected_count.get() == 0 on:click=copy_selected_csv>
                    <span class="i-lucide-clipboard h-4 w-4"></span>"CSV"
                </button>
            </div>
        </div>
    }
}
