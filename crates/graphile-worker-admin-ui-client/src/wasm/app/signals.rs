use std::cell::RefCell;
use std::rc::Rc;

use gloo_timers::callback::Interval;
use leptos::prelude::*;

use super::super::api::RefreshSignals;
use super::state::{empty_overview, stored_accent, stored_compact_density, stored_theme};
use crate::wasm::types::{JobState, ListedJob, Modal};

#[derive(Clone)]
pub(super) struct AdminSignals {
    pub(super) jobs: RwSignal<Vec<ListedJob>>,
    pub(super) overview: RwSignal<crate::wasm::types::OverviewResponse>,
    pub(super) selected_jobs: RwSignal<Vec<i64>>,
    pub(super) selected_workers: RwSignal<Vec<String>>,
    pub(super) active_state: RwSignal<JobState>,
    pub(super) search: RwSignal<String>,
    pub(super) task_filter: RwSignal<String>,
    pub(super) queue_filter: RwSignal<String>,
    pub(super) key_filter: RwSignal<String>,
    pub(super) worker_filter: RwSignal<String>,
    pub(super) limit: RwSignal<i64>,
    pub(super) modal: RwSignal<Option<Modal>>,
    pub(super) toast: RwSignal<Option<String>>,
    pub(super) token: RwSignal<String>,
    pub(super) refreshing: RwSignal<bool>,
    pub(super) refresh_pending: RwSignal<bool>,
    pub(super) theme: RwSignal<String>,
    pub(super) accent: RwSignal<String>,
    pub(super) compact: RwSignal<bool>,
    pub(super) auto_refresh_enabled: RwSignal<bool>,
    pub(super) auto_refresh_timer: Rc<RefCell<Option<Interval>>>,
}

impl AdminSignals {
    pub(super) fn new() -> Self {
        Self {
            jobs: RwSignal::new(Vec::new()),
            overview: RwSignal::new(empty_overview()),
            selected_jobs: RwSignal::new(Vec::new()),
            selected_workers: RwSignal::new(Vec::new()),
            active_state: RwSignal::new(JobState::All),
            search: RwSignal::new(String::new()),
            task_filter: RwSignal::new(String::new()),
            queue_filter: RwSignal::new(String::new()),
            key_filter: RwSignal::new(String::new()),
            worker_filter: RwSignal::new(String::new()),
            limit: RwSignal::new(100),
            modal: RwSignal::new(None),
            toast: RwSignal::new(None),
            token: RwSignal::new(
                super::super::browser::storage_get("gw-admin-token").unwrap_or_default(),
            ),
            refreshing: RwSignal::new(false),
            refresh_pending: RwSignal::new(false),
            theme: RwSignal::new(stored_theme()),
            accent: RwSignal::new(stored_accent()),
            compact: RwSignal::new(stored_compact_density()),
            auto_refresh_enabled: RwSignal::new(false),
            auto_refresh_timer: Rc::new(RefCell::new(None)),
        }
    }

    pub(super) fn refresh(&self) -> RefreshSignals {
        RefreshSignals {
            token: self.token,
            limit: self.limit,
            overview: self.overview,
            jobs: self.jobs,
            selected_jobs: self.selected_jobs,
            toast: self.toast,
            refreshing: self.refreshing,
            refresh_pending: self.refresh_pending,
        }
    }
}
