use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::batcher::{CompletionBatcher, FailureBatcher};
use crate::local_queue::LocalQueueConfig;
use crate::runner::WorkerFn;
use crate::sql::task_identifiers::{get_tasks_details, SharedTaskDetails};
use crate::utils::escape_identifier;
use crate::Worker;
use graphile_worker_crontab_types::Crontab;
use graphile_worker_database::Database;
use graphile_worker_extensions::Extensions;
use graphile_worker_lifecycle_hooks::HookRegistry;
use graphile_worker_migrations::migrate;
use graphile_worker_shutdown_signal::shutdown_signal;
use rand::Rng;

mod batching;
mod core;
mod cron;
mod cron_input;
mod database;
mod errors;
mod hooks;
mod jobs;
mod local_queue;
mod recovery;
mod shutdown;

pub use cron_input::CronInput;
use database::connect_default_database;
pub use errors::WorkerBuildError;
use shutdown::{combine_shutdown_signals, manual_shutdown_signal_pair};

/// Configuration options for initializing a Graphile Worker instance.
///
/// WorkerOptions provides a builder-style API for configuring a worker instance,
/// including database connection settings, concurrency, job definitions, and more.
///
/// # Example
///
/// ```no_run
/// use graphile_worker::WorkerOptions;
/// use graphile_worker::{TaskHandler, WorkerContext, IntoTaskHandlerResult};
/// use std::time::Duration;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Deserialize, Serialize)]
/// struct MyTask { data: String }
///
/// impl TaskHandler for MyTask {
///     const IDENTIFIER: &'static str = "my_task";
///     async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
///         Ok::<(), String>(())
///     }
/// }
///
/// async fn example() -> Result<(), Box<dyn std::error::Error>> {
///     let worker = WorkerOptions::default()
///         .concurrency(5)
///         .schema("my_app_worker")
///         .poll_interval(Duration::from_millis(500))
///         .database_url("postgres://user:password@localhost/mydb")
///         .define_job::<MyTask>()
///         .init()
///         .await?;
///     
///     // Start the worker
///     worker.run().await?;
///     
///     Ok(())
/// }
/// ```
#[derive(Default)]
pub struct WorkerOptions {
    /// Number of jobs to process concurrently
    concurrency: Option<usize>,

    /// How often to poll the database for new jobs
    poll_interval: Option<Duration>,

    /// Map of job identifiers to handler functions
    jobs: HashMap<String, WorkerFn>,

    /// PostgreSQL connection pool
    database: Option<Database>,

    /// PostgreSQL connection string
    database_url: Option<String>,

    /// Maximum number of database connections in the pool
    max_pg_conn: Option<u32>,

    /// PostgreSQL schema name for Graphile Worker tables
    schema: Option<String>,

    /// List of job flags that this worker will refuse to process
    forbidden_flags: Vec<String>,

    /// List of crontab entries for scheduled jobs
    crontabs: Option<Vec<Crontab>>,

    /// Whether to use local application time (true) or database time (false) for timestamps
    use_local_time: bool,

    /// Custom application state and dependencies
    extensions: Extensions,

    /// Lifecycle hooks for observing and intercepting worker events
    hooks: HookRegistry,

    /// Whether to automatically install OS-level shutdown signal listeners.
    /// Defaults to `true` when not explicitly configured.
    listen_os_shutdown_signals: Option<bool>,

    /// Configuration for the local queue (batch-fetching jobs).
    /// When set, enables LocalQueue for improved throughput.
    local_queue_config: Option<LocalQueueConfig>,

    /// Delay before flushing batched job completions.
    /// When set, job completions are collected and flushed in batches.
    /// This reduces SQL round trips and improves throughput.
    complete_job_batch_delay: Option<Duration>,

    /// Delay before flushing batched job failures.
    /// When set, job failures are collected and flushed in batches.
    /// Retryable failures are still processed individually.
    fail_job_batch_delay: Option<Duration>,

    /// Worker crash and shutdown recovery configuration.
    worker_recovery_config: Option<crate::recovery::WorkerRecoveryConfig>,
}

impl WorkerOptions {
    fn append_crontabs(&mut self, mut crontabs: Vec<Crontab>) {
        match self.crontabs.as_mut() {
            Some(existing) => existing.append(&mut crontabs),
            None => {
                self.crontabs = Some(crontabs);
            }
        }
    }

    /// Initializes a worker with the configured options.
    ///
    /// Process:
    /// 1. Establishes a database connection (using the provided pool or creating one from the URL)
    /// 2. Runs database migrations to ensure the schema is up to date
    /// 3. Registers the task handlers
    /// 4. Initializes the worker with a random ID and the configured settings
    ///
    /// # Returns
    /// * `Result<Worker, WorkerBuildError>` - A fully configured worker instance or an error
    ///
    /// # Errors
    /// Can fail if:
    /// * Database connection fails
    /// * Database URL is missing and no pool was provided
    /// * Migrations fail
    /// * Task registration fails
    ///
    /// # Example
    /// ```no_run
    /// # use graphile_worker::WorkerOptions;
    /// # use graphile_worker::{TaskHandler, WorkerContext, IntoTaskHandlerResult};
    /// # use serde::{Deserialize, Serialize};
    /// # #[derive(Deserialize, Serialize)]
    /// # struct MyTask { data: String }
    /// # impl TaskHandler for MyTask {
    /// #     const IDENTIFIER: &'static str = "my_task";
    /// #     async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult { Ok::<(), String>(()) }
    /// # }
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let worker = WorkerOptions::default()
    ///     .database_url("postgres://user:password@localhost/mydb")
    ///     .schema("my_app_worker")
    ///     .define_job::<MyTask>()
    ///     .init()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn init(self) -> Result<Worker, WorkerBuildError> {
        let listen_os_shutdown_signals = self.listen_os_shutdown_signals.unwrap_or(true);

        let database = match self.database {
            Some(database) => database,
            None => {
                let db_url = self
                    .database_url
                    .ok_or(WorkerBuildError::MissingDatabaseUrl)?;

                connect_default_database(&db_url, self.max_pg_conn.unwrap_or(20)).await?
            }
        };

        let schema = self
            .schema
            .unwrap_or_else(|| String::from("graphile_worker"));
        let escaped_schema = escape_identifier(&schema);

        migrate(&database, &escaped_schema).await?;

        let task_details: SharedTaskDetails = get_tasks_details(
            &database,
            &escaped_schema,
            self.jobs.keys().cloned().collect(),
        )
        .await?
        .into();

        let mut random_bytes = [0u8; 9];
        rand::rng().fill_bytes(&mut random_bytes);

        let (manual_signal, shutdown_notifier) = manual_shutdown_signal_pair();
        let shutdown_signal = if listen_os_shutdown_signals {
            combine_shutdown_signals(manual_signal, shutdown_signal())
        } else {
            manual_signal
        };

        let worker_id = format!("graphile_worker_{}", hex::encode(random_bytes));
        let poll_interval = self.poll_interval.unwrap_or(Duration::from_millis(1000));

        let hooks = Arc::new(self.hooks);

        let concurrency = self.concurrency.unwrap_or_else(num_cpus::get);

        let local_queue_config = if self.forbidden_flags.is_empty() {
            self.local_queue_config
        } else {
            None
        };
        if let Some(config) = local_queue_config.as_ref() {
            config.validate(poll_interval)?;
        }

        let completion_batcher = self.complete_job_batch_delay.map(|delay| {
            Arc::new(CompletionBatcher::new(
                delay,
                database.clone(),
                escaped_schema.clone(),
                worker_id.clone(),
                hooks.clone(),
                shutdown_signal.clone(),
            ))
        });

        let failure_batcher = self.fail_job_batch_delay.map(|delay| {
            Arc::new(FailureBatcher::new(
                delay,
                database.clone(),
                escaped_schema.clone(),
                worker_id.clone(),
                hooks.clone(),
                shutdown_signal.clone(),
            ))
        });

        let recovery_config = self.worker_recovery_config.unwrap_or_default();

        let worker = Worker {
            worker_id,
            concurrency,
            poll_interval,
            jobs: self.jobs,
            database,
            escaped_schema,
            task_details,
            forbidden_flags: self.forbidden_flags,
            crontabs: self.crontabs.unwrap_or_default(),
            use_local_time: self.use_local_time,
            shutdown_signal,
            shutdown_notifier,
            extensions: self.extensions.into(),
            hooks,
            local_queue_config,
            completion_batcher,
            failure_batcher,
            recovery_config,
        };

        Ok(worker)
    }
}
