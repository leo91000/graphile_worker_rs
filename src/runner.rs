use std::fmt::Debug;
use std::future::{ready, Future};
use std::pin::Pin;
use std::time::Duration;
use std::{collections::HashMap, time::Instant};

use crate::errors::ArchimedesError;
use crate::sql::get_job::Job;
use crate::sql::{get_job::get_job, task_identifiers::TaskDetails};
use crate::streams::job_signal_stream;
use futures::{StreamExt, TryStreamExt};
use getset::Getters;
use thiserror::Error;
use tracing::{debug, error, info, warn};

use crate::builder::WorkerOptions;
use crate::sql::complete_job::complete_job;
use crate::{sql::fail_job::fail_job, streams::StreamSource};

#[derive(Clone, Getters)]
#[getset(get = "pub")]
pub struct WorkerContext {
    pg_pool: sqlx::PgPool,
}

impl From<&Worker> for WorkerContext {
    fn from(value: &Worker) -> Self {
        WorkerContext {
            pg_pool: value.pg_pool().clone(),
        }
    }
}

pub type WorkerFn =
    Box<dyn Fn(WorkerContext, String) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>>;

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
}

#[derive(Error, Debug)]
pub enum WorkerRuntimeError {
    #[error("Unexpected error occured while processing job : '{0}'")]
    ProcessJobError(#[from] ProcessJobError),
    #[error("Failed to listen to postgres notifications : '{0}'")]
    PgListenError(#[from] ArchimedesError),
}

impl Worker {
    pub fn options() -> WorkerOptions {
        WorkerOptions::default()
    }

    pub async fn run(&self) -> Result<(), WorkerRuntimeError> {
        let job_signal = job_signal_stream(self.pg_pool.clone(), self.poll_interval).await?;

        job_signal
            .then(|source| process_one_job(self, source))
            .try_for_each_concurrent(self.concurrency, |j| {
                if let Some(j) = &j {
                    debug!("Job id={} processed", j.id());
                }
                ready(Ok(()))
            })
            .await?;

        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum ProcessJobError {
    #[error("An error occured while releasing a job : '{0}'")]
    ReleaseJobError(#[from] ReleaseJobError),
    #[error("An error occured while fetching a job to run : '{0}'")]
    GetJobError(#[from] ArchimedesError),
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
            let job_result = run_job(&job, worker, &source).await;
            release_job(job_result, &job, worker).await.map_err(|e| {
                error!("{:?}", e);
                e
            })?;
            Ok(Some(job))
        }
        None => {
            // TODO: Retry one time because maybe synchronization issue
            debug!(source = ?source, "No job found");
            Ok(None)
        }
    }
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
    let task_fut = task_fn(worker.into(), payload.clone());

    let start = Instant::now();
    tokio::spawn(task_fut)
        .await?
        .map_err(RunJobError::TaskError)?;
    let duration = start.elapsed().as_millis();

    info!(
        task_identifier,
        payload,
        job_id = job.id(),
        duration,
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
    source: ArchimedesError,
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
