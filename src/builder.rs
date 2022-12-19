use crate::runner::WorkerFn;
use crate::sql::task_identifiers::get_tasks_details;
use crate::utils::escape_identifier;
use crate::{Worker, WorkerContext};
use archimedes_migrations::migrate;
use futures::FutureExt;
use rand::RngCore;
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::collections::HashMap;
use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

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
                    Err(e) => Err(format!("{e:?}")),
                    Ok(p) => {
                        let job_result = job_fn(ctx, p).await;
                        match job_result {
                            Err(e) => Err(format!("{e:?}")),
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
