use crate::runner::WorkerFn;
use crate::sql::task_identifiers::get_tasks_details;
use crate::utils::escape_identifier;
use crate::Worker;
use futures::FutureExt;
use graphile_worker_crontab_parser::{parse_crontab, CrontabParseError};
use graphile_worker_crontab_types::Crontab;
use graphile_worker_ctx::WorkerContext;
use graphile_worker_extensions::Extensions;
use graphile_worker_lifecycle_hooks::{LifecycleHooks, TypeErasedHooks};
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
    pg_pool: Option<PgPool>,

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

    /// Whether to use local time or PostgreSQL server time for scheduling
    use_local_time: bool,

    /// Custom application state and dependencies
    extensions: Extensions,

    /// Lifecycle hooks for observing and intercepting worker events
    hooks: TypeErasedHooks,
}

/// Errors that can occur when initializing a worker.
#[derive(Error, Debug)]
pub enum WorkerBuildError {
    /// Failed to connect to the PostgreSQL database
    #[error("Error occurred while connecting to the PostgreSQL database: {0}")]
    ConnectError(#[from] sqlx::Error),

    /// Failed while executing a database query
    #[error("Error occurred while executing a query: {0}")]
    QueryError(#[from] crate::errors::GraphileWorkerError),

    /// The database URL was not provided and no PgPool was supplied
    #[error("Missing database_url configuration - must provide either database_url or pg_pool")]
    MissingDatabaseUrl,

    /// Failed to apply database migrations
    #[error("Error occurred while migrating the database schema: {0}")]
    MigrationError(#[from] graphile_worker_migrations::MigrateError),
}

impl WorkerOptions {
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
        rand::rng().fill_bytes(&mut random_bytes);

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
            hooks: self.hooks,
        };

        Ok(worker)
    }

    /// Sets the PostgreSQL schema name for Graphile Worker tables.
    ///
    /// Isolates Graphile Worker tables in a separate schema,
    /// which keeps the database organized or allows running multiple
    /// independent worker instances in the same database.
    ///
    /// # Arguments
    /// * `value` - The schema name to use
    ///
    /// # Default
    /// If not specified, the schema name defaults to "graphile_worker".
    pub fn schema(mut self, value: &str) -> Self {
        self.schema = Some(value.into());
        self
    }

    /// Sets the number of jobs that can be processed concurrently.
    ///
    /// Controls how many jobs the worker processes simultaneously.
    /// Setting an appropriate concurrency level depends on workload:
    /// - CPU-intensive tasks: Set close to the number of cores
    /// - I/O-intensive tasks: Can often use higher values (10-20+)
    ///
    /// # Arguments
    /// * `value` - The maximum number of concurrent jobs
    ///
    /// # Default
    /// If not specified, defaults to the number of logical CPUs in the system.
    ///
    /// # Panics
    /// Panics if the value is 0, as at least one job must be processable.
    pub fn concurrency(mut self, value: usize) -> Self {
        assert!(value > 0, "Concurrency must be greater than 0");
        self.concurrency = Some(value);
        self
    }

    /// Sets how often the worker checks the database for new jobs.
    ///
    /// Controls the polling interval for checking for new jobs when
    /// PostgreSQL notification delivery fails or for jobs scheduled in the future.
    ///
    /// # Arguments
    /// * `value` - The interval between database polls
    ///
    /// # Default
    /// If not specified, defaults to 1000 milliseconds (1 second).
    ///
    /// # Note
    /// Lower values increase responsiveness but may increase database load.
    /// For most applications, the default value is appropriate.
    pub fn poll_interval(mut self, value: Duration) -> Self {
        self.poll_interval = Some(value);
        self
    }

    /// Sets an existing PostgreSQL connection pool for the worker to use.
    ///
    /// Allows reusing an existing connection pool or configuring
    /// the pool with custom settings before passing it to the worker.
    ///
    /// # Arguments
    /// * `value` - The PostgreSQL connection pool
    ///
    /// # Note
    /// If both `pg_pool` and `database_url` are provided, `pg_pool` takes precedence.
    pub fn pg_pool(mut self, value: PgPool) -> Self {
        self.pg_pool = Some(value);
        self
    }

    /// Sets the PostgreSQL database connection URL.
    ///
    /// The URL is used to establish a connection to the database if no
    /// connection pool is provided.
    ///
    /// # Arguments
    /// * `value` - The PostgreSQL connection URL (e.g., "postgres://user:password@localhost/mydb")
    ///
    /// # Note
    /// Either `pg_pool` or `database_url` must be provided before calling `init()`.
    pub fn database_url(mut self, value: &str) -> Self {
        self.database_url = Some(value.into());
        self
    }

    /// Sets the maximum number of database connections in the pool.
    ///
    /// Only applies when creating a new connection pool from
    /// a database URL. Ignored if an existing pool is provided.
    ///
    /// # Arguments
    /// * `value` - The maximum number of connections
    ///
    /// # Default
    /// If not specified, defaults to 20 connections.
    pub fn max_pg_conn(mut self, value: u32) -> Self {
        self.max_pg_conn = Some(value);
        self
    }

    /// Registers a task handler type with the worker.
    ///
    /// Primary way to define what types of jobs this worker can process.
    /// Each task handler implements the `TaskHandler` trait, which defines how to
    /// deserialize and run jobs of a specific type.
    ///
    /// # Type Parameters
    /// * `T` - A type implementing the TaskHandler trait
    ///
    /// # Example
    /// ```
    /// # use graphile_worker::{WorkerOptions, TaskHandler, WorkerContext, IntoTaskHandlerResult};
    /// # use serde::{Deserialize, Serialize};
    /// #
    /// # #[derive(Deserialize, Serialize)]
    /// # struct SendEmail { to: String }
    /// #
    /// # impl TaskHandler for SendEmail {
    /// #     const IDENTIFIER: &'static str = "send_email";
    /// #     async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
    /// #         Ok::<(), String>(())
    /// #     }
    /// # }
    ///
    /// let options = WorkerOptions::default()
    ///     .define_job::<SendEmail>();
    /// ```
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

    /// Adds a flag to the list of forbidden flags.
    ///
    /// Jobs with any forbidden flag will be skipped by this worker instance.
    /// Can be used to implement specialized workers that only handle
    /// certain types of jobs.
    ///
    /// # Arguments
    /// * `flag` - The flag to forbid
    ///
    /// # Example
    /// ```
    /// # use graphile_worker::WorkerOptions;
    ///
    /// // This worker will skip jobs with the "high_memory" flag
    /// let options = WorkerOptions::default()
    ///     .add_forbidden_flag("high_memory");
    /// ```
    pub fn add_forbidden_flag(mut self, flag: &str) -> Self {
        self.forbidden_flags.push(flag.into());
        self
    }

    /// Adds crontab entries for scheduled jobs.
    ///
    /// Crontab entries define jobs that run on a schedule, similar
    /// to Unix cron jobs. The syntax is compatible with Graphile Worker's
    /// crontab format.
    ///
    /// # Arguments
    /// * `input` - A string containing crontab entries
    ///
    /// # Returns
    /// * `Result<Self, CrontabParseError>` - The modified WorkerOptions instance or a parse error
    ///
    /// # Example
    /// ```
    /// # use graphile_worker::WorkerOptions;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    ///
    /// // Run the "send_digest" job at 8:00 AM every day
    /// let options = WorkerOptions::default()
    ///     .with_crontab("0 8 * * * send_digest")?;
    /// # Ok(())
    /// # }
    /// ```
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

    /// Sets whether to use local time or PostgreSQL server time for scheduling.
    ///
    /// Affects how crontab schedules and job run_at times are interpreted.
    ///
    /// # Arguments
    /// * `value` - True to use local time, false to use PostgreSQL server time
    ///
    /// # Default
    /// If not specified, defaults to false (use PostgreSQL server time).
    ///
    /// # Note
    /// Using PostgreSQL server time is recommended for consistent behavior
    /// across multiple worker instances, especially in distributed deployments.
    pub fn use_local_time(mut self, value: bool) -> Self {
        self.use_local_time = value;
        self
    }

    /// Adds a custom extension to the worker context.
    ///
    /// Extensions provide custom application state or dependencies
    /// that are accessible from task handlers. Useful for sharing resources
    /// like API clients, configuration, or other state between jobs.
    ///
    /// # Type Parameters
    /// * `T` - Type of the extension (must be Clone, Send, Sync, Debug, and 'static)
    ///
    /// # Arguments
    /// * `value` - The extension instance to add
    ///
    /// # Example
    /// ```
    /// # use graphile_worker::WorkerOptions;
    /// # use std::sync::Arc;
    /// #
    /// # #[derive(Clone, Debug)]
    /// # struct AppConfig {
    /// #     api_key: String,
    /// # }
    /// #
    /// # #[derive(Clone, Debug)]
    /// # struct Database {
    /// #     // Database connection or client
    /// # }
    ///
    /// let config = AppConfig {
    ///     api_key: "secret".to_string(),
    /// };
    ///
    /// let db = Database {
    ///     // Initialize database
    /// };
    ///
    /// let options = WorkerOptions::default()
    ///     .add_extension(config)
    ///     .add_extension(db);
    /// ```
    pub fn add_extension<T: Clone + Send + Sync + Debug + 'static>(mut self, value: T) -> Self {
        self.extensions.insert(value);
        self
    }

    /// Adds a lifecycle hook plugin to the worker.
    ///
    /// Plugins can observe and intercept various worker events such as
    /// job execution, worker startup/shutdown, and cron scheduling.
    /// Multiple plugins can be registered and they execute in registration order.
    ///
    /// # Type Parameters
    /// * `H` - A type implementing the LifecycleHooks trait
    ///
    /// # Arguments
    /// * `hook` - The hook plugin instance to add
    ///
    /// # Example
    /// ```ignore
    /// use graphile_worker::{WorkerOptions, LifecycleHooks, JobStartContext};
    ///
    /// struct LoggingPlugin;
    ///
    /// impl LifecycleHooks for LoggingPlugin {
    ///     async fn on_job_start(&self, ctx: JobStartContext) {
    ///         println!("Job {} starting", ctx.job.id());
    ///     }
    /// }
    ///
    /// let options = WorkerOptions::default()
    ///     .add_plugin(LoggingPlugin);
    /// ```
    pub fn add_plugin<H: LifecycleHooks>(mut self, hook: H) -> Self {
        self.hooks.register(hook);
        self
    }
}
