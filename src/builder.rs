use crate::runner::WorkerFn;
use crate::sql::task_identifiers::get_tasks_details;
use crate::utils::escape_identifier;
use crate::Worker;
use futures::FutureExt;
use graphile_worker_crontab_parser::{parse_crontab, CrontabParseError};
use graphile_worker_crontab_types::Crontab;
use graphile_worker_ctx::WorkerContext;
use graphile_worker_extensions::Extensions;
use graphile_worker_migrations::migrate;
use graphile_worker_shutdown_signal::shutdown_signal;
use graphile_worker_task_handler::{run_task_from_worker_ctx, TaskHandler};
use rand::RngCore;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::collections::HashMap;
use std::fmt::Debug;
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
    crontabs: Option<Vec<Crontab>>,
    use_local_time: bool,
    extensions: Extensions,
}

#[derive(Error, Debug)]
pub enum WorkerBuildError {
    #[error("Error occured while connecting to the postgres database : {0}")]
    ConnectError(#[from] sqlx::Error),
    #[error("Error occured while querying : {0}")]
    QueryError(#[from] crate::errors::GraphileWorkerError),
    #[error("Missing database_url config")]
    MissingDatabaseUrl,
    #[error("Error occured while migrating : {0}")]
    MigrationError(#[from] graphile_worker_migrations::MigrateError),
}

impl WorkerOptions {
    /// Build a new worker with the given options.
    /// It migrate the database and fetch the tasks details.
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
            .unwrap_or_else(|| String::from("graphile_worker"));
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
            worker_id: format!("graphile_worker_{}", hex::encode(random_bytes)),
            concurrency: self.concurrency.unwrap_or_else(num_cpus::get),
            poll_interval: self.poll_interval.unwrap_or(Duration::from_millis(1000)),
            jobs: self.jobs,
            pg_pool,
            escaped_schema,
            task_details,
            forbidden_flags: self.forbidden_flags,
            crontabs: self.crontabs.unwrap_or_default(),
            use_local_time: self.use_local_time,
            shutdown_signal: shutdown_signal(),
            extensions: self.extensions.into(),
        };

        Ok(worker)
    }

    /// Set the postgresql schema to use for the worker.
    pub fn schema(mut self, value: &str) -> Self {
        self.schema = Some(value.into());
        self
    }

    /// Set the number of concurrent jobs that can be run at the same time.
    /// Default is the number of logical CPUs in the system.
    ///
    /// # Panics
    ///
    /// Panics if the value is 0.
    pub fn concurrency(mut self, value: usize) -> Self {
        if value == 0 {
            panic!("Concurrency must be greater than 0");
        }

        self.concurrency = Some(value);
        self
    }

    /// Set the interval at which the worker should poll the database for new jobs.
    pub fn poll_interval(mut self, value: Duration) -> Self {
        self.poll_interval = Some(value);
        self
    }

    /// Set the postgresql pool to use for the worker.
    pub fn pg_pool(mut self, value: PgPool) -> Self {
        self.pg_pool = Some(value);
        self
    }

    /// Set the postgresql database url to use for the worker.
    pub fn database_url(mut self, value: &str) -> Self {
        self.database_url = Some(value.into());
        self
    }

    /// Set the maximum number of postgresql connections to use for the worker.
    pub fn max_pg_conn(mut self, value: u32) -> Self {
        self.max_pg_conn = Some(value);
        self
    }

    /// Define a job to be run by the worker.
    pub fn define_job<T: TaskHandler>(mut self) -> Self {
        let identifier = T::IDENTIFIER;

        let worker_fn = move |ctx: WorkerContext| {
            let ctx = ctx.clone();
            run_task_from_worker_ctx::<T>(ctx).boxed()
        };

        self.jobs
            .insert(identifier.to_string(), Box::new(worker_fn));
        self
    }

    /// Adds a forbidden flag to the worker.
    pub fn add_forbidden_flag(mut self, flag: &str) -> Self {
        self.forbidden_flags.push(flag.into());
        self
    }

    /// Adds a crontab to the worker.
    pub fn with_crontab(mut self, input: &str) -> Result<Self, CrontabParseError> {
        let mut crontabs = parse_crontab(input)?;
        match self.crontabs.as_mut() {
            Some(c) => c.append(&mut crontabs),
            None => {
                self.crontabs = Some(crontabs);
            }
        }
        Ok(self)
    }

    /// Set whether the worker should use local time or postgresql time.
    pub fn use_local_time(mut self, value: bool) -> Self {
        self.use_local_time = value;
        self
    }

    /// Add an extension to the worker.
    /// Usefull for providing custom app state.
    pub fn add_extension<T: Clone + Send + Sync + Debug + 'static>(mut self, value: T) -> Self {
        self.extensions.insert(value);
        self
    }
}
