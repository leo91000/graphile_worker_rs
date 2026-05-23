use std::fmt::{self, Debug, Display};
use std::sync::Arc;
use std::time::Duration;
use std::{collections::HashMap, time::Instant};

use chrono::Utc;

use crate::batcher::{CompletionRequest, FailureRequest};
use crate::errors::GraphileWorkerError;
use crate::local_queue::LocalQueue;
use crate::sql::{get_job::get_job, task_identifiers::SharedTaskDetails};
use crate::streams::{job_signal_stream, job_signal_stream_with_receiver, job_stream};
use crate::worker_utils::WorkerUtils;
use futures::{stream::FuturesUnordered, try_join, FutureExt, Stream, StreamExt};
use getset::Getters;
use graphile_worker_crontab_runner::{cron_main, ScheduleCronJobError};
use graphile_worker_crontab_types::Crontab;
use graphile_worker_ctx::WorkerContext;
use graphile_worker_database::Database;
use graphile_worker_extensions::ReadOnlyExtensions;
use graphile_worker_job::Job;
use graphile_worker_lifecycle_hooks::{
    AfterJobRunContext, BeforeJobRunContext, HookRegistry, HookResult, JobCompleteContext,
    JobFailContext, JobFetchContext, JobPermanentlyFailContext, JobStartContext, ShutdownReason,
    WorkerShutdownContext, WorkerStartContext,
};
use graphile_worker_runtime as runtime;
use graphile_worker_shutdown_signal::ShutdownSignal;
use graphile_worker_task_handler::TaskHandlerOutcome;
use thiserror::Error;
use tracing::{debug, error, info, trace, warn, Instrument, Span};

use crate::builder::WorkerOptions;
use crate::sql::complete_job::complete_job;
use crate::tracing::link_to_job_create_span;
use crate::{sql::fail_job::fail_job, streams::StreamSource};

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
}

/// Errors that can occur during worker runtime.
///
/// These errors represent various failure scenarios that can happen
/// while the worker is running and processing jobs.
#[derive(Error, Debug)]
pub enum WorkerRuntimeError {
    /// An error occurred while processing or releasing a job
    #[error("Unexpected error occured while processing job : '{0}'")]
    ProcessJob(#[from] ProcessJobError),
    #[error("Worker task failed : '{0}'")]
    WorkerTask(#[from] runtime::JoinError),
    /// Failed to listen to PostgreSQL notifications for new jobs
    #[error("Failed to listen to postgres notifications : '{0}'")]
    PgListen(#[from] GraphileWorkerError),
    /// An error occurred while scheduling or executing a cron job
    #[error("Error occured while trying to schedule cron job : {0}")]
    Crontab(#[from] ScheduleCronJobError),
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
        self.hooks
            .emit(WorkerStartContext {
                database: self.database.clone(),
                worker_id: self.worker_id.clone(),
                extensions: self.extensions.clone(),
            })
            .await;

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

        self.hooks
            .emit(WorkerShutdownContext {
                database: self.database.clone(),
                worker_id: self.worker_id.clone(),
                reason,
            })
            .await;

        result?;

        Ok(())
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
        if let Some(ref config) = self.local_queue_config {
            let (tx, rx) = runtime::channel(self.concurrency * 2);
            if config.queue_count == 0 {
                panic!("local_queue.queue_count must be greater than 0");
            }
            let queue_count = config.queue_count.min(self.concurrency);
            let queues = (0..queue_count)
                .map(|_| {
                    LocalQueue::new(crate::local_queue::LocalQueueParams {
                        config: config.clone(),
                        database: self.database.clone(),
                        escaped_schema: self.escaped_schema.clone(),
                        worker_id: self.worker_id.clone(),
                        task_details: self.task_details.clone(),
                        poll_interval: self.poll_interval,
                        continuous: true,
                        shutdown_signal: Some(self.shutdown_signal.clone()),
                        hooks: self.hooks.clone(),
                        job_signal_sender: tx.clone(),
                        use_local_time: self.use_local_time,
                    })
                })
                .collect();
            Some((queues, rx))
        } else {
            None
        }
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
                    process_local_queue_source(&runner, &local_queues, index, source).await?;
                }

                Ok::<(), ProcessJobError>(())
            }));
        }
        drop(source_rx);

        let dispatch_result =
            dispatch_job_signals(job_signal, source_tx, worker_handles, self.concurrency).await;

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

        dispatch_job_signals(job_signal, source_tx, worker_handles, self.concurrency).await?;

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

/// Errors that can occur while processing a job.
#[derive(Error, Debug)]
pub enum ProcessJobError {
    /// Error occurred when trying to complete or fail a job after processing
    #[error("An error occured while releasing a job : '{0}'")]
    ReleaseJobError(#[from] ReleaseJobError),
    /// Error occurred when trying to fetch a job from the database
    #[error("An error occured while fetching a job to run : '{0}'")]
    GetJobError(#[from] GraphileWorkerError),
}

async fn dispatch_job_signals<S>(
    job_signal: S,
    source_tx: runtime::Sender<StreamSource>,
    mut worker_handles: FuturesUnordered<runtime::JoinHandle<Result<(), ProcessJobError>>>,
    fanout: usize,
) -> Result<(), WorkerRuntimeError>
where
    S: Stream<Item = StreamSource>,
{
    let job_signal = job_signal.fuse();
    futures::pin_mut!(job_signal);

    loop {
        let next_source = job_signal.next().fuse();
        let worker_done = worker_handles.next().fuse();
        futures::pin_mut!(next_source, worker_done);

        futures::select_biased! {
            worker_result = worker_done => {
                match worker_result {
                    Some(Ok(Ok(()))) => {
                        if worker_handles.is_empty() {
                            break;
                        }
                    }
                    Some(Ok(Err(e))) => {
                        source_tx.close();
                        return Err(e.into());
                    }
                    Some(Err(e)) => {
                        source_tx.close();
                        return Err(e.into());
                    }
                    None => break,
                }
            }
            source = next_source => {
                let Some(source) = source else {
                    break;
                };

                let mut closed = false;
                for _ in 0..fanout {
                    if source_tx.try_send(source).is_err() && source_tx.send(source).await.is_err() {
                        closed = true;
                        break;
                    }
                }
                if closed {
                    break;
                }
            }
        }
    }

    drop(source_tx);

    while let Some(result) = worker_handles.next().await {
        result??;
    }

    Ok(())
}

async fn process_local_queue_source(
    worker: &WorkerRunner,
    local_queues: &[LocalQueue],
    start_index: usize,
    source: StreamSource,
) -> Result<(), ProcessJobError> {
    if matches!(source, StreamSource::PgListener) {
        local_queues[start_index % local_queues.len()]
            .pulse(1)
            .await;
    }

    let mut source = source;
    loop {
        let job = get_job_from_local_queues(worker, local_queues, start_index).await;

        let Some(job) = job else {
            break;
        };

        let job = Arc::new(job);

        if !worker.hooks.is_empty() {
            worker
                .hooks
                .emit(JobFetchContext {
                    job: job.clone(),
                    worker_id: worker.worker_id.clone(),
                })
                .await;
        }

        run_and_release_job(job.clone(), worker, &source).await?;
        source = StreamSource::Internal;
    }

    Ok(())
}

async fn get_job_from_local_queues(
    worker: &WorkerRunner,
    local_queues: &[LocalQueue],
    start_index: usize,
) -> Option<Job> {
    if !worker.forbidden_flags.is_empty() {
        return local_queues[start_index % local_queues.len()]
            .get_job(&worker.forbidden_flags)
            .await;
    }

    for offset in 0..local_queues.len() {
        let queue_index = (start_index + offset) % local_queues.len();
        if let Some(job) = local_queues[queue_index]
            .get_job(&worker.forbidden_flags)
            .await
        {
            return Some(job);
        }
    }

    None
}

/// Fetches and processes a single job from the queue.
///
/// This function attempts to get a job from the database, process it by
/// finding and executing the appropriate task handler, and then mark it
/// as completed or failed based on the result.
///
/// # Arguments
///
/// * `worker` - The worker instance that provides access to the database and task handlers
/// * `source` - The source of the job request (signal, run_once, etc.)
///
/// # Returns
///
/// A `Result` containing:
/// - `Ok(Some(job))` if a job was found and processed
/// - `Ok(None)` if no job was available
/// - `Err(ProcessJobError)` if an error occurred during processing
async fn process_one_job(
    worker: &WorkerRunner,
    source: StreamSource,
) -> Result<Option<Job>, ProcessJobError> {
    let now = worker.use_local_time.then(Utc::now);
    let task_details_guard = worker.task_details.read().await;
    let job = get_job(
        &worker.database,
        &task_details_guard,
        &worker.escaped_schema,
        &worker.worker_id,
        &worker.forbidden_flags,
        now,
    )
    .await
    .map_err(|e| {
        error!("Could not get job : {:?}", e);
        e
    })?;
    drop(task_details_guard);

    match job {
        Some(job) => {
            let job = Arc::new(job);

            if !worker.hooks.is_empty() {
                worker
                    .hooks
                    .emit(JobFetchContext {
                        job: job.clone(),
                        worker_id: worker.worker_id.clone(),
                    })
                    .await;
            }

            run_and_release_job(job.clone(), worker, &source).await?;
            Ok(Some(
                Arc::try_unwrap(job).unwrap_or_else(|arc| (*arc).clone()),
            ))
        }
        None => {
            trace!(source = ?source, "No job found");
            Ok(None)
        }
    }
}

/// Executes a job's task handler and then marks the job as completed or failed.
///
/// This function runs the job by finding and executing the appropriate task handler,
/// then releases the job by either marking it as completed (if successful) or
/// failed (if an error occurred).
///
/// # Arguments
///
/// * `job` - The job to process (wrapped in Arc for efficient sharing)
/// * `worker` - The worker instance that provides access to task handlers
/// * `source` - The source of the job request (signal, run_once, etc.)
///
/// # Returns
///
/// A `Result` containing:
/// - `Ok(())` if the job was processed and released successfully
/// - `Err(ProcessJobError)` if an error occurred during processing or releasing
async fn run_and_release_job(
    job: Arc<Job>,
    worker: &WorkerRunner,
    source: &StreamSource,
) -> Result<(), ProcessJobError> {
    let (job_result, duration) = if worker.hooks.is_empty() {
        let start = Instant::now();
        let job_result = run_job(job.clone(), worker, source).await;
        (job_result, start.elapsed())
    } else {
        let before_result = worker
            .hooks
            .intercept(BeforeJobRunContext {
                job: job.clone(),
                worker_id: worker.worker_id.clone(),
                payload: job.payload().clone(),
            })
            .await;

        match before_result {
            HookResult::Continue => {
                worker
                    .hooks
                    .emit(JobStartContext {
                        job: job.clone(),
                        worker_id: worker.worker_id.clone(),
                    })
                    .await;

                let start = Instant::now();
                let job_result = run_job(job.clone(), worker, source).await;
                let duration = start.elapsed();

                let result_for_hook = job_result
                    .as_ref()
                    .map(|_| ())
                    .map_err(|e| format!("{e:?}"));
                let after_result = worker
                    .hooks
                    .intercept(AfterJobRunContext {
                        job: job.clone(),
                        worker_id: worker.worker_id.clone(),
                        result: result_for_hook,
                        duration,
                    })
                    .await;

                match after_result {
                    HookResult::Continue => (job_result, duration),
                    HookResult::Skip => (Ok(()), duration),
                    HookResult::Fail(msg) => (Err(RunJobError::TaskError(msg)), duration),
                }
            }
            HookResult::Skip => {
                debug!(job_id = job.id(), "Job skipped by before_job_run hook");
                (Ok(()), Duration::ZERO)
            }
            HookResult::Fail(msg) => {
                debug!(
                    job_id = job.id(),
                    "Job failed by before_job_run hook: {}", msg
                );
                (Err(RunJobError::TaskError(msg)), Duration::ZERO)
            }
        }
    };

    release_job(job_result, job.clone(), worker, duration)
        .await
        .map_err(|e| {
            error!("Release job error : {:?}", e);
            e
        })?;
    Ok(())
}

/// Errors that can occur during the execution of a job's task handler.
#[derive(Error, Debug)]
enum RunJobError {
    /// No task identifier was found for the given task ID
    #[error("Cannot find any task identifier for given task id '{0}'. This is probably a bug !")]
    IdentifierNotFound(i32),
    /// No task handler function was registered for the given task identifier
    #[error("Cannot find any task fn for given task identifier '{0}'. This is probably a bug !")]
    FnNotFound(String),
    /// The task handler panicked during execution
    #[error("Task failed execution to complete : {0}")]
    TaskPanic(String),
    /// The task handler returned an error string
    #[error("Task returned the following error : {0}")]
    TaskError(String),
    /// The batch task handler returned partial failures with a replacement payload
    #[error("Task returned the following error : {message}")]
    TaskErrorWithReplacement {
        message: String,
        replacement_payload: Redacted<serde_json::Value>,
    },
    /// The task was aborted due to a shutdown signal
    #[error("Task was aborted by shutdown signal")]
    TaskAborted,
}

struct Redacted<T>(T);

impl<T> Redacted<T> {
    fn new(value: T) -> Self {
        Self(value)
    }

    fn get(&self) -> &T {
        &self.0
    }
}

impl<T> Debug for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

impl<T> Display for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

impl RunJobError {
    fn persisted_error(&self) -> String {
        match self {
            RunJobError::TaskErrorWithReplacement { message, .. } => {
                format!("TaskError({message:?})")
            }
            _ => format!("{self:?}"),
        }
    }

    fn replacement_payload(&self) -> Option<&serde_json::Value> {
        match self {
            RunJobError::TaskErrorWithReplacement {
                replacement_payload,
                ..
            } => Some(replacement_payload.get()),
            _ => None,
        }
    }
}

fn panic_payload_to_string(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return (*message).to_string();
    }

    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }

    "task panicked".to_string()
}

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    use futures::FutureExt;
    use graphile_worker_database::{
        BoxFuture, Database, DatabaseDriver, DbError, DbExecutor, DbParams, DbRow, DbTransaction,
        NotificationStream,
    };
    use graphile_worker_extensions::{Extensions, ReadOnlyExtensions};
    use graphile_worker_job::Job;
    use graphile_worker_lifecycle_hooks::HookRegistry;
    use graphile_worker_shutdown_signal::ShutdownSignal;

    use crate::streams::StreamSource;

    use super::{panic_payload_to_string, release_job, Redacted, RunJobError, WorkerRunner};

    #[derive(Debug)]
    struct FailingDriver;

    impl DbExecutor for FailingDriver {
        fn execute<'a>(
            &'a self,
            _sql: &'a str,
            _params: DbParams,
        ) -> BoxFuture<'a, Result<u64, DbError>> {
            Box::pin(async { Err(DbError::new("forced failure")) })
        }

        fn fetch_all<'a>(
            &'a self,
            _sql: &'a str,
            _params: DbParams,
        ) -> BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
            Box::pin(async { Ok(Vec::new()) })
        }
    }

    impl DatabaseDriver for FailingDriver {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn begin<'a>(&'a self) -> BoxFuture<'a, Result<DbTransaction, DbError>> {
            Box::pin(async { Err(DbError::new("transactions are unavailable")) })
        }

        fn listen<'a>(
            &'a self,
            _channel: &'a str,
        ) -> BoxFuture<'a, Result<Option<NotificationStream>, DbError>> {
            Box::pin(async { Ok(None) })
        }
    }

    fn pending_shutdown_signal() -> ShutdownSignal {
        futures::future::pending::<()>().boxed().shared()
    }

    #[tokio::test]
    async fn failing_driver_contract_is_exercised() {
        let driver = FailingDriver;

        assert!(driver.as_any().is::<FailingDriver>());
        assert!(driver
            .execute("", DbParams::new())
            .await
            .expect_err("execute should fail")
            .to_string()
            .contains("forced failure"));
        assert!(driver
            .fetch_all("", DbParams::new())
            .await
            .expect("fetch_all should return an empty result")
            .is_empty());
        assert!(driver.begin().await.is_err());
        assert!(driver
            .listen("")
            .await
            .expect("listen should succeed without a stream")
            .is_none());
    }

    #[test]
    fn panic_payload_to_string_handles_static_str() {
        assert_eq!(
            panic_payload_to_string(Box::new("static panic")),
            "static panic"
        );
    }

    #[test]
    fn run_job_error_debug_redacts_replacement_payload() {
        let payload = serde_json::json!({ "secret": "token" });
        let error = RunJobError::TaskErrorWithReplacement {
            message: "failed".to_string(),
            replacement_payload: Redacted::new(payload.clone()),
        };

        let debug = format!("{error:?}");

        assert!(debug.contains("[redacted]"));
        assert!(!debug.contains("token"));
        assert_eq!(format!("{}", Redacted::new(payload.clone())), "[redacted]");
        assert_eq!(error.persisted_error(), "TaskError(\"failed\")");
        assert_eq!(error.replacement_payload(), Some(&payload));
    }

    #[test]
    fn panic_payload_to_string_handles_unknown_payload() {
        assert_eq!(panic_payload_to_string(Box::new(1usize)), "task panicked");
    }

    #[test]
    fn panic_payload_to_string_handles_string_payload() {
        assert_eq!(
            panic_payload_to_string(Box::new(String::from("dynamic panic"))),
            "dynamic panic"
        );
    }

    #[test]
    fn stream_source_is_copy_for_worker_fanout() {
        fn assert_copy<T: Copy>() {}

        assert_copy::<StreamSource>();
    }

    #[tokio::test]
    async fn release_job_returns_error_when_replacement_payload_cannot_be_persisted() {
        let database = Database::new(FailingDriver);
        let hooks = Arc::new(HookRegistry::default());
        let shutdown_signal = pending_shutdown_signal();
        let failure_batcher = Arc::new(crate::batcher::FailureBatcher::new(
            Duration::from_secs(60),
            database.clone(),
            "graphile_worker".to_string(),
            "worker".to_string(),
            hooks.clone(),
            shutdown_signal.clone(),
        ));
        let worker = WorkerRunner {
            worker_id: "worker".to_string(),
            jobs: HashMap::new(),
            database,
            escaped_schema: "graphile_worker".to_string(),
            task_details: Default::default(),
            forbidden_flags: Vec::new(),
            use_local_time: false,
            shutdown_signal,
            extensions: ReadOnlyExtensions::from(Extensions::new()),
            hooks,
            completion_batcher: None,
            failure_batcher: Some(failure_batcher),
        };
        let job = Arc::new(
            Job::builder()
                .id(42)
                .attempts(1)
                .max_attempts(1)
                .locked_by("worker".to_string())
                .task_identifier("batch")
                .payload(serde_json::json!({ "items": [1] }))
                .build(),
        );

        let error = release_job(
            Err(RunJobError::TaskErrorWithReplacement {
                message: "failed".to_string(),
                replacement_payload: Redacted::new(serde_json::json!({ "items": [2] })),
            }),
            job,
            &worker,
            Duration::from_millis(5),
        )
        .await
        .expect_err("release should surface the failed persistence query");

        assert_eq!(error.job_id, 42);
        assert!(error.source.to_string().contains("forced failure"));
    }
}

/// Executes a job's task handler function.
///
/// This function looks up the appropriate task handler for the job, creates
/// a context with the job's payload and other information, and executes the
/// handler. It also handles shutdown signals gracefully, allowing tasks a
/// timeout period to complete before aborting them.
///
/// # Arguments
///
/// * `job` - The job to execute
/// * `worker` - The worker instance that provides access to task handlers
/// * `source` - The source of the job request (signal, run_once, etc.)
///
/// # Returns
///
/// A `Result` indicating whether the job was successfully executed:
/// - `Ok(())` if the task handler completed successfully
/// - `Err(RunJobError)` if an error occurred during execution
#[tracing::instrument(
    "run_job",
    skip(job, worker, source),
    fields(
        job_id = job.id(),
        messaging.system = "graphile-worker",
        messaging.operation.name = "run_job",
        messaging.destination.name = tracing::field::Empty,
        otel.name = tracing::field::Empty
    )
)]
async fn run_job(
    job: Arc<Job>,
    worker: &WorkerRunner,
    source: &StreamSource,
) -> Result<(), RunJobError> {
    link_to_job_create_span(job.payload());
    let task_id = job.task_id();
    let task_identifier = job.task_identifier();
    if task_identifier.is_empty() {
        return Err(RunJobError::IdentifierNotFound(*task_id));
    }

    let span = Span::current();
    span.record("otel.name", task_identifier.as_str());
    span.record("messaging.destination.name", task_identifier.as_str());

    let task_fn = worker
        .jobs
        .get(task_identifier)
        .ok_or_else(|| RunJobError::FnNotFound(task_identifier.clone()))?;

    debug!(source = ?source, job_id = job.id(), task_identifier, task_id, "Found task");

    // Create a context for the task handler
    let worker_ctx = WorkerContext::from_shared_job(
        job.clone(),
        worker.database.clone(),
        worker.escaped_schema.clone(),
        worker.worker_id.clone(),
        worker.extensions.clone(),
        worker.task_details.clone(),
        worker.use_local_time,
    );

    // Get the future that will execute the task
    let task_fut = task_fn(worker_ctx);

    // Start timing the execution
    let start = Instant::now();

    // Set up a shutdown handler that waits for the shutdown signal
    // and then gives tasks 5 seconds to complete before aborting them
    let mut shutdown_signal = worker.shutdown_signal.clone();
    let shutdown_timeout = async {
        (&mut shutdown_signal).await;
        runtime::sleep(Duration::from_secs(5)).await;
    };

    let job_task = std::panic::AssertUnwindSafe(task_fut.instrument(span))
        .catch_unwind()
        .fuse();
    let shutdown_timeout = shutdown_timeout.fuse();
    futures::pin_mut!(job_task, shutdown_timeout);

    // Wait for either the task to complete or the shutdown timeout to occur
    futures::select_biased! {
        res = job_task => {
            match res {
                Ok(TaskHandlerOutcome::Complete) => Ok(()),
                Ok(TaskHandlerOutcome::Failed {
                    error,
                    replacement_payload: Some(replacement_payload),
                }) => Err(RunJobError::TaskErrorWithReplacement {
                    message: error,
                    replacement_payload: Redacted::new(replacement_payload),
                }),
                Ok(TaskHandlerOutcome::Failed {
                    error,
                    replacement_payload: None,
                }) => Err(RunJobError::TaskError(error)),
                Err(e) => Err(RunJobError::TaskPanic(panic_payload_to_string(e))),
            }
        }
        _ = shutdown_timeout => {
            let payload = job.payload().to_string();
            warn!(task_identifier, payload, job_id = job.id(), "Job interrupted by shutdown signal after 5 seconds timeout");
            Err(RunJobError::TaskAborted)
        }
    }?;

    // Calculate execution duration
    let duration = start.elapsed();

    if tracing::enabled!(tracing::Level::INFO) {
        let payload = job.payload().to_string();
        info!(
            task_identifier,
            payload,
            job_id = job.id(),
            duration = duration.as_millis(),
            "Completed task with success"
        );
    }

    // TODO: Handle batch jobs (vec of futures returned by
    // function)

    Ok(())
}

/// Error that occurs when trying to mark a job as completed or failed.
#[derive(Error, Debug)]
#[error("Failed to release job '{job_id}'. {source}")]
pub struct ReleaseJobError {
    /// The ID of the job that could not be released
    job_id: i64,
    /// The underlying error that caused the release operation to fail
    #[source]
    source: GraphileWorkerError,
}

/// Marks a job as completed or failed based on the result of its execution.
///
/// This function releases a job by either:
/// - Marking it as completed if the task handler executed successfully
/// - Marking it as failed if the task handler encountered an error
///
/// When marking a job as failed, it will be retried later if the maximum
/// number of attempts has not been reached.
///
/// # Arguments
///
/// * `job_result` - The result of executing the job's task handler
/// * `job` - The job that was executed (wrapped in Arc)
/// * `worker` - The worker instance that provides access to the database
/// * `duration` - How long the job took to execute
///
/// # Returns
///
/// A `Result` indicating whether the job was successfully released:
/// - `Ok(())` if the job was marked as completed or failed in the database
/// - `Err(ReleaseJobError)` if an error occurred while updating the database
async fn release_job(
    job_result: Result<(), RunJobError>,
    job: Arc<Job>,
    worker: &WorkerRunner,
    duration: Duration,
) -> Result<(), ReleaseJobError> {
    match job_result {
        Ok(_) => {
            if let Some(batcher) = &worker.completion_batcher {
                batcher
                    .complete(CompletionRequest {
                        job_id: *job.id(),
                        has_queue: job.job_queue_id().is_some(),
                        job,
                        duration,
                    })
                    .await;
            } else {
                complete_job(
                    &worker.database,
                    &job,
                    &worker.worker_id,
                    &worker.escaped_schema,
                )
                .await
                .map_err(|e| ReleaseJobError {
                    job_id: *job.id(),
                    source: e,
                })?;

                if !worker.hooks.is_empty() {
                    worker
                        .hooks
                        .emit(JobCompleteContext {
                            job,
                            worker_id: worker.worker_id.clone(),
                            duration,
                        })
                        .await;
                }
            }
        }
        Err(e) => {
            let error_str = e.persisted_error();
            let will_retry = job.attempts() < job.max_attempts();
            let replacement_payload = e.replacement_payload().cloned();

            if let Some(batcher) = &worker.failure_batcher {
                if let Some(replacement_payload) = replacement_payload.clone() {
                    if let Err(e) = fail_job(
                        &worker.database,
                        &job,
                        &worker.escaped_schema,
                        &worker.worker_id,
                        &error_str,
                        Some(replacement_payload),
                    )
                    .await
                    {
                        error!(error = ?e, job_id = job.id(), "Failed to persist failed job");
                        let job_id = *job.id();
                        return Err(ReleaseJobError { job_id, source: e });
                    }

                    if !will_retry {
                        error!(error = ?e, task_id = job.task_id(), payload = ?job.payload(), job_id = job.id(), "Job max attempts reached");
                    } else {
                        warn!(error = ?e, task_id = job.task_id(), payload = ?job.payload(), job_id = job.id(), "Failed task");
                    }

                    if !worker.hooks.is_empty() {
                        if will_retry {
                            worker
                                .hooks
                                .emit(JobFailContext {
                                    job,
                                    worker_id: worker.worker_id.clone(),
                                    error: error_str,
                                    will_retry,
                                })
                                .await;
                        } else {
                            worker
                                .hooks
                                .emit(JobPermanentlyFailContext {
                                    job,
                                    worker_id: worker.worker_id.clone(),
                                    error: error_str,
                                })
                                .await;
                        }
                    }
                    return Ok(());
                }

                if !will_retry {
                    error!(
                        error = ?e,
                        task_id = job.task_id(),
                        payload = ?job.payload(),
                        job_id = job.id(),
                        "Job max attempts reached"
                    );
                } else {
                    warn!(
                        error = ?e,
                        task_id = job.task_id(),
                        payload = ?job.payload(),
                        job_id = job.id(),
                        "Failed task"
                    );
                }

                batcher
                    .fail(FailureRequest {
                        job,
                        error: error_str,
                        will_retry,
                    })
                    .await;
            } else {
                if !will_retry {
                    error!(
                        error = ?e,
                        task_id = job.task_id(),
                        payload = ?job.payload(),
                        job_id = job.id(),
                        "Job max attempts reached"
                    );
                } else {
                    warn!(
                        error = ?e,
                        task_id = job.task_id(),
                        payload = ?job.payload(),
                        job_id = job.id(),
                        "Failed task"
                    );
                }

                if let Err(e) = fail_job(
                    &worker.database,
                    &job,
                    &worker.escaped_schema,
                    &worker.worker_id,
                    &error_str,
                    replacement_payload,
                )
                .await
                {
                    error!(error = ?e, job_id = job.id(), "Failed to persist failed job");
                    return Err(ReleaseJobError {
                        job_id: *job.id(),
                        source: e,
                    });
                }

                if !worker.hooks.is_empty() {
                    if will_retry {
                        worker
                            .hooks
                            .emit(JobFailContext {
                                job,
                                worker_id: worker.worker_id.clone(),
                                error: error_str,
                                will_retry,
                            })
                            .await;
                    } else {
                        worker
                            .hooks
                            .emit(JobPermanentlyFailContext {
                                job,
                                worker_id: worker.worker_id.clone(),
                                error: error_str,
                            })
                            .await;
                    }
                }
            }
        }
    }

    Ok(())
}
