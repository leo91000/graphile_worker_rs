use std::fmt::Debug;

use getset::Getters;
use graphile_worker_extensions::ReadOnlyExtensions;
use graphile_worker_job::Job;
use serde_json::Value;
use sqlx::PgPool;

/// Context provided to task handlers when processing a job.
///
/// `WorkerContext` provides task handlers with access to:
/// - The job payload data
/// - The complete job record with all metadata
/// - A PostgreSQL connection pool for database operations
/// - Worker identification information
/// - Any custom application state via the extensions system
///
/// This context is created by the worker and passed to task handlers,
/// providing everything they need to process a job within the task handler's
/// execution environment.
#[derive(Getters, Clone, Debug)]
#[getset(get = "pub")]
pub struct WorkerContext {
    /// The JSON payload of the job, containing task-specific data
    payload: Value,
    /// PostgreSQL connection pool for database access during job processing
    pg_pool: PgPool,
    /// The complete job record with all metadata
    job: Job,
    /// Unique identifier of the worker processing this job
    worker_id: String,
    /// Application-specific extensions/state that can be accessed by task handlers
    extensions: ReadOnlyExtensions,
}

impl WorkerContext {
    /// Creates a new WorkerContext with all required components.
    ///
    /// # Arguments
    ///
    /// * `payload` - The job's JSON payload data
    /// * `pg_pool` - PostgreSQL connection pool
    /// * `job` - The complete job record
    /// * `worker_id` - Identifier for the worker processing this job
    /// * `extensions` - Custom application state/extensions
    ///
    /// # Returns
    ///
    /// A new `WorkerContext` instance that will be passed to the task handler
    pub fn new(
        payload: Value,
        pg_pool: PgPool,
        job: Job,
        worker_id: String,
        extensions: ReadOnlyExtensions,
    ) -> Self {
        WorkerContext {
            payload,
            pg_pool,
            job,
            worker_id,
            extensions,
        }
    }

    /// Retrieves a reference to an extension value by its type.
    ///
    /// This method allows task handlers to access custom application state
    /// that was registered with the worker. Extensions are identified by their
    /// Rust type.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type of extension to retrieve
    ///
    /// # Returns
    ///
    /// * `Some(&T)` - A reference to the extension value if it exists
    /// * `None` - If no extension of the requested type is registered
    ///
    /// # Examples
    ///
    /// ```
    /// # use graphile_worker_ctx::WorkerContext;
    /// struct AppState { db_config: String }
    ///
    /// // In your task handler:
    /// fn handle_task(ctx: &WorkerContext) {
    ///     if let Some(state) = ctx.get_ext::<AppState>() {
    ///         // Use state.db_config
    ///     }
    /// }
    /// ```
    pub fn get_ext<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.extensions.get()
    }
}
