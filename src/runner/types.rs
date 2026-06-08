use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use getset::Getters;
use graphile_worker_crontab_types::Crontab;
use graphile_worker_database::{Database, Schema};
use graphile_worker_extensions::ReadOnlyExtensions;
use graphile_worker_lifecycle_hooks::HookRegistry;
use graphile_worker_recovery::WorkerRecoveryConfig;
use graphile_worker_runtime as runtime;
use graphile_worker_shutdown_signal::ShutdownSignal;

use crate::WorkerShutdownConfig;
use graphile_worker_queries::task_identifiers::SharedTaskDetails;

/// Type alias for task handler functions.
///
/// A task handler is a closure that takes a `WorkerContext` and returns a future
/// that resolves to a `TaskHandlerOutcome`.
pub type WorkerFn = graphile_worker_task_handler::TaskHandlerFn;

/// The main worker struct that processes jobs from the queue.
///
/// The `Worker` is responsible for:
/// - Polling the database for new jobs
/// - Executing jobs with the appropriate task handlers
/// - Managing concurrency and job execution
/// - Processing cron jobs according to schedules
/// - Handling job failures and retries
/// - Providing utilities for job management
#[derive(Getters)]
#[getset(get = "pub")]
pub struct Worker {
    /// Unique identifier for this worker instance
    pub(crate) worker_id: String,
    /// Maximum number of jobs to process concurrently
    pub(crate) concurrency: usize,
    /// How often to poll for new jobs when no notifications are received
    pub(crate) poll_interval: Duration,
    /// Map of task identifiers to their handler functions
    pub(crate) jobs: HashMap<String, WorkerFn>,
    /// Database connection pool
    pub(crate) database: Database,
    /// Database schema where Graphile Worker tables are located.
    #[getset(skip)]
    pub(crate) schema: Schema,
    /// Mapping of task IDs to their string identifiers
    pub(crate) task_details: SharedTaskDetails,
    /// List of job flags that this worker will not process
    pub(crate) forbidden_flags: Vec<String>,
    /// List of cron job definitions to be scheduled
    pub(crate) crontabs: Vec<Crontab>,
    /// Whether to use local application time (true) or database time (false) for timestamps
    pub(crate) use_local_time: bool,
    /// Signal that can be triggered to gracefully shut down the worker
    pub(crate) shutdown_signal: ShutdownSignal,
    /// Internal notifier used to request shutdown programmatically
    #[getset(skip)]
    pub(crate) shutdown_notifier: Arc<runtime::Notify>,
    /// Extensions that can modify worker behavior
    pub(crate) extensions: ReadOnlyExtensions,
    /// Lifecycle hooks for observing and intercepting worker events
    pub(crate) hooks: Arc<HookRegistry>,
    /// Optional local queue config (LocalQueue created lazily in run())
    #[getset(skip)]
    pub(crate) local_queue_config: Option<crate::local_queue::LocalQueueConfig>,
    /// Optional completion batcher for batching job completions
    #[getset(skip)]
    pub(crate) completion_batcher: Option<Arc<crate::batcher::CompletionBatcher>>,
    /// Optional failure batcher for batching job failures
    #[getset(skip)]
    pub(crate) failure_batcher: Option<Arc<crate::batcher::FailureBatcher>>,
    /// Dead worker recovery configuration
    pub(crate) recovery_config: WorkerRecoveryConfig,
    /// Worker shutdown behavior
    pub(crate) shutdown_config: WorkerShutdownConfig,
}

#[derive(Clone)]
pub(crate) struct WorkerRunner {
    pub(crate) worker_id: String,
    pub(crate) jobs: HashMap<String, WorkerFn>,
    pub(crate) database: Database,
    pub(crate) schema: Schema,
    pub(crate) task_details: SharedTaskDetails,
    pub(crate) forbidden_flags: Vec<String>,
    pub(crate) use_local_time: bool,
    pub(crate) shutdown_signal: ShutdownSignal,
    pub(crate) extensions: ReadOnlyExtensions,
    pub(crate) hooks: Arc<HookRegistry>,
    pub(crate) completion_batcher: Option<Arc<crate::batcher::CompletionBatcher>>,
    pub(crate) failure_batcher: Option<Arc<crate::batcher::FailureBatcher>>,
    pub(crate) shutdown_config: WorkerShutdownConfig,
}
