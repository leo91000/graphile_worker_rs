use leptos::prelude::*;

use crate::wasm::api::{refresh_data, RefreshSignals};
use crate::wasm::components::ColumnFilter;
use crate::wasm::types::AdminClientConfig;

#[component]
pub(super) fn JobFilters(
    config: AdminClientConfig,
    refresh: RefreshSignals,
    search: RwSignal<String>,
    task_filter: RwSignal<String>,
    queue_filter: RwSignal<String>,
    key_filter: RwSignal<String>,
    worker_filter: RwSignal<String>,
) -> impl IntoView {
    let limit = refresh.limit;

    view! {
        <div class="grid gap-2 border-b p-3 lg:grid-cols-[minmax(240px,1fr)_repeat(4,minmax(120px,180px))_110px]" style="border-color: rgb(var(--border));">
            <input id="global-search" name="global_search" class="gw-input" type="search" placeholder="Search id, task, queue, key, worker, payload..." prop:value=move || search.get() on:input=move |event| search.set(event_target_value(&event)) />
            <ColumnFilter name="task_filter" placeholder="Task filter" value=task_filter />
            <ColumnFilter name="queue_filter" placeholder="Queue filter" value=queue_filter />
            <ColumnFilter name="key_filter" placeholder="Key filter" value=key_filter />
            <ColumnFilter name="worker_filter" placeholder="Worker filter" value=worker_filter />
            <select
                id="limit-select"
                name="limit"
                class="gw-input"
                prop:value=move || limit.get().to_string()
                on:change=move |event| {
                    limit.set(event_target_value(&event).parse().unwrap_or(100));
                    refresh_data(config.clone(), refresh);
                }
            >
                <option value="50">"50"</option>
                <option value="100">"100"</option>
                <option value="250">"250"</option>
                <option value="500">"500"</option>
            </select>
        </div>
    }
}
