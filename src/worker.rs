use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use std::{collections::HashMap, time::Instant};

use crate::errors::ArchimedesError;
use crate::migrate::migrate;
use crate::sql::get_job::Job;
use crate::sql::{
    get_job::get_job,
    task_identifiers::{get_tasks_details, TaskDetails},
};
use crate::streams::job_signal_stream;
use crate::utils::escape_identifier;
use futures::{FutureExt, StreamExt};
use getset::Getters;
use rand::RngCore;
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use thiserror::Error;
use tracing::{debug, error, info, warn};

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

type WorkerFn =
    Box<dyn Fn(WorkerContext, String) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>>;

#[derive(Getters)]
#[getset(get = "pub")]
pub struct Worker {
    worker_id: String,
    concurrency: usize,
    poll_interval: Duration,
    jobs: HashMap<String, WorkerFn>,
    pg_pool: sqlx::PgPool,
    escaped_schema: String,
    task_details: TaskDetails,
    forbidden_flags: Vec<String>,
}

impl Worker {
    pub fn options() -> WorkerOptions {
        WorkerOptions::default()
    }

    pub async fn run(&self) -> crate::errors::Result<()> {
        let job_signal = job_signal_stream(self.pg_pool.clone(), self.poll_interval).await?;

        job_signal
            .for_each_concurrent(self.concurrency, |source| process_one_job(self, source))
            .await;

        Ok(())
    }
}

async fn process_one_job(worker: &Worker, source: StreamSource) {
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
    })
    .ok()
    .flatten();

    dbg!(&job);

    match job {
        Some(job) => {
            let job_result = run_job(&job, worker, &source).await;
            release_job(job_result, &job, worker)
                .await
                .map_err(|e| {
                    error!("{:?}", e);
                    e
                })
                .ok();
        }
        None => {
            // Retry one time because maybe synchronization issue
            debug!(source = ?source, "No job found");
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
struct ReleaseJobError {
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
                &format!("{:?}", e),
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

#[derive(Default)]
pub struct WorkerOptions {
    concurrency: Option<usize>,
    poll_interval: Option<Duration>,
    jobs: HashMap<String, WorkerFn>,
    pg_pool: Option<PgPool>,
    database_url: Option<String>,
    max_pg_conn: Option<u32>,
    schema: Option<String>,
    forbidden_flags: Vec<String>,
}

#[derive(Error, Debug)]
pub enum WorkerBuildError {
    #[error("Error occured while connecting to the postgres database : {0}")]
    ConnectError(#[from] sqlx::Error),
    #[error("Error occured while querying : {0}")]
    QueryError(#[from] crate::errors::ArchimedesError),
    #[error("Missing database_url config")]
    MissingDatabaseUrl,
}

impl WorkerOptions {
    pub async fn init(self) -> Result<Worker, WorkerBuildError> {
        let pg_pool = match self.pg_pool {
            Some(pg_pool) => pg_pool,
            None => {
                let db_url = self
                    .database_url
                    .ok_or(WorkerBuildError::MissingDatabaseUrl)?;

                PgPoolOptions::new()
                    .max_connections(self.max_pg_conn.unwrap_or(20))
                    .connect(&db_url)
                    .await?
            }
        };

        let schema = self
            .schema
            .unwrap_or_else(|| String::from("archimedes_worker"));
        let escaped_schema = escape_identifier(&pg_pool, &schema).await?;

        migrate(&pg_pool, &escaped_schema).await?;

        let task_details = get_tasks_details(
            &pg_pool,
            &escaped_schema,
            self.jobs.keys().cloned().collect(),
        )
        .await?;

        let mut random_bytes = [0u8; 9];
        rand::thread_rng().fill_bytes(&mut random_bytes);
        let worker = Worker {
            worker_id: format!("archimedes_worker_{}", hex::encode(random_bytes)),
            concurrency: self.concurrency.unwrap_or_else(num_cpus::get),
            poll_interval: self.poll_interval.unwrap_or(Duration::from_millis(1000)),
            jobs: self.jobs,
            pg_pool,
            escaped_schema,
            task_details,
            forbidden_flags: self.forbidden_flags,
        };

        Ok(worker)
    }

    pub fn schema(mut self, value: &str) -> Self {
        self.schema = Some(value.into());
        self
    }

    pub fn concurrency(mut self, value: usize) -> Self {
        self.concurrency = Some(value);
        self
    }

    pub fn poll_interval(mut self, value: Duration) -> Self {
        self.poll_interval = Some(value);
        self
    }

    pub fn pg_pool(mut self, value: PgPool) -> Self {
        self.pg_pool = Some(value);
        self
    }

    pub fn database_url(mut self, value: &str) -> Self {
        self.database_url = Some(value.into());
        self
    }

    pub fn max_pg_conn(mut self, value: u32) -> Self {
        self.max_pg_conn = Some(value);
        self
    }

    pub fn define_job<T, E, Fut, F>(mut self, identifier: &str, job_fn: F) -> Self
    where
        T: for<'de> Deserialize<'de> + Send,
        E: Debug,
        Fut: Future<Output = Result<(), E>> + Send,
        F: Fn(WorkerContext, T) -> Fut + Send + Sync + 'static,
    {
        let job_fn = Arc::new(job_fn);
        let worker_fn = move |ctx: WorkerContext, payload: String| {
            let job_fn = job_fn.clone();
            async move {
                let de_payload = serde_json::from_str(&payload);

                match de_payload {
                    Err(e) => Err(format!("{:?}", e)),
                    Ok(p) => {
                        let job_result = job_fn(ctx, p).await;
                        match job_result {
                            Err(e) => Err(format!("{:?}", e)),
                            Ok(v) => Ok(v),
                        }
                    }
                }
            }
            .boxed()
        };

        self.jobs
            .insert(identifier.to_string(), Box::new(worker_fn));
        self
    }

    pub fn add_forbidden_flag(mut self, flag: &str) -> Self {
        self.forbidden_flags.push(flag.into());
        self
    }
}
