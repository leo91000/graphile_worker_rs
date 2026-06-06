use leptos::prelude::*;

use super::api::refresh_data;
use super::browser::apply_theme;
use super::components::{ActiveWorkersPanel, Overview, QueuesPanel, WorkersPanel};
use super::filters::{filter_values, job_search_text, job_state, matches_column};
use super::modals::ModalView;
use super::types::{AdminClientConfig, JobState};

mod panels;
mod signals;
mod state;
use panels::{AuthTokenPanel, JobsPanel, MaintenancePanel, Sidebar, Topbar};
use signals::AdminSignals;

#[component]
pub(super) fn AdminApp(config: AdminClientConfig) -> impl IntoView {
    let signals = AdminSignals::new();
    let jobs = signals.jobs;
    let overview = signals.overview;
    let selected_jobs = signals.selected_jobs;
    let selected_workers = signals.selected_workers;
    let active_state = signals.active_state;
    let search = signals.search;
    let task_filter = signals.task_filter;
    let queue_filter = signals.queue_filter;
    let key_filter = signals.key_filter;
    let worker_filter = signals.worker_filter;
    let limit = signals.limit;
    let modal = signals.modal;
    let toast = signals.toast;
    let token = signals.token;
    let refreshing = signals.refreshing;
    let refresh_pending = signals.refresh_pending;
    let theme = signals.theme;
    let accent = signals.accent;
    let compact = signals.compact;
    let auto_refresh_enabled = signals.auto_refresh_enabled;
    let auto_refresh_timer = signals.auto_refresh_timer;

    let filtered_jobs = Memo::new(move |_| {
        let search = search.get().trim().to_lowercase();
        let active_state = active_state.get();
        let filters = [
            ("task_identifier", filter_values(&task_filter.get())),
            ("queue_name", filter_values(&queue_filter.get())),
            ("key", filter_values(&key_filter.get())),
            ("locked_by", filter_values(&worker_filter.get())),
        ];

        jobs.get()
            .into_iter()
            .filter(|job| {
                if active_state != JobState::All && job_state(job) != active_state {
                    return false;
                }
                if !search.is_empty() && !job_search_text(job).contains(&search) {
                    return false;
                }
                filters
                    .iter()
                    .all(|(column, values)| matches_column(job, column, values))
            })
            .collect::<Vec<_>>()
    });

    let selected_count = Memo::new(move |_| selected_jobs.get().len());
    let all_visible_selected = Memo::new(move |_| {
        let visible = filtered_jobs.get();
        let selected = selected_jobs.get();
        !visible.is_empty() && visible.iter().all(|job| selected.contains(&job.id))
    });
    let show_token_login = config.auth_mode.requires_token();

    Effect::new(move |_| {
        apply_theme(&theme.get(), &accent.get(), compact.get());
    });

    refresh_data(
        config.clone(),
        token,
        limit,
        overview,
        jobs,
        selected_jobs,
        toast,
        refreshing,
        refresh_pending,
    );

    view! {
        <Sidebar config=config.clone() />

        <main class="gw-main">
            <Topbar
                config=config.clone()
                token=token
                limit=limit
                overview=overview
                jobs=jobs
                selected_jobs=selected_jobs
                toast=toast
                refreshing=refreshing
                refresh_pending=refresh_pending
                theme=theme
                accent=accent
                compact=compact
                auto_refresh_enabled=auto_refresh_enabled
                auto_refresh_timer=auto_refresh_timer.clone()
            />

            <div class="gw-scroll">
                <AuthTokenPanel
                    show_token_login=show_token_login
                    config=config.clone()
                    token=token
                    limit=limit
                    overview=overview
                    jobs=jobs
                    selected_jobs=selected_jobs
                    toast=toast
                    refreshing=refreshing
                    refresh_pending=refresh_pending
                />

                <Overview stats=move || overview.get().stats />

                <JobsPanel
                    config=config.clone()
                    token=token
                    limit=limit
                    overview=overview
                    jobs=jobs
                    filtered_jobs=filtered_jobs
                    selected_jobs=selected_jobs
                    selected_count=selected_count
                    all_visible_selected=all_visible_selected
                    active_state=active_state
                    search=search
                    task_filter=task_filter
                    queue_filter=queue_filter
                    key_filter=key_filter
                    worker_filter=worker_filter
                    modal=modal
                    toast=toast
                    refreshing=refreshing
                    refresh_pending=refresh_pending
                />

                <section id="queues-workers" class="mt-4 grid gap-4 lg:grid-cols-2">
                    <QueuesPanel overview=overview queue_filter=queue_filter />
                    <div class="grid gap-4">
                        <ActiveWorkersPanel
                            config=config.clone()
                            token=token
                            overview=overview
                            jobs=jobs
                            selected_jobs=selected_jobs
                            limit=limit
                            toast=toast
                            refreshing=refreshing
                            refresh_pending=refresh_pending
                        />
                        <WorkersPanel
                            config=config.clone()
                            token=token
                            overview=overview
                            jobs=jobs
                            selected_jobs=selected_jobs
                            selected_workers=selected_workers
                            limit=limit
                            toast=toast
                            refreshing=refreshing
                            refresh_pending=refresh_pending
                        />
                    </div>
                </section>

                <MaintenancePanel
                    config=config.clone()
                    token=token
                    limit=limit
                    overview=overview
                    jobs=jobs
                    selected_jobs=selected_jobs
                    toast=toast
                    refreshing=refreshing
                    refresh_pending=refresh_pending
                />
            </div>
        </main>

        <ModalView
            modal=modal
            config=config
            token=token
            overview=overview
            jobs=jobs
            selected_jobs=selected_jobs
            limit=limit
            toast=toast
            refreshing=refreshing
            refresh_pending=refresh_pending
        />
        <div class="gw-toast" data-open=move || toast.get().is_some().to_string()>{move || toast.get().unwrap_or_default()}</div>
    }
}
