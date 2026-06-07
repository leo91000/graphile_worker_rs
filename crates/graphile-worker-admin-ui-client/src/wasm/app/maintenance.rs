use leptos::prelude::*;

use super::super::api::{post_maintenance, RefreshSignals};
use super::super::types::{
    AdminClientConfig, CleanupTaskName, MaintenanceAction, MaintenanceRequest,
};

#[component]
pub(super) fn MaintenancePanel(
    config: AdminClientConfig,
    refresh: RefreshSignals,
) -> impl IntoView {
    view! {
    <section id="maintenance" class="gw-panel mt-4 p-4">
        <div class="flex flex-wrap items-center justify-between gap-3">
            <div>
                <h3 class="font-semibold">"Maintenance"</h3>
                <p class="gw-muted text-sm">"Run migrations, cleanup orphaned queue metadata, and recover abandoned locks."</p>
            </div>
            <div class="flex flex-wrap gap-2">
                <button class="gw-btn" type="button" disabled=config.read_only on:click={
                    let config = config.clone();
                    move |_| post_maintenance(
                        config.clone(),
                        MaintenanceRequest {
                            action: MaintenanceAction::Migrate,
                            cleanup_tasks: Vec::new(),
                            worker_ids: Vec::new(),
                            dry_run: false,
                            sweep_threshold_secs: None,
                            recovery_delay_secs: None,
                        },
                        refresh,
                    )
                }>
                    <span class="i-lucide-database-zap h-4 w-4"></span>"Migrate"
                </button>
                <button class="gw-btn" type="button" disabled=config.read_only on:click={
                    let config = config.clone();
                    move |_| post_maintenance(
                        config.clone(),
                        MaintenanceRequest {
                            action: MaintenanceAction::Cleanup,
                            cleanup_tasks: vec![
                                CleanupTaskName::DeletePermanentlyFailedJobs,
                                CleanupTaskName::GcTaskIdentifiers,
                                CleanupTaskName::GcJobQueues,
                            ],
                            worker_ids: Vec::new(),
                            dry_run: false,
                            sweep_threshold_secs: None,
                            recovery_delay_secs: None,
                        },
                        refresh,
                    )
                }>
                    <span class="i-lucide-sparkles h-4 w-4"></span>"Cleanup"
                </button>
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
                    <span class="i-lucide-radar h-4 w-4"></span>"Sweep stale workers"
                </button>
            </div>
        </div>
    </section>
        }
}
