use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use std::{collections::HashMap, time::Instant};

use chrono::Utc;

use crate::errors::GraphileWorkerError;
use crate::local_queue::LocalQueue;
use crate::sql::{get_job::get_job, task_identifiers::SharedTaskDetails};
use crate::streams::{job_signal_stream, job_signal_stream_with_receiver, job_stream};
use crate::worker_utils::WorkerUtils;
use futures::{try_join, StreamExt, TryStreamExt};
use getset::Getters;
use graphile_worker_crontab_runner::{cron_main, ScheduleCronJobError};
use graphile_worker_crontab_types::Crontab;
use graphile_worker_ctx::WorkerContext;
use graphile_worker_extensions::ReadOnlyExtensions;
use graphile_worker_job::Job;
use graphile_worker_lifecycle_hooks::{
    AfterJobRunContext, BeforeJobRunContext, HookRegistry, HookResult, JobCompleteContext,
    JobFailContext, JobFetchContext, JobPermanentlyFailContext, JobStartContext, ShutdownReason,
    WorkerShutdownContext, WorkerStartContext,
};
use graphile_worker_shutdown_signal::ShutdownSignal;
use thiserror::Error;
use tokio::sync::Notify;
use tracing::{debug, error, info, trace, warn, Instrument, Span};

use crate::builder::WorkerOptions;
use crate::sql::complete_job::complete_job;
use crate::tracing::link_to_job_create_span;
use crate::{sql::fail_job::fail_job, streams::StreamSource};

/// Type alias for task handler functions.
///
/// A task handler is a closure that takes a `WorkerContext` and returns a future
/// that resolves to a `Result<(), String>`. The string in the error case represents
/// the error message if the task fails.
pub type WorkerFn = Box<
    dyn Fn(WorkerContext) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> + Send + Sync,
>;

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
    pub(crate) pg_pool: sqlx::PgPool,
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
    pub(crate) shutdown_notifier: Arc<Notify>,
    /// Extensions that can modify worker behavior
    pub(crate) extensions: ReadOnlyExtensions,
    /// Lifecycle hooks for observing and intercepting worker events
    pub(crate) hooks: Arc<HookRegistry>,
    /// Optional local queue config (LocalQueue created lazily in run())
    #[getset(skip)]
    pub(crate) local_queue_config: Option<crate::local_queue::LocalQueueConfig>,
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
                pool: self.pg_pool.clone(),
                worker_id: self.worker_id.clone(),
                extensions: self.extensions.clone(),
            })
            .await;

        let local_queue = self.create_local_queue();
        let job_runner = self.job_runner_internal(local_queue);
        let crontab_scheduler = self.crontab_scheduler();

        let result = try_join!(crontab_scheduler, job_runner);

        let reason = match &result {
            Ok(_) => ShutdownReason::Graceful,
            Err(_) => ShutdownReason::Error,
        };

        self.hooks
            .emit(WorkerShutdownContext {
                pool: self.pg_pool.clone(),
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
            self.pg_pool.clone(),
            self.shutdown_signal.clone(),
            self.task_details.clone(),
            self.escaped_schema.clone(),
            self.worker_id.clone(),
            self.forbidden_flags.clone(),
            self.use_local_time,
        );

        job_stream
            .for_each_concurrent(self.concurrency, |mut job| async move {
                loop {
                    let job_id = *job.id();
                    let has_queue = job.job_queue_id().is_some();
                    let result =
                        run_and_release_job(Arc::new(job), self, &StreamSource::RunOnce).await;

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
                    let now = self.use_local_time.then(Utc::now);
                    let task_details_guard = self.task_details.read().await;
                    let new_job = get_job(
                        self.pg_pool(),
                        &task_details_guard,
                        self.escaped_schema(),
                        self.worker_id(),
                        self.forbidden_flags(),
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
    fn create_local_queue(&self) -> Option<(LocalQueue, crate::streams::JobSignalReceiver)> {
        if let Some(ref config) = self.local_queue_config {
            let (tx, rx) = tokio::sync::mpsc::channel(self.concurrency * 2);
            let queue = LocalQueue::new(crate::local_queue::LocalQueueParams {
                config: config.clone(),
                pg_pool: self.pg_pool.clone(),
                escaped_schema: self.escaped_schema.clone(),
                worker_id: self.worker_id.clone(),
                task_details: self.task_details.clone(),
                poll_interval: self.poll_interval,
                continuous: true,
                shutdown_signal: Some(self.shutdown_signal.clone()),
                hooks: self.hooks.clone(),
                job_signal_sender: tx,
                use_local_time: self.use_local_time,
            });
            Some((queue, rx))
        } else {
            None
        }
    }

    async fn job_runner_internal(
        &self,
        local_queue: Option<(LocalQueue, crate::streams::JobSignalReceiver)>,
    ) -> Result<(), WorkerRuntimeError> {
        match local_queue {
            Some((local_queue, rx)) => self.job_runner_with_local_queue(local_queue, rx).await,
            None => self.job_runner_direct().await,
        }
    }

    /// Job runner implementation using LocalQueue for batch-fetching jobs.
    async fn job_runner_with_local_queue(
        &self,
        local_queue: LocalQueue,
        job_signal_rx: crate::streams::JobSignalReceiver,
    ) -> Result<(), WorkerRuntimeError> {
        let job_signal = job_signal_stream_with_receiver(
            self.pg_pool.clone(),
            self.poll_interval,
            self.shutdown_signal.clone(),
            self.concurrency,
            job_signal_rx,
        )
        .await?;

        debug!("Listening for jobs with LocalQueue...");
        job_signal
            .map(Ok::<_, ProcessJobError>)
            .try_for_each_concurrent(self.concurrency, |source| {
                let local_queue = local_queue.clone();
                async move {
                    if matches!(source, StreamSource::PgListener) {
                        local_queue.pulse(1).await;
                    }

                    let job = local_queue.get_job(&self.forbidden_flags).await;

                    if let Some(job) = job {
                        let job = Arc::new(job);

                        self.hooks
                            .emit(JobFetchContext {
                                job: job.clone(),
                                worker_id: self.worker_id().clone(),
                            })
                            .await;

                        run_and_release_job(job.clone(), self, &source).await?;
                        debug!(job_id = job.id(), "Job processed");
                    }

                    Ok(())
                }
            })
            .await?;

        if let Err(e) = local_queue.release().await {
            warn!(error = %e, "Error releasing LocalQueue");
        }

        Ok(())
    }

    /// Job runner implementation without LocalQueue (direct database queries).
    async fn job_runner_direct(&self) -> Result<(), WorkerRuntimeError> {
        let job_signal = job_signal_stream(
            self.pg_pool.clone(),
            self.poll_interval,
            self.shutdown_signal.clone(),
            self.concurrency,
        )
        .await?;

        debug!("Listening for jobs...");
        job_signal
            .map(Ok::<_, ProcessJobError>)
            .try_for_each_concurrent(self.concurrency, |source| async move {
                let res = process_one_job(self, source).await?;

                if let Some(job) = res {
                    debug!(job_id = job.id(), "Job processed");
                }

                Ok(())
            })
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
            self.pg_pool(),
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
        WorkerUtils::new(self.pg_pool.clone(), self.escaped_schema.clone())
            .with_hooks(self.hooks.clone())
            .with_task_details(self.task_details.clone())
    }

    /// Requests a graceful shutdown of the worker.
    ///
    /// Wakes all internal listeners waiting on the shutdown signal so that
    /// `run`/`run_once` loops exit once in-flight work has finished.
    pub fn request_shutdown(&self) {
        self.shutdown_notifier.notify_waiters();
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
    worker: &Worker,
    source: StreamSource,
) -> Result<Option<Job>, ProcessJobError> {
    let now = worker.use_local_time.then(Utc::now);
    let task_details_guard = worker.task_details.read().await;
    let job = get_job(
        worker.pg_pool(),
        &task_details_guard,
        worker.escaped_schema(),
        worker.worker_id(),
        worker.forbidden_flags(),
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

            worker
                .hooks
                .emit(JobFetchContext {
                    job: job.clone(),
                    worker_id: worker.worker_id().clone(),
                })
                .await;

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
    worker: &Worker,
    source: &StreamSource,
) -> Result<(), ProcessJobError> {
    let before_result = worker
        .hooks
        .intercept(BeforeJobRunContext {
            job: job.clone(),
            worker_id: worker.worker_id().clone(),
            payload: job.payload().clone(),
        })
        .await;

    let (job_result, duration) = match before_result {
        HookResult::Continue => {
            worker
                .hooks
                .emit(JobStartContext {
                    job: job.clone(),
                    worker_id: worker.worker_id().clone(),
                })
                .await;

            let start = Instant::now();
            let job_result = run_job(&job, worker, source).await;
            let duration = start.elapsed();

            let result_for_hook = job_result
                .as_ref()
                .map(|_| ())
                .map_err(|e| format!("{e:?}"));
            let after_result = worker
                .hooks
                .intercept(AfterJobRunContext {
                    job: job.clone(),
                    worker_id: worker.worker_id().clone(),
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
    };

    release_job(job_result, job, worker, duration)
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
    TaskPanic(#[from] tokio::task::JoinError),
    /// The task handler returned an error string
    #[error("Task returned the following error : {0}")]
    TaskError(String),
    /// The task was aborted due to a shutdown signal
    #[error("Task was aborted by shutdown signal")]
    TaskAborted,
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
async fn run_job(job: &Job, worker: &Worker, source: &StreamSource) -> Result<(), RunJobError> {
    link_to_job_create_span(job.payload().clone());
    let task_id = job.task_id();

    // Look up the task identifier (string) from the task ID (integer)
    let task_details_guard = worker.task_details.read().await;
    let task_identifier = task_details_guard
        .get(task_id)
        .ok_or_else(|| RunJobError::IdentifierNotFound(*task_id))?
        .clone();
    drop(task_details_guard);

    let span = Span::current();
    span.record("otel.name", task_identifier.as_str());
    span.record("messaging.destination.name", task_identifier.as_str());

    // Find the handler function for this task identifier
    let task_fn = worker
        .jobs()
        .get(&task_identifier)
        .ok_or_else(|| RunJobError::FnNotFound(task_identifier.clone()))?;

    debug!(source = ?source, job_id = job.id(), task_identifier, task_id, "Found task");
    let payload = job.payload().to_string();

    // Create a context for the task handler
    let worker_ctx = WorkerContext::builder()
        .payload(job.payload().clone())
        .pg_pool(worker.pg_pool().clone())
        .escaped_schema(worker.escaped_schema().clone())
        .job(job.clone())
        .worker_id(worker.worker_id().clone())
        .extensions(worker.extensions().clone())
        .task_details(worker.task_details().clone())
        .use_local_time(worker.use_local_time)
        .build();

    // Get the future that will execute the task
    let task_fut = task_fn(worker_ctx);

    // Start timing the execution
    let start = Instant::now();

    // Spawn the task on a separate Tokio task
    let job_task = tokio::spawn(task_fut.instrument(span));
    let abort_handle = job_task.abort_handle();

    // Set up a shutdown handler that waits for the shutdown signal
    // and then gives tasks 5 seconds to complete before aborting them
    let mut shutdown_signal = worker.shutdown_signal().clone();
    let shutdown_timeout = async {
        (&mut shutdown_signal).await;
        tokio::time::sleep(Duration::from_secs(5)).await;
    };

    // Wait for either the task to complete or the shutdown timeout to occur
    tokio::select! {
        res = job_task => {
            match res {
                Err(e) => Err(RunJobError::TaskPanic(e)),
                Ok(Err(e)) => Err(RunJobError::TaskError(e)),
                Ok(Ok(_)) => Ok(()),
            }
        }
        _ = shutdown_timeout => {
            abort_handle.abort();
            warn!(task_identifier, payload, job_id = job.id(), "Job interrupted by shutdown signal after 5 seconds timeout");
            Err(RunJobError::TaskAborted)
        }
    }?;

    // Calculate execution duration
    let duration = start.elapsed();

    info!(
        task_identifier,
        payload,
        job_id = job.id(),
        duration = duration.as_millis(),
        "Completed task with success"
    );

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
    worker: &Worker,
    duration: Duration,
) -> Result<(), ReleaseJobError> {
    match job_result {
        Ok(_) => {
            complete_job(
                worker.pg_pool(),
                &job,
                worker.worker_id(),
                worker.escaped_schema(),
            )
            .await
            .map_err(|e| ReleaseJobError {
                job_id: *job.id(),
                source: e,
            })?;

            worker
                .hooks
                .emit(JobCompleteContext {
                    job,
                    worker_id: worker.worker_id().clone(),
                    duration,
                })
                .await;
        }
        Err(e) => {
            let error_str = format!("{e:?}");
            let will_retry = job.attempts() < job.max_attempts();

            if !will_retry {
                error!(
                    error = ?e,
                    task_id = job.task_id(),
                    payload = ?job.payload(),
                    job_id = job.id(),
                    "Job max attempts reached"
                );

                worker
                    .hooks
                    .emit(JobPermanentlyFailContext {
                        job: job.clone(),
                        worker_id: worker.worker_id().clone(),
                        error: error_str.clone(),
                    })
                    .await;
            } else {
                warn!(
                    error = ?e,
                    task_id = job.task_id(),
                    payload = ?job.payload(),
                    job_id = job.id(),
                    "Failed task"
                );

                worker
                    .hooks
                    .emit(JobFailContext {
                        job: job.clone(),
                        worker_id: worker.worker_id().clone(),
                        error: error_str.clone(),
                        will_retry,
                    })
                    .await;
            }

            fail_job(
                worker.pg_pool(),
                &job,
                worker.escaped_schema(),
                worker.worker_id(),
                &error_str,
                None,
            )
            .await
            .map_err(|e| ReleaseJobError {
                job_id: *job.id(),
                source: e,
            })?;
        }
    }

    Ok(())
}
