use std::fmt::Debug;
use std::sync::Arc;

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
#[derive(Clone, Debug)]
pub struct WorkerContext {
    /// The JSON payload of the job, containing task-specific data
    payload: Option<Value>,
    /// PostgreSQL connection pool for database access during job processing
    pg_pool: PgPool,
    /// SQL-escaped schema name where Graphile Worker tables are located
    escaped_schema: String,
    /// The complete job record with all metadata
    job: Arc<Job>,
    /// Unique identifier of the worker processing this job
    worker_id: String,
    /// Application-specific extensions/state that can be accessed by task handlers
    extensions: ReadOnlyExtensions,
    /// Shared task details mapping task IDs to identifiers
    task_details: SharedTaskDetails,
    /// Whether to use local application time (true) or database time (false) for timestamps
    use_local_time: bool,
}

impl WorkerContext {
    /// Creates a new builder for constructing a `WorkerContext`.
    pub fn builder() -> WorkerContextBuilder {
        WorkerContextBuilder::default()
    }

    pub fn from_shared_job(
        job: Arc<Job>,
        pg_pool: PgPool,
        escaped_schema: String,
        worker_id: String,
        extensions: ReadOnlyExtensions,
        task_details: SharedTaskDetails,
        use_local_time: bool,
    ) -> Self {
        Self {
            payload: None,
            pg_pool,
            escaped_schema,
            job,
            worker_id,
            extensions,
            task_details,
            use_local_time,
        }
    }

    pub fn payload(&self) -> &Value {
        self.payload.as_ref().unwrap_or_else(|| self.job.payload())
    }

    pub fn pg_pool(&self) -> &PgPool {
        &self.pg_pool
    }

    pub fn escaped_schema(&self) -> &String {
        &self.escaped_schema
    }

    pub fn job(&self) -> &Job {
        self.job.as_ref()
    }

    pub fn worker_id(&self) -> &String {
        &self.worker_id
    }

    pub fn extensions(&self) -> &ReadOnlyExtensions {
        &self.extensions
    }

    pub fn task_details(&self) -> &SharedTaskDetails {
        &self.task_details
    }

    pub fn use_local_time(&self) -> &bool {
        &self.use_local_time
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

#[derive(Clone, Default, Debug)]
pub struct WorkerContextBuilder {
    payload: Option<Value>,
    pg_pool: Option<PgPool>,
    escaped_schema: Option<String>,
    job: Option<Job>,
    worker_id: Option<String>,
    extensions: Option<ReadOnlyExtensions>,
    task_details: Option<SharedTaskDetails>,
    use_local_time: Option<bool>,
}

impl WorkerContextBuilder {
    pub fn payload(mut self, payload: Value) -> Self {
        self.payload = Some(payload);
        self
    }

    pub fn pg_pool(mut self, pg_pool: PgPool) -> Self {
        self.pg_pool = Some(pg_pool);
        self
    }

    pub fn escaped_schema(mut self, escaped_schema: impl Into<String>) -> Self {
        self.escaped_schema = Some(escaped_schema.into());
        self
    }

    pub fn job(mut self, job: Job) -> Self {
        self.job = Some(job);
        self
    }

    pub fn worker_id(mut self, worker_id: impl Into<String>) -> Self {
        self.worker_id = Some(worker_id.into());
        self
    }

    pub fn extensions(mut self, extensions: ReadOnlyExtensions) -> Self {
        self.extensions = Some(extensions);
        self
    }

    pub fn task_details(mut self, task_details: SharedTaskDetails) -> Self {
        self.task_details = Some(task_details);
        self
    }

    pub fn use_local_time(mut self, use_local_time: bool) -> Self {
        self.use_local_time = Some(use_local_time);
        self
    }

    pub fn build(self) -> WorkerContext {
        WorkerContext {
            payload: Some(self.payload.unwrap_or_else(|| missing_field("payload"))),
            pg_pool: self.pg_pool.unwrap_or_else(|| missing_field("pg_pool")),
            escaped_schema: self
                .escaped_schema
                .unwrap_or_else(|| missing_field("escaped_schema")),
            job: Arc::new(self.job.unwrap_or_else(|| missing_field("job"))),
            worker_id: self.worker_id.unwrap_or_else(|| missing_field("worker_id")),
            extensions: self
                .extensions
                .unwrap_or_else(|| missing_field("extensions")),
            task_details: self
                .task_details
                .unwrap_or_else(|| missing_field("task_details")),
            use_local_time: self.use_local_time.unwrap_or_default(),
        }
    }
}

fn missing_field<T>(field: &str) -> T {
    panic!("UninitializedField(\"{field}\")")
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

    #[derive(Clone, Debug)]
    struct TestExtension {
        value: &'static str,
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
    async fn test_worker_context_from_shared_job_uses_job_payload() {
        let job = std::sync::Arc::new(create_test_job());
        let pool = create_test_pool();
        let mut extensions = Extensions::default();
        extensions.insert(TestExtension { value: "present" });
        let extensions = ReadOnlyExtensions::new(extensions);
        let task_details = SharedTaskDetails::default();

        let ctx = WorkerContext::from_shared_job(
            job.clone(),
            pool,
            "graphile_worker".to_string(),
            "worker-1".to_string(),
            extensions,
            task_details,
            true,
        );

        assert_eq!(ctx.payload(), job.payload());
        assert_eq!(ctx.job().id(), job.id());
        assert_eq!(ctx.escaped_schema(), "graphile_worker");
        assert_eq!(ctx.worker_id(), "worker-1");
        assert!(ctx.use_local_time());
        assert_eq!(
            ctx.extensions().get::<TestExtension>().unwrap().value,
            "present"
        );
        assert_eq!(ctx.get_ext::<TestExtension>().unwrap().value, "present");
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
