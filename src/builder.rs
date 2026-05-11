use crate::batcher::{CompletionBatcher, FailureBatcher};
use crate::cron::CronBuilder;
use crate::local_queue::LocalQueueConfig;
use crate::runner::WorkerFn;
use crate::sql::task_identifiers::{get_tasks_details, SharedTaskDetails};
use crate::utils::escape_identifier;
use crate::Worker;
use futures::FutureExt;
use graphile_worker_crontab_parser::{parse_crontab, CrontabParseError};
use graphile_worker_crontab_types::Crontab;
use graphile_worker_ctx::WorkerContext;
use graphile_worker_database::{Database, DbError};
use graphile_worker_extensions::Extensions;
use graphile_worker_lifecycle_hooks::{Event, HookRegistry, Plugin};
use graphile_worker_migrations::migrate;
use graphile_worker_runtime::Notify;
use graphile_worker_shutdown_signal::{shutdown_signal, ShutdownSignal};
use graphile_worker_task_handler::{run_task_from_worker_ctx, TaskHandler};
use rand::Rng;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

/// Input accepted by [`WorkerOptions::with_cron`].
///
/// Typed cron builders and raw [`Crontab`] values are infallible and return
/// `WorkerOptions` directly. Crontab text is parsed and returns
/// `Result<WorkerOptions, CrontabParseError>`.
pub trait CronInput {
    type Output;

    fn append_to(self, options: WorkerOptions) -> Self::Output;
}

impl CronInput for Crontab {
    type Output = WorkerOptions;

    fn append_to(self, mut options: WorkerOptions) -> Self::Output {
        options.append_crontabs(vec![self]);
        options
    }
}

impl<T: TaskHandler> CronInput for CronBuilder<T> {
    type Output = WorkerOptions;

    fn append_to(self, options: WorkerOptions) -> Self::Output {
        self.build().append_to(options)
    }
}

impl CronInput for &str {
    type Output = Result<WorkerOptions, CrontabParseError>;

    fn append_to(self, mut options: WorkerOptions) -> Self::Output {
        let crontabs = parse_crontab(self)?;
        options.append_crontabs(crontabs);
        Ok(options)
    }
}

impl CronInput for String {
    type Output = Result<WorkerOptions, CrontabParseError>;

    fn append_to(self, options: WorkerOptions) -> Self::Output {
        self.as_str().append_to(options)
    }
}

impl CronInput for &String {
    type Output = Result<WorkerOptions, CrontabParseError>;

    fn append_to(self, options: WorkerOptions) -> Self::Output {
        self.as_str().append_to(options)
    }
}

/// Creates a shutdown signal that can be triggered manually via the returned notifier.
fn manual_shutdown_signal_pair() -> (ShutdownSignal, Arc<Notify>) {
    let notify = Arc::new(Notify::new());
    let notify_for_signal = notify.clone();
    let signal = async move {
        notify_for_signal.notified().await;
    }
    .boxed()
    .shared();

    (signal, notify)
}

/// Resolves as soon as either of the provided shutdown signals completes.
fn combine_shutdown_signals(left: ShutdownSignal, right: ShutdownSignal) -> ShutdownSignal {
    async move {
        let left = left.fuse();
        let right = right.fuse();
        futures::pin_mut!(left, right);
        futures::select_biased! {
            _ = left => (),
            _ = right => (),
        };
    }
    .boxed()
    .shared()
}

#[cfg(feature = "driver-sqlx")]
async fn connect_default_database(db_url: &str, max_connections: u32) -> Result<Database, DbError> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(db_url)
        .await
        .map_err(DbError::from)?;
    Ok(pool.into())
}

#[cfg(all(not(feature = "driver-sqlx"), feature = "driver-tokio-postgres"))]
async fn connect_default_database(db_url: &str, max_connections: u32) -> Result<Database, DbError> {
    let database = graphile_worker_database::tokio_postgres::TokioPostgresDatabase::from_url(
        db_url,
        max_connections as usize,
    )?;
    Ok(database.into())
}

#[cfg(not(any(feature = "driver-sqlx", feature = "driver-tokio-postgres")))]
async fn connect_default_database(
    _db_url: &str,
    _max_connections: u32,
) -> Result<Database, DbError> {
    Err(DbError::new(
        "database_url requires enabling a database driver feature",
    ))
}

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
}

/// Errors that can occur when initializing a worker.
#[derive(Error, Debug)]
pub enum WorkerBuildError {
    /// Failed to connect to the PostgreSQL database
    #[error("Error occurred while connecting to the PostgreSQL database: {0}")]
    ConnectError(#[from] DbError),

    /// Failed while executing a database query
    #[error("Error occurred while executing a query: {0}")]
    QueryError(#[from] crate::errors::GraphileWorkerError),

    /// The database URL was not provided and no Database was supplied
    #[error("Missing database configuration - must provide either database_url or database")]
    MissingDatabaseUrl,

    /// Failed to apply database migrations
    #[error("Error occurred while migrating the database schema: {0}")]
    MigrationError(#[from] graphile_worker_migrations::MigrateError),
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
        let escaped_schema = escape_identifier(&database, &schema).await?;

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

    /// Sets an existing PostgreSQL database connection for the worker to use.
    ///
    /// Allows reusing an existing driver-specific pool or configuring the
    /// database connection with custom settings before passing it to the worker.
    ///
    /// # Arguments
    /// * `value` - The PostgreSQL database connection
    ///
    /// # Note
    /// If both `database` and `database_url` are provided, `database` takes precedence.
    pub fn database(mut self, value: impl Into<Database>) -> Self {
        self.database = Some(value.into());
        self
    }

    /// Sets an existing SQLx PostgreSQL pool for the worker to use.
    ///
    /// This is a SQLx-only convenience wrapper over [`Self::database`]. Prefer
    /// [`Self::database`] when passing a driver-agnostic database connection.
    ///
    /// # Note
    /// If both `pg_pool` and `database_url` are provided, `pg_pool` takes precedence.
    #[cfg(feature = "driver-sqlx")]
    pub fn pg_pool(mut self, value: sqlx::PgPool) -> Self {
        self.database = Some(value.into());
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
    /// Either `database`, `pg_pool`, or `database_url` must be provided before calling `init()`.
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
            .insert(identifier.to_string(), Arc::new(worker_fn));
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

    /// Adds cron entries for scheduled jobs.
    ///
    /// This accepts typed cron builders, raw [`Crontab`] values, and crontab
    /// text. Typed inputs return `WorkerOptions` directly; text input returns
    /// `Result<WorkerOptions, CrontabParseError>`.
    ///
    /// # Typed example
    /// ```
    /// # use graphile_worker::{Cron, CrontabFill, WorkerOptions};
    /// # use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
    /// # use serde::{Deserialize, Serialize};
    /// #
    /// # #[derive(Deserialize, Serialize)]
    /// # struct SendDigest;
    /// #
    /// # impl TaskHandler for SendDigest {
    /// #     const IDENTIFIER: &'static str = "send_digest";
    /// #     async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {}
    /// # }
    ///
    /// let options = WorkerOptions::default()
    ///     .define_job::<SendDigest>()
    ///     .with_cron(
    ///         Cron::daily_at::<SendDigest>(8, 0)
    ///             .expect("valid cron schedule")
    ///             .fill(CrontabFill::hours(1)),
    ///     );
    /// ```
    ///
    /// # Crontab text example
    /// ```
    /// # use graphile_worker::WorkerOptions;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let options = WorkerOptions::default()
    ///     .with_cron("0 8 * * * send_digest")?;
    /// # let _ = options;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_cron<C: CronInput>(self, cron: C) -> C::Output {
        cron.append_to(self)
    }

    /// Adds typed cron entries for scheduled jobs.
    pub fn with_crons<I, C>(mut self, crontabs: I) -> Self
    where
        I: IntoIterator<Item = C>,
        C: Into<Crontab>,
    {
        self.append_crontabs(crontabs.into_iter().map(Into::into).collect());
        self
    }

    /// Adds crontab text entries for scheduled jobs.
    ///
    /// Use [`Self::with_cron`] with a string instead.
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
    /// // Run the "send_digest" job at 8:00 AM every day.
    /// let options = WorkerOptions::default()
    ///     .with_cron("0 8 * * * send_digest")?;
    /// # Ok(())
    /// # }
    /// ```
    #[deprecated(note = "use WorkerOptions::with_cron(...) instead")]
    pub fn with_crontab(self, input: &str) -> Result<Self, CrontabParseError> {
        self.with_cron(input)
    }

    /// Sets whether to use local application time or database time for timestamps.
    ///
    /// When `use_local_time` is true, the application's `Utc::now()` is used for timestamps,
    /// which can help handle clock drift between the application server and database server.
    /// When false (default), PostgreSQL's `now()` is used instead.
    ///
    /// Affects job fetching, job scheduling, and crontab scheduling.
    ///
    /// # Arguments
    /// * `value` - True to use application time, false to use database time
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

    /// Controls whether the worker installs OS-level shutdown signal handlers.
    ///
    /// By default Graphile Worker listens to signals like SIGINT/SIGTERM to
    /// trigger a graceful shutdown. Embedding applications that already manage
    /// signal handling can disable this behavior by setting the value to `false`.
    ///
    /// # Arguments
    /// * `value` - `true` to install the default OS signal listeners, `false` to skip them
    pub fn listen_os_shutdown_signals(mut self, value: bool) -> Self {
        self.listen_os_shutdown_signals = Some(value);
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

    /// Registers an event handler for a specific lifecycle event.
    ///
    /// This method allows registering individual handlers for specific events
    /// without creating a full plugin. The handler is a closure that receives
    /// the event context and returns the appropriate output type.
    ///
    /// # Type Parameters
    /// * `E` - The event type to handle
    ///
    /// # Arguments
    /// * `event` - The event marker (e.g., `JobStart`, `BeforeJobRun`)
    /// * `handler` - The async handler closure
    ///
    /// # Example
    /// ```ignore
    /// use graphile_worker::{WorkerOptions, JobStart, BeforeJobRun, HookResult};
    ///
    /// let options = WorkerOptions::default()
    ///     .on(JobStart, |ctx| async move {
    ///         println!("Job {} starting", ctx.job.id());
    ///     })
    ///     .on(BeforeJobRun, |ctx| async move {
    ///         if ctx.payload.get("skip").and_then(|v| v.as_bool()).unwrap_or(false) {
    ///             return HookResult::Skip;
    ///         }
    ///         HookResult::Continue
    ///     });
    /// ```
    pub fn on<E, F, Fut>(mut self, event: E, handler: F) -> Self
    where
        E: Event,
        F: Fn(E::Context) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = E::Output> + Send + 'static,
    {
        self.hooks.on(event, handler);
        self
    }

    /// Adds a lifecycle hook plugin to the worker.
    ///
    /// Plugins can observe and intercept various worker events such as
    /// job execution, worker startup/shutdown, and cron scheduling.
    /// Multiple plugins can be registered and they execute in registration order.
    ///
    /// # Type Parameters
    /// * `P` - A type implementing the Plugin trait
    ///
    /// # Arguments
    /// * `plugin` - The plugin instance to add
    ///
    /// # Example
    /// ```ignore
    /// use graphile_worker::{WorkerOptions, Plugin, HookRegistry, JobStart};
    ///
    /// struct LoggingPlugin;
    ///
    /// impl Plugin for LoggingPlugin {
    ///     fn register(self, hooks: &mut HookRegistry) {
    ///         hooks.on::<JobStart>(|ctx| async move {
    ///             println!("Job {} starting", ctx.job.id());
    ///         });
    ///     }
    /// }
    ///
    /// let options = WorkerOptions::default()
    ///     .add_plugin(LoggingPlugin);
    /// ```
    pub fn add_plugin<P: Plugin>(mut self, plugin: P) -> Self {
        plugin.register(&mut self.hooks);
        self
    }

    /// Enables the LocalQueue with the specified configuration.
    ///
    /// LocalQueue batch-fetches jobs from the database to reduce DB load,
    /// trading latency for throughput. Jobs are cached locally and distributed
    /// to workers without additional database queries until the cache is empty.
    ///
    /// # Arguments
    /// * `config` - The LocalQueue configuration (size and TTL)
    ///
    /// # Note
    /// When LocalQueue is enabled, jobs may experience slightly higher latency
    /// as they wait in the local cache. The cache has a TTL after which
    /// unclaimed jobs are returned to the database.
    ///
    /// `LocalQueueConfig::queue_count` can be raised above 1 to run multiple
    /// independent local queues in the same worker. `size` is applied per queue,
    /// so total local capacity is `size * queue_count`.
    /// There is no universal best setting for throughput; benchmark realistic
    /// jobs with your own PostgreSQL latency, pool size, worker concurrency, and
    /// local queue settings before tuning this in production.
    ///
    /// Workers with `forbidden_flags` will bypass the LocalQueue and fetch
    /// jobs directly from the database.
    ///
    /// # Example
    /// ```
    /// # use graphile_worker::{WorkerOptions, LocalQueueConfig, RefetchDelayConfig};
    /// # use std::time::Duration;
    ///
    /// let options = WorkerOptions::default()
    ///     .local_queue(
    ///         LocalQueueConfig::default()
    ///             .with_size(100)
    ///             .with_queue_count(2)
    ///             .with_ttl(Duration::from_secs(300))
    ///             .with_refetch_delay(
    ///                 RefetchDelayConfig::default()
    ///                     .with_duration(Duration::from_millis(100))
    ///                     .with_threshold(10)
    ///                     .with_max_abort_threshold(500),
    ///             ),
    ///     );
    /// ```
    pub fn local_queue(mut self, config: LocalQueueConfig) -> Self {
        self.local_queue_config = Some(config);
        self
    }

    /// Sets the delay before flushing batched job completions.
    ///
    /// When configured, job completions are collected in a batch and flushed
    /// together after the specified delay. This reduces the number of SQL
    /// round trips and can significantly improve throughput.
    ///
    /// # Arguments
    ///
    /// * `delay` - The duration to wait before flushing the batch.
    ///   A small value like 1-5ms is recommended.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use graphile_worker::WorkerOptions;
    /// # use std::time::Duration;
    /// let worker = WorkerOptions::default()
    ///     .complete_job_batch_delay(Duration::from_millis(5));
    /// ```
    pub fn complete_job_batch_delay(mut self, delay: Duration) -> Self {
        self.complete_job_batch_delay = Some(delay);
        self
    }

    /// Sets the delay before flushing batched job failures.
    ///
    /// When configured, permanent job failures are collected and flushed
    /// together after the specified delay. Retryable failures are still
    /// processed individually to ensure proper backoff timing.
    ///
    /// # Arguments
    ///
    /// * `delay` - The duration to wait before flushing the batch.
    ///   A small value like 1-5ms is recommended.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use graphile_worker::WorkerOptions;
    /// # use std::time::Duration;
    /// let worker = WorkerOptions::default()
    ///     .fail_job_batch_delay(Duration::from_millis(5));
    /// ```
    pub fn fail_job_batch_delay(mut self, delay: Duration) -> Self {
        self.fail_job_batch_delay = Some(delay);
        self
    }
}
