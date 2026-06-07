use leptos::prelude::*;

use crate::wasm::api::RefreshSignals;
use crate::wasm::components::JobRow;
use crate::wasm::types::{ListedJob, Modal};

#[component]
pub(super) fn JobsTable(
    refresh: RefreshSignals,
    filtered_jobs: Memo<Vec<ListedJob>>,
    all_visible_selected: Memo<bool>,
    modal: RwSignal<Option<Modal>>,
) -> impl IntoView {
    let selected_jobs = refresh.selected_jobs;
    let toast = refresh.toast;

    view! {
        <div class="max-h-[58vh] overflow-auto">
            <table class="gw-table">
                <thead>
                    <tr>
                        <th>
                            <input
                                id="select-all-jobs"
                                name="select_all_jobs"
                                type="checkbox"
                                class="h-4 w-4"
                                prop:checked=move || all_visible_selected.get()
                                on:change=move |event| {
                                    let visible_ids = filtered_jobs.get_untracked().into_iter().map(|job| job.id).collect::<Vec<_>>();
                                    selected_jobs.update(|selected| {
                                        if event_target_checked(&event) {
                                            for id in visible_ids {
                                                if !selected.contains(&id) {
                                                    selected.push(id);
                                                }
                                            }
                                        } else {
                                            selected.retain(|id| !visible_ids.contains(id));
                                        }
                                    });
                                }
                            />
                        </th>
                        <th>"ID"</th><th>"Task"</th><th>"Queue"</th><th>"State"</th><th>"Run at"</th><th>"Attempts"</th><th>"Priority"</th><th>"Key"</th><th>"Payload"</th><th>"Error"</th><th>"Actions"</th>
                    </tr>
                </thead>
                <tbody>
                    {move || if filtered_jobs.get().is_empty() {
                        view! { <tr><td colspan="12" class="py-8 text-center gw-muted">"No jobs match the current filters."</td></tr> }.into_any()
                    } else {
                        ().into_any()
                    }}
                    <For
                        each=move || filtered_jobs.get()
                        key=|job| job.id
                        children=move |job| {
                            view! {
                                <JobRow
                                    job=job
                                    selected_jobs=selected_jobs
                                    modal=modal
                                    toast=toast
                                />
                            }
                        }
                    />
                </tbody>
            </table>
        </div>
    }
}
