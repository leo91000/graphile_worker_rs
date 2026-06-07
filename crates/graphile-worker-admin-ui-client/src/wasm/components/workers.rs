use leptos::prelude::*;

use super::super::api::{post_maintenance, RefreshSignals};
use super::super::browser::show_toast;
use super::super::filters::format_date;
use super::super::types::{AdminClientConfig, MaintenanceAction, MaintenanceRequest};

#[component]
pub(in crate::wasm) fn ActiveWorkersPanel(
    config: AdminClientConfig,
    refresh: RefreshSignals,
) -> impl IntoView {
    view! {
        <div id="active-workers" class="gw-panel">
            <div class="flex items-center justify-between gap-2 border-b p-3" style="border-color: rgb(var(--border));">
                <div class="flex items-center gap-2">
                    <span class="i-lucide-heart-pulse h-4 w-4"></span>
                    <h3 class="font-semibold">"Active workers"</h3>
                </div>
                <button class="gw-btn" type="button" disabled=config.read_only on:click={
                    let config = config.clone();
                    move |_| post_maintenance(
                        config.clone(),
                        MaintenanceRequest {
                            action: MaintenanceAction::SweepStaleWorkers,
                            cleanup_tasks: Vec::new(),
                            worker_ids: Vec::new(),
                            dry_run: false,
                            sweep_threshold_secs: None,
                            recovery_delay_secs: None,
                        },
                        refresh,
                    )
                }>
                    <span class="i-lucide-radar h-4 w-4"></span>"Sweep stale"
                </button>
            </div>
            <div class="divide-y" style="border-color: rgb(var(--border));">
                {move || if refresh.overview.get().active_workers.is_empty() {
                    view! { <div class="p-4 gw-muted">"No registered workers yet."</div> }.into_any()
                } else {
                    ().into_any()
                }}
                <For
                    each=move || refresh.overview.get().active_workers
                    key=|worker| worker.worker_id.clone()
                    children=move |worker| {
                        let worker_id = worker.worker_id.clone();
                        view! {
                            <div class="flex items-center justify-between gap-3 p-3">
                                <div class="min-w-0">
                                    <span class="block truncate font-mono text-sm">{worker_id.clone()}</span>
                                    <p class="gw-muted text-xs">
                                        {format!("started {}", format_date(worker.started_at))}
                                        " · heartbeat "
                                        {format_date(worker.last_heartbeat_at)}
                                    </p>
                                </div>
                                <span class=format!(
                                    "gw-pill {}",
                                    if worker.is_stale { "text-amber-600 dark:text-amber-300" } else { "text-emerald-600 dark:text-emerald-300" }
                                )>
                                    {if worker.is_stale { "stale" } else { "healthy" }}
                                </span>
                            </div>
                        }
                    }
                />
            </div>
        </div>
    }
}

#[component]
pub(in crate::wasm) fn WorkersPanel(
    config: AdminClientConfig,
    selected_workers: RwSignal<Vec<String>>,
    refresh: RefreshSignals,
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
                            show_toast(refresh.toast, "Select at least one worker");
                            return;
                        }
                        post_maintenance(
                            config.clone(),
                            MaintenanceRequest {
                                action: MaintenanceAction::ForceUnlock,
                                cleanup_tasks: Vec::new(),
                                worker_ids,
                                dry_run: false,
                                sweep_threshold_secs: None,
                                recovery_delay_secs: None,
                            },
                            refresh,
                        );
                    }
                }>
                    <span class="i-lucide-unlock h-4 w-4"></span>"Force unlock"
                </button>
            </div>
            <div class="divide-y" style="border-color: rgb(var(--border));">
                {move || if refresh.overview.get().workers.is_empty() {
                    view! { <div class="p-4 gw-muted">"No active locks."</div> }.into_any()
                } else {
                    ().into_any()
                }}
                <For
                    each=move || refresh.overview.get().workers
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
