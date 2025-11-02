use chrono::{DateTime, Utc};
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

/// A variant of this enum is emitted at each lifecycle point.
#[derive(Debug, Clone)]
pub enum LifeCycleEvent {
    /// Variant emitted when a job is being created, before database insert.
    Creating(JobCreating),
    /// Variant emitted after a job has been created and inserted into the database.
    Created(JobCreated),
    /// Variant emitted when a job starts execution.
    Started(JobStarted),
    /// Variant emitted when a job completes successfully.
    Completed(JobCompleted),
    /// Variant emitted when a job fails.
    Failed(JobFailed),
}

/// Event emitted when a job is being created, before database insert.
#[derive(Debug, Clone)]
pub struct JobCreating {
    /// Task identifier (e.g., "send_email")
    pub task_identifier: String,
    /// Optional queue name for serialized execution
    pub queue_name: Option<String>,
    /// Job priority (higher values = higher priority)
    pub priority: i16,
    /// Job payload as JSON value
    pub payload: serde_json::Value,
    /// Optional scheduled run time (None for immediate execution)
    pub run_at: Option<DateTime<Utc>>,
}

/// Event emitted after a job has been created and inserted into the database.
#[derive(Debug, Clone)]
pub struct JobCreated {
    /// Unique identifier assigned to the job
    pub job_id: i64,
    /// Task identifier (e.g., "send_email")
    pub task_identifier: String,
    /// Optional queue name for serialized execution
    pub queue_name: Option<String>,
    /// Job priority (higher values = higher priority)
    pub priority: i16,
}

/// Event emitted when a job starts execution.
#[derive(Debug, Clone)]
pub struct JobStarted {
    /// Unique identifier for the job
    pub job_id: i64,
    /// Task identifier (e.g., "send_email")
    pub task_identifier: String,
    /// Optional queue name for serialized execution
    pub queue_name: Option<String>,
    /// Job priority (higher values = higher priority)
    pub priority: i16,
    /// Number of execution attempts (1 for first attempt)
    pub attempts: i16,
    /// Job payload as JSON value (useful for extracting trace context)
    pub payload: serde_json::Value,
}

/// Event emitted when a job completes successfully.
#[derive(Debug, Clone)]
pub struct JobCompleted {
    /// Unique identifier for the job
    pub job_id: i64,
    /// Task identifier (e.g., "send_email")
    pub task_identifier: String,
    /// Optional queue name for serialized execution
    pub queue_name: Option<String>,
    /// Duration of job execution
    pub duration: Duration,
    /// Number of execution attempts (includes retries)
    pub attempts: i16,
}

/// Event emitted when a job fails.
#[derive(Debug, Clone)]
pub struct JobFailed {
    /// Unique identifier for the job
    pub job_id: i64,
    /// Task identifier (e.g., "send_email")
    pub task_identifier: String,
    /// Optional queue name for serialized execution
    pub queue_name: Option<String>,
    /// Error message from the failed execution
    pub error: String,
    /// Duration of job execution before failure
    pub duration: Duration,
    /// Number of execution attempts (includes retries)
    pub attempts: i16,
    /// Whether the job will be retried
    pub will_retry: bool,
}

/// Trait for implementing a lifecycle hook on job execution.
///
/// This trait allows you to observe job execution lifecycle events for
/// observability, metrics collection, logging, or other cross-cutting concerns.
///
/// Implement the single method, match on event, and handle the variants you are
/// interested in. For example, you might emit metrics on each event.
///
/// Multiple hooks can be registered and will execute concurrently for each event.
///
/// # Example
///
/// ```rust
/// use graphile_worker_hooks::{LifeCycleHook, LifeCycleEvent};
/// use std::future::Future;
/// use std::pin::Pin;
///
/// struct MetricsHook;
///
/// impl LifeCycleHook for MetricsHook {
///     fn on_event(&self, event: LifeCycleEvent) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> {
///         Box::pin(async move {
///             match event {
///                 /// Called when a job is being created, before database insert.
///                 LifeCycleEvent::Creating(creating) => todo!(),
///                 /// Called after a job has been created and inserted.
///                 LifeCycleEvent::Created(created) => todo!(),
///                 /// Called immediately before the task handler runs.
///                 LifeCycleEvent::Started(started) => todo!(),
///                 /// Called when a job completes successfully, after the task handler
///                 /// returns Ok and before the job is marked completed in the database.
///                 LifeCycleEvent::Completed(completed) => todo!(),
///                 /// Called when a job fails, after the task handler returns an error
///                 /// and before the job is marked as failed or scheduled for retry.
///                 LifeCycleEvent::Failed(failed) => todo!(),
///             }
///         })
///     }
/// }
/// ```
pub trait LifeCycleHook: Send + Sync {
    /// Called on any job lifecycle event.
    fn on_event(
        &self,
        _event: LifeCycleEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> {
        Box::pin(async {})
    }
}

/// Unit type implementation provides zero-cost abstraction when hooks are not used.
impl LifeCycleHook for () {}
