use leptos::prelude::*;

use super::super::types::OverviewResponse;

#[component]
pub(in crate::wasm) fn QueuesPanel(
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
