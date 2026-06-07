use std::time::Duration;

use super::WorkerOptions;
use graphile_worker_database::Schema;

impl WorkerOptions {
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
    pub fn schema(mut self, value: impl Into<Schema>) -> Self {
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
}
