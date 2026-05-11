use std::cell::RefCell;
use std::rc::Rc;

use gloo_timers::callback::Interval;
use leptos::prelude::*;

use super::api::{post_job_action, post_maintenance, refresh_data};
use super::browser::{apply_theme, copy_to_clipboard, storage_get, storage_remove, storage_set};
use super::components::{ColumnFilter, JobRow, Overview, QueuesPanel, StateTab, WorkersPanel};
use super::filters::{
    filter_values, job_search_text, job_state, matches_column, selected_csv, selected_rows,
};
use super::modals::ModalView;
use super::types::{
    AdminClientConfig, CleanupTaskName, JobAction, JobActionRequest, JobState, ListedJob,
    MaintenanceAction, MaintenanceRequest, Modal, OverviewResponse,
};

#[component]
pub(super) fn AdminApp(config: AdminClientConfig) -> impl IntoView {
    let jobs = RwSignal::new(Vec::<ListedJob>::new());
    let overview = RwSignal::new(OverviewResponse {
        stats: Default::default(),
        queues: Vec::new(),
        workers: Vec::new(),
    });
    let selected_jobs = RwSignal::new(Vec::<i64>::new());
    let selected_workers = RwSignal::new(Vec::<String>::new());
    let active_state = RwSignal::new(JobState::All);
    let search = RwSignal::new(String::new());
    let task_filter = RwSignal::new(String::new());
    let queue_filter = RwSignal::new(String::new());
    let key_filter = RwSignal::new(String::new());
    let worker_filter = RwSignal::new(String::new());
    let limit = RwSignal::new(100_i64);
    let modal = RwSignal::new(None::<Modal>);
    let toast = RwSignal::new(None::<String>);
    let token = RwSignal::new(storage_get("gw-admin-token").unwrap_or_default());
    let refreshing = RwSignal::new(false);
    let refresh_pending = RwSignal::new(false);
    let theme =
        RwSignal::new(storage_get("gw-admin-theme").unwrap_or_else(|| "system".to_string()));
    let accent =
        RwSignal::new(storage_get("gw-admin-accent").unwrap_or_else(|| "cyan".to_string()));
    let compact = RwSignal::new(storage_get("gw-admin-density").as_deref() == Some("compact"));
    let auto_refresh_enabled = RwSignal::new(false);
    let auto_refresh_timer = Rc::new(RefCell::new(None::<Interval>));

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

    let refresh_click = {
        let config = config.clone();
        move |_| {
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
        }
    };

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
        <aside class="gw-sidebar">
            <div class="flex items-center gap-3">
                <div class="flex h-10 w-10 items-center justify-center rounded-lg bg-cyan-600 text-white">
                    <span class="i-lucide-workflow h-5 w-5"></span>
                </div>
                <div>
                    <h1 class="text-base font-semibold">"Graphile Worker"</h1>
                    <p class="gw-muted text-xs">"Admin UI"</p>
                </div>
            </div>

            <nav class="mt-6 grid gap-1">
                <a class="gw-tab" href="#jobs" aria-selected="true"><span class="i-lucide-list-checks h-4 w-4"></span>"Jobs"</a>
                <a class="gw-tab" href="#queues"><span class="i-lucide-git-branch h-4 w-4"></span>"Queues"</a>
                <a class="gw-tab" href="#workers"><span class="i-lucide-hard-drive h-4 w-4"></span>"Workers"</a>
                <a class="gw-tab" href="#maintenance"><span class="i-tabler-tool h-4 w-4"></span>"Maintenance"</a>
            </nav>

            <div class="mt-auto grid gap-3 rounded-lg border p-3 text-xs" style="border-color: rgb(var(--border));">
                <div class="flex items-center justify-between"><span class="gw-muted">"Schema"</span><span class="font-mono">{config.schema.clone()}</span></div>
                <div class="flex items-center justify-between"><span class="gw-muted">"Auth"</span><span class="gw-pill">{config.auth_mode.as_str()}</span></div>
                <div class="flex items-center justify-between"><span class="gw-muted">"Writes"</span><span class="gw-pill">{if config.read_only { "read only" } else { "enabled" }}</span></div>
            </div>
        </aside>

        <main class="gw-main">
            <header class="gw-topbar">
                <div class="min-w-0">
                    <p class="gw-muted text-xs">"PostgreSQL-backed queue control plane"</p>
                    <h2 class="truncate text-lg font-semibold">"Jobs, queues, and workers"</h2>
                </div>
                <div class="flex flex-wrap items-center justify-end gap-2">
                    <select
                        id="theme-select"
                        name="theme"
                        class="gw-input w-32"
                        aria-label="Theme"
                        prop:value=move || theme.get()
                        on:change=move |event| {
                            let value = event_target_value(&event);
                            storage_set("gw-admin-theme", &value);
                            theme.set(value);
                        }
                    >
                        <option value="system">"System"</option>
                        <option value="light">"Light"</option>
                        <option value="dark">"Dark"</option>
                    </select>
                    <select
                        id="accent-select"
                        name="accent"
                        class="gw-input w-32"
                        aria-label="Accent"
                        prop:value=move || accent.get()
                        on:change=move |event| {
                            let value = event_target_value(&event);
                            storage_set("gw-admin-accent", &value);
                            accent.set(value);
                        }
                    >
                        <option value="cyan">"Cyan"</option>
                        <option value="emerald">"Emerald"</option>
                        <option value="violet">"Violet"</option>
                        <option value="amber">"Amber"</option>
                    </select>
                    <button
                        class="gw-btn"
                        type="button"
                        title="Toggle density"
                        on:click=move |_| {
                            let next = !compact.get_untracked();
                            storage_set("gw-admin-density", if next { "compact" } else { "comfortable" });
                            compact.set(next);
                        }
                    >
                        <span class="i-lucide-align-justify h-4 w-4"></span>
                    </button>
                    <label class="gw-btn cursor-pointer">
                        <input
                            id="auto-refresh"
                            name="auto_refresh"
                            class="h-4 w-4"
                            type="checkbox"
                            prop:checked=move || auto_refresh_enabled.get()
                            on:change={
                                let config = config.clone();
                                let auto_refresh_timer = auto_refresh_timer.clone();
                                move |event| {
                                    if event_target_checked(&event) {
                                        auto_refresh_enabled.set(true);
                                        if auto_refresh_timer.borrow().is_some() {
                                            return;
                                        }
                                        let config = config.clone();
                                        let handle = Interval::new(5000, move || {
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
                                        });
                                        *auto_refresh_timer.borrow_mut() = Some(handle);
                                    } else {
                                        auto_refresh_enabled.set(false);
                                        *auto_refresh_timer.borrow_mut() = None;
                                    }
                                }
                            }
                        />
                        <span class="text-sm">"Auto"</span>
                    </label>
                    <button class="gw-btn gw-btn-primary" type="button" on:click=refresh_click>
                        <span class="i-lucide-refresh-cw h-4 w-4"></span>
                        "Refresh"
                    </button>
                </div>
            </header>

            <div class="gw-scroll">
                <section class=if show_token_login { "gw-panel mb-4 p-4" } else { "hidden" }>
                    <div class="flex flex-wrap items-end gap-3">
                        <div class="min-w-72 flex-1">
                            <label class="mb-1 block text-sm font-medium" for="auth-token">"API token"</label>
                            <input
                                id="auth-token"
                                class="gw-input w-full"
                                type="password"
                                autocomplete="current-password"
                                prop:value=move || token.get()
                                on:input=move |event| token.set(event_target_value(&event))
                            />
                        </div>
                        <button
                            class="gw-btn gw-btn-primary"
                            type="button"
                            on:click={
                                let config = config.clone();
                                move |_| {
                                    storage_set("gw-admin-token", &token.get_untracked());
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
                                }
                            }
                        >
                            <span class="i-lucide-key-round h-4 w-4"></span>
                            "Use token"
                        </button>
                        <button class="gw-btn" type="button" on:click=move |_| {
                            storage_remove("gw-admin-token");
                            token.set(String::new());
                        }>"Clear"</button>
                    </div>
                </section>

                <Overview stats=move || overview.get().stats />

                <section id="jobs" class="gw-panel mt-4">
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
                            on:change={
                                let config = config.clone();
                                move |event| {
                                    limit.set(event_target_value(&event).parse().unwrap_or(100));
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
                                }
                            }
                        >
                            <option value="50">"50"</option>
                            <option value="100">"100"</option>
                            <option value="250">"250"</option>
                            <option value="500">"500"</option>
                        </select>
                    </div>

                    <div class="flex flex-wrap items-center gap-2 border-b p-3" style="border-color: rgb(var(--border));">
                        <span class="gw-pill">{move || format!("{} selected", selected_count.get())}</span>
                        <button class="gw-btn" type="button" disabled=move || selected_count.get() == 0 || config.read_only on:click={
                            let config = config.clone();
                            move |_| post_job_action(
                                config.clone(),
                                token,
                                JobActionRequest {
                                    action: JobAction::Complete,
                                    ids: selected_jobs.get_untracked(),
                                    reason: None,
                                    run_at: None,
                                    priority: None,
                                    attempts: None,
                                    max_attempts: None,
                                },
                                overview,
                                jobs,
                                selected_jobs,
                                limit,
                                None,
                                toast,
                                refreshing,
                                refresh_pending,
                            )
                        }>
                            <span class="i-lucide-check h-4 w-4"></span>"Complete"
                        </button>
                        <button class="gw-btn" type="button" disabled=move || selected_count.get() == 0 || config.read_only on:click={
                            let config = config.clone();
                            move |_| post_job_action(
                                config.clone(),
                                token,
                                JobActionRequest {
                                    action: JobAction::RunNow,
                                    ids: selected_jobs.get_untracked(),
                                    reason: None,
                                    run_at: None,
                                    priority: None,
                                    attempts: None,
                                    max_attempts: None,
                                },
                                overview,
                                jobs,
                                selected_jobs,
                                limit,
                                None,
                                toast,
                                refreshing,
                                refresh_pending,
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
                            <span class="i-lucide-key-x h-4 w-4"></span>"Remove by key"
                        </button>
                    </div>

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
                </section>

                <section id="queues-workers" class="mt-4 grid gap-4 lg:grid-cols-2">
                    <QueuesPanel overview=overview queue_filter=queue_filter />
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
                </section>

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
                                    token,
                                    MaintenanceRequest {
                                        action: MaintenanceAction::Migrate,
                                        cleanup_tasks: Vec::new(),
                                        worker_ids: Vec::new(),
                                    },
                                    overview,
                                    jobs,
                                    selected_jobs,
                                    limit,
                                    toast,
                                    refreshing,
                                    refresh_pending,
                                )
                            }>
                                <span class="i-lucide-database-zap h-4 w-4"></span>"Migrate"
                            </button>
                            <button class="gw-btn" type="button" disabled=config.read_only on:click={
                                let config = config.clone();
                                move |_| post_maintenance(
                                    config.clone(),
                                    token,
                                    MaintenanceRequest {
                                        action: MaintenanceAction::Cleanup,
                                        cleanup_tasks: vec![
                                            CleanupTaskName::DeletePermanentlyFailedJobs,
                                            CleanupTaskName::GcTaskIdentifiers,
                                            CleanupTaskName::GcJobQueues,
                                        ],
                                        worker_ids: Vec::new(),
                                    },
                                    overview,
                                    jobs,
                                    selected_jobs,
                                    limit,
                                    toast,
                                    refreshing,
                                    refresh_pending,
                                )
                            }>
                                <span class="i-lucide-sparkles h-4 w-4"></span>"Cleanup"
                            </button>
                        </div>
                    </div>
                </section>
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
