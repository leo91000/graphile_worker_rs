use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use std::{collections::HashMap, time::Instant};

use crate::errors::GraphileWorkerError;
use crate::sql::{get_job::get_job, task_identifiers::TaskDetails};
use crate::streams::{job_signal_stream, job_stream};
use crate::worker_utils::WorkerUtils;
use futures::{try_join, StreamExt, TryStreamExt};
use getset::Getters;
use graphile_worker_crontab_runner::{cron_main, ScheduleCronJobError};
use graphile_worker_crontab_types::Crontab;
use graphile_worker_ctx::WorkerContext;
use graphile_worker_extensions::ReadOnlyExtensions;
use graphile_worker_hooks::*;
use graphile_worker_job::Job;
use graphile_worker_shutdown_signal::ShutdownSignal;
use thiserror::Error;
use tracing::{debug, error, info, trace, warn};

use crate::builder::WorkerOptions;
use crate::sql::complete_job::complete_job;
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
    pub(crate) task_details: TaskDetails,
    /// List of job flags that this worker will not process
    pub(crate) forbidden_flags: Vec<String>,
    /// List of cron job definitions to be scheduled
    pub(crate) crontabs: Vec<Crontab>,
    /// Whether to use local time for cron jobs (true) or UTC (false)
    pub(crate) use_local_time: bool,
    /// Signal that can be triggered to gracefully shut down the worker
    pub(crate) shutdown_signal: ShutdownSignal,
    /// Extensions that can modify worker behavior
    pub(crate) extensions: ReadOnlyExtensions,
    /// Optional lifecycle hooks for observability
    pub(crate) hooks: Option<Arc<dyn JobLifecycleHooks>>,
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
        let job_runner = self.job_runner();
        let crontab_scheduler = self.crontab_scheduler();

        try_join!(crontab_scheduler, job_runner)?;

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
        );

        job_stream
            .for_each_concurrent(self.concurrency, |mut job| async move {
                loop {
                    let result = run_and_release_job(&job, self, &StreamSource::RunOnce).await;

                    match result {
                        Ok(_) => {
                            info!(job_id = job.id(), "Job processed");
                        }
                        Err(e) => {
                            error!("Error while processing job : {:?}", e);
                        }
                    };

                    // If the job has a queue, we need to fetch another job because the job_signal will not trigger
                    // Is there a simpler way to do this ?
                    if job.job_queue_id().is_none() {
                        break;
                    }
                    info!(job_id = job.id(), "Job has queue, fetching another job");
                    let new_job = get_job(
                        self.pg_pool(),
                        self.task_details(),
                        self.escaped_schema(),
                        self.worker_id(),
                        self.forbidden_flags(),
                    )
                    .await
                    .unwrap_or(None);
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
    async fn job_runner(&self) -> Result<(), WorkerRuntimeError> {
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
    let job = get_job(
        worker.pg_pool(),
        worker.task_details(),
        worker.escaped_schema(),
        worker.worker_id(),
        worker.forbidden_flags(),
    )
    .await
    .map_err(|e| {
        error!("Could not get job : {:?}", e);
        e
    })?;

    match job {
        Some(job) => {
            run_and_release_job(&job, worker, &source).await?;
            Ok(Some(job))
        }
        None => {
            // TODO: Retry one time because maybe synchronization issue
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
/// * `job` - The job to process
/// * `worker` - The worker instance that provides access to task handlers
/// * `source` - The source of the job request (signal, run_once, etc.)
///
/// # Returns
///
/// A `Result` containing:
/// - `Ok(&'a Job)` if the job was processed and released successfully
/// - `Err(ProcessJobError)` if an error occurred during processing or releasing
async fn run_and_release_job<'a>(
    job: &'a Job,
    worker: &Worker,
    source: &StreamSource,
) -> Result<&'a Job, ProcessJobError> {
    let (job_result, duration) = run_job(job, worker, source).await;
    release_job(job_result, job, worker, duration)
        .await
        .map_err(|e| {
            error!("Release job error : {:?}", e);
            e
        })?;
    Ok(job)
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
/// A tuple containing:
/// - A `Result` indicating whether the job was successfully executed
/// - The `Duration` of the job execution
async fn run_job(
    job: &Job,
    worker: &Worker,
    source: &StreamSource,
) -> (Result<(), RunJobError>, Duration) {
    let task_id = job.task_id();

    // Look up the task identifier (string) from the task ID (integer)
    let task_identifier_result = worker
        .task_details()
        .get(task_id)
        .ok_or_else(|| RunJobError::IdentifierNotFound(*task_id));

    let task_identifier = match task_identifier_result {
        Ok(id) => id,
        Err(e) => return (Err(e), Duration::from_secs(0)),
    };

    // Find the handler function for this task identifier
    let task_fn_result = worker
        .jobs()
        .get(task_identifier)
        .ok_or_else(|| RunJobError::FnNotFound(task_identifier.into()));

    let task_fn = match task_fn_result {
        Ok(f) => f,
        Err(e) => return (Err(e), Duration::from_secs(0)),
    };

    debug!(source = ?source, job_id = job.id(), task_identifier, task_id, "Found task");
    let payload = job.payload().to_string();

    // Create a context for the task handler
    let worker_ctx = WorkerContext::new(
        job.payload().clone(),
        worker.pg_pool().clone(),
        worker.escaped_schema().clone(),
        job.clone(),
        worker.worker_id().clone(),
        worker.extensions().clone(),
    );

    // Emit job started event, now that we have a valid job to start.
    if let Some(hooks) = &worker.hooks {
        hooks
            .on_event(LifeCycleEvent::Started(JobStarted {
                job_id: *job.id(),
                task_identifier: task_identifier.clone(),
                priority: *job.priority(),
                attempts: *job.attempts(),
            }))
            .await;
    }

    // Get the future that will execute the task
    let task_fut = task_fn(worker_ctx);

    // Start timing the execution
    let start = Instant::now();

    // Spawn the task on a separate Tokio task
    let job_task = tokio::spawn(task_fut);
    let abort_handle = job_task.abort_handle();

    // Set up a shutdown handler that waits for the shutdown signal
    // and then gives tasks 5 seconds to complete before aborting them
    let mut shutdown_signal = worker.shutdown_signal().clone();
    let shutdown_timeout = async {
        (&mut shutdown_signal).await;
        tokio::time::sleep(Duration::from_secs(5)).await;
    };

    // Wait for either the task to complete or the shutdown timeout to occur
    let result = tokio::select! {
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
    };

    // Calculate execution duration
    let duration = start.elapsed();

    if result.is_ok() {
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

    (result, duration)
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
/// * `job` - The job that was executed
/// * `worker` - The worker instance that provides access to the database
/// * `duration` - The duration of the job execution
///
/// # Returns
///
/// A `Result` indicating whether the job was successfully released:
/// - `Ok(())` if the job was marked as completed or failed in the database
/// - `Err(ReleaseJobError)` if an error occurred while updating the database
async fn release_job(
    job_result: Result<(), RunJobError>,
    job: &Job,
    worker: &Worker,
    duration: Duration,
) -> Result<(), ReleaseJobError> {
    // Get task identifier for hooks
    let task_identifier = worker
        .task_details()
        .get(job.task_id())
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());

    match job_result {
        Ok(_) => {
            // Emit job completed event
            if let Some(hooks) = &worker.hooks {
                hooks
                    .on_event(LifeCycleEvent::Completed(JobCompleted {
                        job_id: *job.id(),
                        task_identifier: task_identifier.clone(),
                        duration,
                        attempts: *job.attempts(),
                    }))
                    .await;
            }

            // The job was successful - mark it as completed
            complete_job(
                worker.pg_pool(),
                job,
                worker.worker_id(),
                worker.escaped_schema(),
            )
            .await
            .map_err(|e| ReleaseJobError {
                job_id: *job.id(),
                source: e,
            })?;
        }
        Err(e) => {
            // The job failed - log the error and mark it as failed
            let will_retry = job.attempts() < job.max_attempts();

            // Emit job failed event
            if let Some(hooks) = &worker.hooks {
                hooks
                    .on_event(LifeCycleEvent::Failed(JobFailed {
                        job_id: *job.id(),
                        task_identifier: task_identifier.clone(),
                        error: format!("{e:?}"),
                        duration,
                        attempts: *job.attempts(),
                        will_retry,
                    }))
                    .await;
            }

            if !will_retry {
                // This was the last attempt - log as an error
                error!(
                    error = ?e,
                    task_id = job.task_id(),
                    payload = ?job.payload(),
                    job_id = job.id(),
                    "Job max attempts reached"
                );
            } else {
                // The job will be retried - log as a warning
                warn!(
                    error = ?e,
                    task_id = job.task_id(),
                    payload = ?job.payload(),
                    job_id = job.id(),
                    "Failed task"
                );
            }

            // Mark the job as failed in the database
            fail_job(
                worker.pg_pool(),
                job,
                worker.escaped_schema(),
                worker.worker_id(),
                &format!("{e:?}"),
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
