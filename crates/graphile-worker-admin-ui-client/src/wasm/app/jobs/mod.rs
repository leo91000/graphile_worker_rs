mod bulk_actions;
mod filters;
mod table;
mod toolbar;

use leptos::prelude::*;

use self::bulk_actions::JobBulkActions;
use self::filters::JobFilters;
use self::table::JobsTable;
use self::toolbar::JobsToolbar;
use super::super::api::RefreshSignals;
use super::super::types::{AdminClientConfig, JobState, ListedJob, Modal};

#[component]
pub(super) fn JobsPanel(
    config: AdminClientConfig,
    refresh: RefreshSignals,
    filtered_jobs: Memo<Vec<ListedJob>>,
    selected_count: Memo<usize>,
    all_visible_selected: Memo<bool>,
    active_state: RwSignal<JobState>,
    search: RwSignal<String>,
    task_filter: RwSignal<String>,
    queue_filter: RwSignal<String>,
    key_filter: RwSignal<String>,
    worker_filter: RwSignal<String>,
    modal: RwSignal<Option<Modal>>,
) -> impl IntoView {
    view! {
        <section id="jobs" class="gw-panel mt-4">
            <JobsToolbar
                config=config.clone()
                refresh=refresh
                selected_count=selected_count
                active_state=active_state
                modal=modal
            />
            <JobFilters
                config=config.clone()
                refresh=refresh
                search=search
                task_filter=task_filter
                queue_filter=queue_filter
                key_filter=key_filter
                worker_filter=worker_filter
            />
            <JobBulkActions
                config=config
                refresh=refresh
                selected_count=selected_count
                modal=modal
            />
            <JobsTable
                refresh=refresh
                filtered_jobs=filtered_jobs
                all_visible_selected=all_visible_selected
                modal=modal
            />
        </section>
    }
}
