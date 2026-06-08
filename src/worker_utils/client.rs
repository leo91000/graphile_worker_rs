use std::sync::Arc;

use graphile_worker_database::{Database, Schema};
use graphile_worker_lifecycle_hooks::HookRegistry;

use graphile_worker_queries::task_identifiers::SharedTaskDetails;

/// The WorkerUtils struct provides a set of utility methods for managing jobs.
///
/// This is the primary interface for adding jobs to the queue, managing existing jobs,
/// performing maintenance tasks, and migrating the database schema.
#[derive(Clone)]
pub struct WorkerUtils {
    /// Database connection pool
    pub(super) database: Database,

    /// Database schema where Graphile Worker tables are located.
    pub(super) schema: Schema,

    /// Optional lifecycle hooks for intercepting job scheduling
    pub(super) hooks: Option<Arc<HookRegistry>>,

    /// Shared task details for refreshing after GcTaskIdentifiers cleanup
    pub(super) task_details: SharedTaskDetails,

    /// Whether to use local application time (true) or database time (false) for timestamps
    pub(super) use_local_time: bool,
}

impl WorkerUtils {
    /// Creates a new instance of WorkerUtils.
    ///
    /// # Arguments
    /// * `database` - Database connection handle
    /// * `schema` - The schema where Graphile Worker tables are stored
    ///
    /// # Returns
    /// A new WorkerUtils instance
    pub fn new(database: impl Into<Database>, schema: impl Into<Schema>) -> Self {
        Self {
            database: database.into(),
            schema: schema.into(),
            hooks: None,
            task_details: SharedTaskDetails::default(),
            use_local_time: false,
        }
    }

    /// Adds lifecycle hooks to this WorkerUtils instance.
    pub fn with_hooks(mut self, hooks: Arc<HookRegistry>) -> Self {
        self.hooks = Some(hooks);
        self
    }

    /// Adds task details to this WorkerUtils instance.
    ///
    /// When task_details is provided, cleanup operations that include `GcTaskIdentifiers`
    /// will automatically refresh the task details to ensure the worker can still pick
    /// up jobs after task identifiers are garbage collected.
    pub fn with_task_details(mut self, task_details: SharedTaskDetails) -> Self {
        self.task_details = task_details;
        self
    }

    /// Sets whether to use local application time or database time for timestamps.
    ///
    /// When `use_local_time` is true, the application's `Utc::now()` is used for timestamps,
    /// which can help handle clock drift between the application server and database server.
    /// When false (default), PostgreSQL's `now()` is used instead.
    pub fn with_use_local_time(mut self, use_local_time: bool) -> Self {
        self.use_local_time = use_local_time;
        self
    }
}
