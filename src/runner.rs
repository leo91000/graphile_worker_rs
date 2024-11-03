use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
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
use graphile_worker_job::Job;
use graphile_worker_shutdown_signal::ShutdownSignal;
use thiserror::Error;
use tracing::{debug, error, info, trace, warn};

use crate::builder::WorkerOptions;
use crate::sql::complete_job::complete_job;
use crate::{sql::fail_job::fail_job, streams::StreamSource};

pub type WorkerFn =
    Box<dyn Fn(WorkerContext) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>>;

#[derive(Getters)]
#[getset(get = "pub")]
pub struct Worker {
    pub(crate) worker_id: String,
    pub(crate) concurrency: usize,
    pub(crate) poll_interval: Duration,
    pub(crate) jobs: HashMap<String, WorkerFn>,
    pub(crate) pg_pool: sqlx::PgPool,
    pub(crate) escaped_schema: String,
    pub(crate) task_details: TaskDetails,
    pub(crate) forbidden_flags: Vec<String>,
    pub(crate) crontabs: Vec<Crontab>,
    pub(crate) use_local_time: bool,
    pub(crate) shutdown_signal: ShutdownSignal,
    pub(crate) extensions: ReadOnlyExtensions,
}

#[derive(Error, Debug)]
pub enum WorkerRuntimeError {
    #[error("Unexpected error occured while processing job : '{0}'")]
    ProcessJob(#[from] ProcessJobError),
    #[error("Failed to listen to postgres notifications : '{0}'")]
    PgListen(#[from] GraphileWorkerError),
    #[error("Error occured while trying to schedule cron job : {0}")]
    Crontab(#[from] ScheduleCronJobError),
}

impl Worker {
    pub fn options() -> WorkerOptions {
        WorkerOptions::default()
    }

    /// Run the worker and return when the shutdown signal is triggered
    /// It listen for new jobs and run them as soon as they are available
    pub async fn run(&self) -> Result<(), WorkerRuntimeError> {
        let job_runner = self.job_runner();
        let crontab_scheduler = self.crontab_scheduler();

        try_join!(crontab_scheduler, job_runner)?;

        Ok(())
    }

    /// Run the worker once and return when all jobs are processed
    /// An error in the job will not stop the stream
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

    async fn crontab_scheduler<'e>(&self) -> Result<(), WorkerRuntimeError> {
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

    pub fn create_utils(&self) -> WorkerUtils {
        WorkerUtils::new(self.pg_pool.clone(), self.escaped_schema.clone())
    }
}

#[derive(Error, Debug)]
pub enum ProcessJobError {
    #[error("An error occured while releasing a job : '{0}'")]
    ReleaseJobError(#[from] ReleaseJobError),
    #[error("An error occured while fetching a job to run : '{0}'")]
    GetJobError(#[from] GraphileWorkerError),
}

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

async fn run_and_release_job<'a>(
    job: &'a Job,
    worker: &Worker,
    source: &StreamSource,
) -> Result<&'a Job, ProcessJobError> {
    let job_result = run_job(job, worker, source).await;
    release_job(job_result, job, worker).await.map_err(|e| {
        error!("Release job error : {:?}", e);
        e
    })?;
    Ok(job)
}

#[derive(Error, Debug)]
enum RunJobError {
    #[error("Cannot find any task identifier for given task id '{0}'. This is probably a bug !")]
    IdentifierNotFound(i32),
    #[error("Cannot find any task fn for given task identifier '{0}'. This is probably a bug !")]
    FnNotFound(String),
    #[error("Task failed execution to complete : {0}")]
    TaskPanic(#[from] tokio::task::JoinError),
    #[error("Task returned the following error : {0}")]
    TaskError(String),
    #[error("Task was aborted by shutdown signal")]
    TaskAborted,
}

async fn run_job(job: &Job, worker: &Worker, source: &StreamSource) -> Result<(), RunJobError> {
    let task_id = job.task_id();

    let task_identifier = worker
        .task_details()
        .get(task_id)
        .ok_or_else(|| RunJobError::IdentifierNotFound(*task_id))?;

    let task_fn = worker
        .jobs()
        .get(task_identifier)
        .ok_or_else(|| RunJobError::FnNotFound(task_identifier.into()))?;

    debug!(source = ?source, job_id = job.id(), task_identifier, task_id, "Found task");
    let payload = job.payload().to_string();
    let worker_ctx = WorkerContext::new(
        job.payload().clone(),
        worker.pg_pool().clone(),
        job.clone(),
        worker.worker_id().clone(),
        worker.extensions().clone(),
    );
    let task_fut = task_fn(worker_ctx);

    let start = Instant::now();
    let job_task = tokio::spawn(task_fut);
    let abort_handle = job_task.abort_handle();
    let mut shutdown_signal = worker.shutdown_signal().clone();
    let shutdown_timeout = async {
        (&mut shutdown_signal).await;
        tokio::time::sleep(Duration::from_secs(5)).await;
    };
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

#[derive(Error, Debug)]
#[error("Failed to release job '{job_id}'. {source}")]
pub struct ReleaseJobError {
    job_id: i64,
    #[source]
    source: GraphileWorkerError,
}

async fn release_job(
    job_result: Result<(), RunJobError>,
    job: &Job,
    worker: &Worker,
) -> Result<(), ReleaseJobError> {
    match job_result {
        Ok(_) => {
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
            if job.attempts() >= job.max_attempts() {
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
