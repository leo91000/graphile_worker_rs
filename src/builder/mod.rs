use std::collections::HashMap;
use std::time::Duration;

use crate::local_queue::LocalQueueConfig;
use crate::runner::WorkerFn;
use graphile_worker_crontab_types::Crontab;
use graphile_worker_database::{Database, Schema};
use graphile_worker_extensions::Extensions;
use graphile_worker_lifecycle_hooks::HookRegistry;
use graphile_worker_recovery::WorkerRecoveryConfig;

mod batching;
mod core;
mod cron;
mod cron_input;
mod database;
mod errors;
mod hooks;
mod init;
mod jobs;
mod local_queue;
mod recovery;
mod shutdown;

pub use cron_input::CronInput;
pub use errors::WorkerBuildError;

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

    /// Use notification delivery
    use_notification_delivery: Option<bool>,

    /// Map of job identifiers to handler functions
    jobs: HashMap<String, WorkerFn>,

    /// PostgreSQL connection pool
    database: Option<Database>,

    /// PostgreSQL connection string
    database_url: Option<String>,

    /// Maximum number of database connections in the pool
    max_pg_conn: Option<u32>,

    /// PostgreSQL schema for Graphile Worker tables
    schema: Option<Schema>,

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

    /// Dead worker recovery configuration.
    worker_recovery_config: Option<WorkerRecoveryConfig>,

    /// Worker shutdown behavior.
    worker_shutdown_config: Option<crate::WorkerShutdownConfig>,
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
}
