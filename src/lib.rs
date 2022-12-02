use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use std::{collections::HashMap, time::Instant};

use futures::{FutureExt, StreamExt};
use getset::Getters;
use migrate::migrate;
use rand::RngCore;
use serde::Deserialize;
use sql::{
    get_job::get_job,
    task_identifiers::{get_tasks_details, TaskDetails},
};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use streams::job_signal_stream;
use thiserror::Error;
use tracing::{debug, error, info, warn};
use utils::escape_identifier;

use crate::sql::complete_job::complete_job;
use crate::sql::fail_job::fail_job;

pub mod errors;
pub mod migrate;
mod migrations;
mod sql;
mod streams;
mod utils;

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
            .for_each_concurrent(self.concurrency, |source| {
                async move {
                    let job = get_job(
                        self.pg_pool(),
                        self.task_details(),
                        self.escaped_schema(),
                        self.worker_id(),
                        self.forbidden_flags(),
                    )
                    .await;

                    dbg!(&job);

                    match job {
                        Ok(Some(j)) => {
                            let task_id = j.task_id();
                            let task = self.task_details().get(task_id).and_then(|task_identifier| { 
                                let task_fn = self.jobs().get(task_identifier);
                                task_fn.map(|task_fn| (task_identifier, task_fn))
                            });
                            if let Some((task_identifier, task_fn)) = task {
                                debug!(source = ?source, job_id = j.id(), task_identifier, "Found task");
                                let payload = j.payload().to_string();
                                let task_fut = task_fn(self.into(), payload.clone());

                                let start = Instant::now();
                                let spawn_result = tokio::spawn(task_fut).await;
                                let duration = start.elapsed().as_millis();

                                match spawn_result {
                                    Ok(task_result) => {
                                        // TODO: Handle batch jobs (vec of futures returned by
                                        // function)
                                        match task_result {
                                            Ok(_) => {
                                                info!(task_identifier, payload, job_id = j.id(), duration, "Completed task with success");
                                                complete_job(self.pg_pool(), &j, self.worker_id(), self.escaped_schema()).await;
                                            },
                                            Err(message) => {
                                                warn!(error = message, task_identifier, payload, job_id = j.id(), "Failed task");
                                                if j.attempts() >= j.max_attempts() {
                                                    error!(error = message, task_identifier, payload, job_id = j.id(), "Job max attempts reached");
                                                }

                                                let fj_res = fail_job(self.pg_pool(), &j, self.escaped_schema(), self.worker_id(), &message, None).await;
                                                dbg!(fj_res);
                                            }
                                        }
                                    },
                                    Err(e) => {
                                        error!(error = ?e, task_identifier, payload, "Task panicked");
                                    },
                                }
                            } else {
                                error!(source = ?source, task_id, "Unsupported task identifier");
                            }
                        }
                        Ok(None) => {
                            // Retry one time because maybe synchronization issue
                            debug!(source = ?source, "No job found");
                        }
                        Err(e) => {
                            // Retry or throw error after N failures
                        }
                    }

                }
            })
            .await;

        Ok(())
    }
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
