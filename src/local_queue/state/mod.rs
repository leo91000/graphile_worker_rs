mod access;
mod mode;

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::Arc;
use std::time::Duration;

use graphile_worker_database::{Database, Schema};
use graphile_worker_job::Job;
use graphile_worker_lifecycle_hooks::{HookRegistry, LocalQueueMode};
use graphile_worker_runtime as runtime;

use crate::background_tasks::TaskSlot;
use graphile_worker_queries::task_identifiers::SharedTaskDetails;

use super::{LocalQueueConfig, LocalQueueParams, LocalQueueSignalSender};

pub(super) struct RefetchDelayState {
    pub(super) active: AtomicBool,
    pub(super) fetch_on_complete: AtomicBool,
    pub(super) counter: AtomicUsize,
    pub(super) abort_threshold: runtime::RwLock<usize>,
    pub(super) abort_notify: runtime::Notify,
}

impl Default for RefetchDelayState {
    fn default() -> Self {
        Self {
            active: AtomicBool::new(false),
            fetch_on_complete: AtomicBool::new(false),
            counter: AtomicUsize::new(0),
            abort_threshold: runtime::RwLock::new(usize::MAX),
            abort_notify: runtime::Notify::new(),
        }
    }
}

pub(super) struct LocalQueueState {
    pub(super) mode: runtime::RwLock<LocalQueueMode>,
    pub(super) job_queue: runtime::Mutex<VecDeque<Job>>,
    pub(super) job_signal_sender: LocalQueueSignalSender,
    pub(super) fetch_in_progress: AtomicBool,
    pub(super) fetch_again: AtomicBool,
    pub(super) refetch_delay: RefetchDelayState,
    pub(super) state_notify: runtime::Notify,
    pub(super) run_task: TaskSlot,
    pub(super) shutdown_task: TaskSlot,
    pub(super) refetch_delay_task: TaskSlot,
    pub(super) ttl_timer_task: TaskSlot,
    pub(super) run_complete_notify: runtime::Notify,
    pub(super) config: LocalQueueConfig,
    pub(super) database: Database,
    pub(super) schema: Schema,
    pub(super) worker_id: String,
    pub(super) task_details: SharedTaskDetails,
    pub(super) poll_interval: Duration,
    pub(super) continuous: bool,
    pub(super) hooks: Arc<HookRegistry>,
    pub(super) use_local_time: bool,
}

impl LocalQueueState {
    pub(super) fn new(params: LocalQueueParams) -> Self {
        Self {
            mode: runtime::RwLock::new(LocalQueueMode::Starting),
            job_queue: runtime::Mutex::new(VecDeque::new()),
            job_signal_sender: params.job_signal_sender,
            fetch_in_progress: AtomicBool::new(false),
            fetch_again: AtomicBool::new(false),
            refetch_delay: RefetchDelayState::default(),
            state_notify: runtime::Notify::new(),
            run_task: TaskSlot::empty("local_queue_run"),
            shutdown_task: TaskSlot::empty("local_queue_shutdown"),
            refetch_delay_task: TaskSlot::empty("local_queue_refetch_delay"),
            ttl_timer_task: TaskSlot::empty("local_queue_ttl"),
            run_complete_notify: runtime::Notify::new(),
            config: params.config,
            database: params.database,
            schema: params.schema,
            worker_id: params.worker_id,
            task_details: params.task_details,
            poll_interval: params.poll_interval,
            continuous: params.continuous,
            hooks: params.hooks,
            use_local_time: params.use_local_time,
        }
    }
}
