use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;

use crate::local_queue::LocalQueue;
use crate::sql::{get_job::get_job, task_identifiers::SharedTaskDetails};
use crate::streams::{job_signal_stream, job_signal_stream_with_receiver, job_stream};
use crate::worker_utils::WorkerUtils;
use futures::{stream::FuturesUnordered, try_join, StreamExt};
use getset::Getters;
use graphile_worker_crontab_runner::cron_main;
use graphile_worker_crontab_types::Crontab;
use graphile_worker_database::Database;
use graphile_worker_extensions::ReadOnlyExtensions;
use graphile_worker_lifecycle_hooks::{
    HookRegistry, ShutdownReason, WorkerShutdownContext, WorkerStartContext,
};
use graphile_worker_runtime as runtime;
use graphile_worker_shutdown_signal::ShutdownSignal;
use tracing::{debug, error, info, warn};

use crate::builder::WorkerOptions;
use crate::recovery::WorkerRecoveryConfig;
use crate::recovery_tasks::{deregister_worker, register_worker, spawn_recovery_tasks};
use crate::streams::StreamSource;

mod errors;
mod job_execution;
mod recovery_release;
mod release;
mod sources;
#[cfg(test)]
mod tests;
use errors::ProcessJobError;
pub use errors::{ReleaseJobError, WorkerRuntimeError};
use job_execution::{process_one_job, run_and_release_job};

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
    /// Database schema name (properly escaped for SQL)
    pub(crate) escaped_schema: String,
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
    /// Worker crash and shutdown recovery configuration
    pub(crate) recovery_config: WorkerRecoveryConfig,
}

#[derive(Clone)]
struct WorkerRunner {
    worker_id: String,
    jobs: HashMap<String, WorkerFn>,
    database: Database,
    escaped_schema: String,
    task_details: SharedTaskDetails,
    forbidden_flags: Vec<String>,
    use_local_time: bool,
    shutdown_signal: ShutdownSignal,
    extensions: ReadOnlyExtensions,
    hooks: Arc<HookRegistry>,
    completion_batcher: Option<Arc<crate::batcher::CompletionBatcher>>,
    failure_batcher: Option<Arc<crate::batcher::FailureBatcher>>,
    recovery_config: WorkerRecoveryConfig,
}

impl Worker {
    /// Creates a new `WorkerOptions` builder with default settings.
    ///
    /// This is the starting point for configuring and creating a new worker.
    ///
    /// # Returns
    ///
    /// A default `WorkerOptions` instance that can be customized to configure
    /// the worker's behavior.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use graphile_worker::Worker;
    /// use std::time::Duration;
    /// use graphile_worker_ctx::WorkerContext;
    /// use serde::{Deserialize, Serialize};
    /// use graphile_worker::{TaskHandler, IntoTaskHandlerResult};
    ///
    /// #[derive(Deserialize, Serialize)]
    /// struct MyTask;
    ///
    /// impl TaskHandler for MyTask {
    ///     const IDENTIFIER: &'static str = "task_name";
    ///     async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
    ///         Ok::<(), String>(())
    ///     }
    /// }
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let worker = Worker::options()
    ///     .concurrency(5)
    ///     .poll_interval(Duration::from_secs(1))
    ///     .database_url("postgres://user:password@localhost/mydb")
    ///     .define_job::<MyTask>()
    ///     .init()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn options() -> WorkerOptions {
        WorkerOptions::default()
    }

    /// Returns the underlying SQLx PostgreSQL pool when the worker uses the SQLx driver.
    #[cfg(feature = "driver-sqlx")]
    pub fn try_pg_pool(&self) -> Option<&sqlx::PgPool> {
        self.database
            .downcast_ref::<graphile_worker_database::sqlx::SqlxDatabase>()
            .map(|database| database.pool())
    }

    /// Returns the underlying SQLx PostgreSQL pool.
    ///
    /// # Panics
    /// Panics if the worker was configured with a non-SQLx database driver.
    #[cfg(feature = "driver-sqlx")]
    pub fn pg_pool(&self) -> &sqlx::PgPool {
        self.try_pg_pool()
            .expect("Worker does not use the SQLx database driver")
    }

    /// Runs the worker until the shutdown signal is triggered.
    ///
    /// This method starts both the job runner and the crontab scheduler (if any crontabs are configured),
    /// and runs them concurrently. It continuously polls for new jobs and processes them as they become
    /// available, respecting the configured concurrency limit.
    ///
    /// The worker will continue running until the shutdown signal is triggered, at which point
    /// it will gracefully shut down after completing any in-progress jobs (with a timeout).
    ///
    /// # Returns
    ///
    /// A `Result` that is:
    /// - `Ok(())` if the worker shuts down gracefully
    /// - `Err(WorkerRuntimeError)` if an error occurs during worker execution
    pub async fn run(&self) -> Result<(), WorkerRuntimeError> {
        register_worker(self, None).await?;

        self.hooks
            .emit(WorkerStartContext {
                database: self.database.clone(),
                worker_id: self.worker_id.clone(),
                extensions: self.extensions.clone(),
            })
            .await;

        let recovery_tasks = spawn_recovery_tasks(self);

        let local_queue = self.create_local_queues();
        let job_runner = self.job_runner_internal(local_queue);
        let crontab_scheduler = self.crontab_scheduler();

        let result = try_join!(crontab_scheduler, job_runner);

        if let Some(batcher) = &self.completion_batcher {
            batcher.await_shutdown().await;
        }
        if let Some(batcher) = &self.failure_batcher {
            batcher.await_shutdown().await;
        }

        let reason = match &result {
            Ok(_) => ShutdownReason::Graceful,
            Err(_) => ShutdownReason::Error,
        };

        recovery_tasks.stop().await;

        self.hooks
            .emit(WorkerShutdownContext {
                database: self.database.clone(),
                worker_id: self.worker_id.clone(),
                reason,
            })
            .await;

        let deregister_result = deregister_worker(self).await;

        match (result, deregister_result) {
            (Ok(_), Ok(())) => Ok(()),
            (Ok(_), Err(error)) => Err(error.into()),
            (Err(error), Ok(())) => Err(error),
            (Err(error), Err(cleanup_error)) => {
                warn!(
                    error = %cleanup_error,
                    "Failed to deregister worker after runtime error"
                );
                Err(error)
            }
        }
    }

    pub(crate) fn clone_for_recovery(&self) -> Self {
        Self {
            worker_id: self.worker_id.clone(),
            concurrency: self.concurrency,
            poll_interval: self.poll_interval,
            jobs: self.jobs.clone(),
            database: self.database.clone(),
            escaped_schema: self.escaped_schema.clone(),
            task_details: self.task_details.clone(),
            forbidden_flags: self.forbidden_flags.clone(),
            crontabs: self.crontabs.clone(),
            use_local_time: self.use_local_time,
            shutdown_signal: self.shutdown_signal.clone(),
            shutdown_notifier: self.shutdown_notifier.clone(),
            extensions: self.extensions.clone(),
            hooks: self.hooks.clone(),
            local_queue_config: self.local_queue_config.clone(),
            completion_batcher: self.completion_batcher.clone(),
            failure_batcher: self.failure_batcher.clone(),
            recovery_config: self.recovery_config.clone(),
        }
    }

    /// Runs the worker once and processes all available jobs, then returns.
    ///
    /// Unlike the `run` method which continuously polls for new jobs, this method
    /// processes only the jobs that are currently available in the queue and then
    /// completes. It's useful for batch processing or one-time job execution.
    ///
    /// This method will process jobs up to the configured concurrency limit.
    /// An error in one job will not stop the processing of other jobs.
    ///
    /// If a job is part of a queue (has job_queue_id), the method will attempt to
    /// fetch and process subsequent jobs in the same queue until none are available.
    ///
    /// # Returns
    ///
    /// A `Result` that is:
    /// - `Ok(())` if all available jobs were processed (even if some failed)
    /// - `Err(WorkerRuntimeError)` if a system-level error occurs
    pub async fn run_once(&self) -> Result<(), WorkerRuntimeError> {
        let job_stream = job_stream(
            self.database.clone(),
            self.shutdown_signal.clone(),
            self.task_details.clone(),
            self.escaped_schema.clone(),
            self.worker_id.clone(),
            self.forbidden_flags.clone(),
            self.use_local_time,
        );

        let runner = self.runner();

        job_stream
            .for_each_concurrent(self.concurrency, {
                let runner = runner.clone();
                move |mut job| {
                    let runner = runner.clone();
                    async move {
                        loop {
                            let job_id = *job.id();
                            let has_queue = job.job_queue_id().is_some();
                            let result =
                                run_and_release_job(Arc::new(job), &runner, &StreamSource::RunOnce)
                                    .await;

                            match result {
                                Ok(_) => {
                                    info!(job_id, "Job processed");
                                }
                                Err(e) => {
                                    error!("Error while processing job : {:?}", e);
                                }
                            };

                            if !has_queue {
                                break;
                            }
                            info!(job_id, "Job has queue, fetching another job");
                            let now = runner.use_local_time.then(Utc::now);
                            let task_details_guard = runner.task_details.read().await;
                            let new_job = get_job(
                                &runner.database,
                                &task_details_guard,
                                &runner.escaped_schema,
                                &runner.worker_id,
                                &runner.forbidden_flags,
                                now,
                            )
                            .await
                            .unwrap_or(None);
                            drop(task_details_guard);
                            let Some(new_job) = new_job else {
                                break;
                            };
                            job = new_job;
                        }
                    }
                }
            })
            .await;

        Ok(())
    }

    /// Internal method that runs the continuous job processing loop.
    ///
    /// This method sets up a stream of job signals (either from database notifications
    /// or regular polling) and processes jobs as they become available, respecting
    /// the configured concurrency limit.
    ///
    /// # Returns
    ///
    /// A `Result` that is:
    /// - `Ok(())` if the job runner shuts down gracefully
    /// - `Err(WorkerRuntimeError)` if an error occurs during execution
    fn create_local_queues(&self) -> Option<(Vec<LocalQueue>, crate::streams::JobSignalReceiver)> {
        sources::create_local_queues(self)
    }

    fn runner(&self) -> WorkerRunner {
        WorkerRunner {
            worker_id: self.worker_id.clone(),
            jobs: self.jobs.clone(),
            database: self.database.clone(),
            escaped_schema: self.escaped_schema.clone(),
            task_details: self.task_details.clone(),
            forbidden_flags: self.forbidden_flags.clone(),
            use_local_time: self.use_local_time,
            shutdown_signal: self.shutdown_signal.clone(),
            extensions: self.extensions.clone(),
            hooks: self.hooks.clone(),
            completion_batcher: self.completion_batcher.clone(),
            failure_batcher: self.failure_batcher.clone(),
            recovery_config: self.recovery_config.clone(),
        }
    }

    async fn job_runner_internal(
        &self,
        local_queue: Option<(Vec<LocalQueue>, crate::streams::JobSignalReceiver)>,
    ) -> Result<(), WorkerRuntimeError> {
        match local_queue {
            Some((local_queues, rx)) => self.job_runner_with_local_queue(local_queues, rx).await,
            None => self.job_runner_direct().await,
        }
    }

    /// Job runner implementation using LocalQueue for batch-fetching jobs.
    async fn job_runner_with_local_queue(
        &self,
        local_queues: Vec<LocalQueue>,
        job_signal_rx: crate::streams::JobSignalReceiver,
    ) -> Result<(), WorkerRuntimeError> {
        let job_signal = job_signal_stream_with_receiver(
            self.database.clone(),
            self.poll_interval,
            self.shutdown_signal.clone(),
            1,
            job_signal_rx,
        )
        .await?;

        debug!("Listening for jobs with LocalQueue...");
        let (source_tx, source_rx) = runtime::channel(self.concurrency * 4);
        let worker_handles = FuturesUnordered::new();
        let runner = self.runner();
        let local_queues = Arc::new(local_queues);

        for index in 0..self.concurrency {
            let local_queues = local_queues.clone();
            let runner = runner.clone();
            let source_rx = source_rx.clone();
            worker_handles.push(runtime::spawn(async move {
                while let Ok(source) = source_rx.recv().await {
                    sources::process_local_queue_source(&runner, &local_queues, index, source)
                        .await?;
                }

                Ok::<(), ProcessJobError>(())
            }));
        }
        drop(source_rx);

        let dispatch_result =
            sources::dispatch_job_signals(job_signal, source_tx, worker_handles, self.concurrency)
                .await;

        for local_queue in local_queues.iter() {
            if let Err(e) = local_queue.release().await {
                warn!(error = %e, "Error releasing LocalQueue");
            }
        }

        dispatch_result?;
        Ok(())
    }

    /// Job runner implementation without LocalQueue (direct database queries).
    async fn job_runner_direct(&self) -> Result<(), WorkerRuntimeError> {
        let job_signal = job_signal_stream(
            self.database.clone(),
            self.poll_interval,
            self.shutdown_signal.clone(),
            1,
        )
        .await?;

        debug!("Listening for jobs...");
        let (source_tx, source_rx) = runtime::channel(self.concurrency * 4);
        let worker_handles = FuturesUnordered::new();
        let runner = self.runner();

        for _ in 0..self.concurrency {
            let runner = runner.clone();
            let source_rx = source_rx.clone();
            worker_handles.push(runtime::spawn(async move {
                while let Ok(source) = source_rx.recv().await {
                    let res = process_one_job(&runner, source).await?;

                    if let Some(job) = res {
                        debug!(job_id = job.id(), "Job processed");
                    }
                }

                Ok::<(), ProcessJobError>(())
            }));
        }
        drop(source_rx);

        sources::dispatch_job_signals(job_signal, source_tx, worker_handles, self.concurrency)
            .await?;

        Ok(())
    }

    /// Internal method that runs the crontab scheduler.
    ///
    /// This method schedules and processes cron jobs according to their defined
    /// schedules. If no crontabs are configured, it returns immediately.
    ///
    /// # Returns
    ///
    /// A `Result` that is:
    /// - `Ok(())` if the crontab scheduler shuts down gracefully
    /// - `Err(WorkerRuntimeError)` if an error occurs during execution
    async fn crontab_scheduler(&self) -> Result<(), WorkerRuntimeError> {
        if self.crontabs().is_empty() {
            return Ok(());
        }

        cron_main(
            self.database(),
            self.escaped_schema(),
            self.crontabs(),
            *self.use_local_time(),
            self.shutdown_signal.clone(),
            &self.hooks,
        )
        .await?;

        Ok(())
    }

    /// Creates a utils object for performing worker-related operations.
    ///
    /// The `WorkerUtils` object provides utility methods for interacting with
    /// the job queue, such as adding, rescheduling, and completing jobs directly.
    ///
    /// # Returns
    ///
    /// A new `WorkerUtils` instance configured with this worker's database connection
    /// pool and schema.
    pub fn create_utils(&self) -> WorkerUtils {
        let utils = WorkerUtils::new(self.database.clone(), self.escaped_schema.clone())
            .with_task_details(self.task_details.clone());

        if self.hooks.is_empty() {
            utils
        } else {
            utils.with_hooks(self.hooks.clone())
        }
    }

    /// Requests a graceful shutdown of the worker.
    ///
    /// Wakes all internal listeners waiting on the shutdown signal so that
    /// `run`/`run_once` loops exit once in-flight work has finished.
    pub fn request_shutdown(&self) {
        self.shutdown_notifier.notify_one();
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        // Safety net for a Worker that is constructed and dropped without request_shutdown being called.
        self.shutdown_notifier.notify_one();
    }
}
