use std::sync::Arc;

use graphile_worker_database::{Database, Schema};
use graphile_worker_extensions::ReadOnlyExtensions;
use graphile_worker_job::Job;
use serde_json::Value;

use crate::{SharedTaskDetails, WorkerContextBuilder};

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
    pub(crate) payload: Option<Value>,
    /// Database handle for database access during job processing
    pub(crate) database: Database,
    /// Database schema where Graphile Worker tables are located.
    pub(crate) schema: Schema,
    /// The complete job record with all metadata
    pub(crate) job: Arc<Job>,
    /// Unique identifier of the worker processing this job
    pub(crate) worker_id: String,
    /// Application-specific extensions/state that can be accessed by task handlers
    pub(crate) extensions: ReadOnlyExtensions,
    /// Shared task details mapping task IDs to identifiers
    pub(crate) task_details: SharedTaskDetails,
    /// Whether to use local application time (true) or database time (false) for timestamps
    pub(crate) use_local_time: bool,
}

impl WorkerContext {
    /// Creates a new builder for constructing a `WorkerContext`.
    pub fn builder() -> WorkerContextBuilder {
        WorkerContextBuilder::default()
    }

    pub fn from_shared_job(
        job: Arc<Job>,
        database: impl Into<Database>,
        schema: impl Into<Schema>,
        worker_id: String,
        extensions: ReadOnlyExtensions,
        task_details: SharedTaskDetails,
        use_local_time: bool,
    ) -> Self {
        Self {
            payload: None,
            database: database.into(),
            schema: schema.into(),
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

    pub fn database(&self) -> &Database {
        &self.database
    }

    #[cfg(feature = "driver-sqlx")]
    pub fn try_pg_pool(&self) -> Option<&sqlx::PgPool> {
        self.database
            .downcast_ref::<graphile_worker_database::sqlx::SqlxDatabase>()
            .map(|database| database.pool())
    }

    #[cfg(feature = "driver-sqlx")]
    pub fn pg_pool(&self) -> &sqlx::PgPool {
        self.try_pg_pool()
            .expect("WorkerContext does not use the SQLx database driver")
    }

    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    pub fn job(&self) -> &Job {
        self.job.as_ref()
    }

    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    pub fn extensions(&self) -> &ReadOnlyExtensions {
        &self.extensions
    }

    pub fn task_details(&self) -> &SharedTaskDetails {
        &self.task_details
    }

    pub fn use_local_time(&self) -> bool {
        self.use_local_time
    }

    /// Retrieves a reference to an extension value by its type.
    ///
    /// This method allows task handlers to access custom application state
    /// that was registered with the worker. Extensions are identified by their
    /// Rust type.
    pub fn get_ext<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.extensions.get()
    }
}
