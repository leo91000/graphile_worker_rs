use std::fmt::Debug;

use derive_builder::Builder;
use getset::Getters;
use graphile_worker_extensions::ReadOnlyExtensions;
use graphile_worker_job::Job;
pub use graphile_worker_task_details::{SharedTaskDetails, TaskDetails};
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
///
/// Use [`WorkerContextBuilder`] to construct a new instance.
#[derive(Getters, Clone, Debug, Builder)]
#[getset(get = "pub")]
#[builder(build_fn(private, name = "build_internal"), pattern = "owned")]
pub struct WorkerContext {
    /// The JSON payload of the job, containing task-specific data
    payload: Value,
    /// PostgreSQL connection pool for database access during job processing
    pg_pool: PgPool,
    /// SQL-escaped schema name where Graphile Worker tables are located
    #[builder(setter(into))]
    escaped_schema: String,
    /// The complete job record with all metadata
    job: Job,
    /// Unique identifier of the worker processing this job
    #[builder(setter(into))]
    worker_id: String,
    /// Application-specific extensions/state that can be accessed by task handlers
    extensions: ReadOnlyExtensions,
    /// Shared task details mapping task IDs to identifiers
    task_details: SharedTaskDetails,
    /// Whether to use local application time (true) or database time (false) for timestamps
    #[builder(default)]
    use_local_time: bool,
}

impl WorkerContext {
    /// Creates a new builder for constructing a `WorkerContext`.
    pub fn builder() -> WorkerContextBuilder {
        WorkerContextBuilder::default()
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

impl WorkerContextBuilder {
    /// Builds the `WorkerContext`.
    ///
    /// # Panics
    ///
    /// Panics if any required field is not set.
    pub fn build(self) -> WorkerContext {
        self.build_internal()
            .expect("Required field missing in WorkerContextBuilder")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphile_worker_extensions::Extensions;
    use graphile_worker_job::Job;
    use sqlx::postgres::PgPoolOptions;

    fn create_test_job() -> Job {
        Job::builder()
            .id(1)
            .payload(serde_json::json!({"test": "data"}))
            .task_identifier("test_task".to_string())
            .build()
    }

    fn create_test_pool() -> PgPool {
        PgPoolOptions::new()
            .connect_lazy("postgres://test:test@localhost/test")
            .expect("Failed to create lazy pool")
    }

    fn create_extensions() -> ReadOnlyExtensions {
        ReadOnlyExtensions::new(Extensions::default())
    }

    #[tokio::test]
    async fn test_worker_context_builder() {
        let job = create_test_job();
        let pool = create_test_pool();
        let extensions = create_extensions();
        let task_details = SharedTaskDetails::default();

        let ctx = WorkerContext::builder()
            .payload(serde_json::json!({"key": "value"}))
            .pg_pool(pool)
            .escaped_schema("graphile_worker".to_string())
            .job(job)
            .worker_id("worker-1".to_string())
            .extensions(extensions)
            .task_details(task_details)
            .use_local_time(true)
            .build();

        assert_eq!(ctx.payload(), &serde_json::json!({"key": "value"}));
        assert_eq!(ctx.escaped_schema(), "graphile_worker");
        assert_eq!(ctx.worker_id(), "worker-1");
        assert!(ctx.use_local_time());
    }

    #[tokio::test]
    async fn test_worker_context_builder_use_local_time_default() {
        let job = create_test_job();
        let pool = create_test_pool();
        let extensions = create_extensions();
        let task_details = SharedTaskDetails::default();

        let ctx = WorkerContext::builder()
            .payload(serde_json::json!({}))
            .pg_pool(pool)
            .escaped_schema("schema".to_string())
            .job(job)
            .worker_id("worker".to_string())
            .extensions(extensions)
            .task_details(task_details)
            .build();

        assert!(!ctx.use_local_time());
    }

    #[tokio::test]
    #[should_panic(expected = "UninitializedField(\"payload\")")]
    async fn test_worker_context_builder_missing_payload() {
        let job = create_test_job();
        let pool = create_test_pool();
        let extensions = create_extensions();
        let task_details = SharedTaskDetails::default();

        WorkerContext::builder()
            .pg_pool(pool)
            .escaped_schema("schema".to_string())
            .job(job)
            .worker_id("worker".to_string())
            .extensions(extensions)
            .task_details(task_details)
            .build();
    }
}
