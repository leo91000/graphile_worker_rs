use thiserror::Error;

/// Errors that can occur during Graphile Worker operations.
///
/// This enum represents the various errors that can occur when interacting
/// with the job queue database or processing jobs.
#[derive(Error, Debug)]
pub enum GraphileWorkerError {
    /// An error occurred while executing an SQL query
    #[error("Error occured while query: {0}")]
    SqlError(#[from] sqlx::Error),

    /// An error occurred while serializing or deserializing JSON data
    #[error("Error while serializing params: {0}")]
    JsonSerializeError(#[from] serde_json::Error),

    /// Job scheduling was skipped by a before_job_schedule hook
    #[error("Job scheduling was skipped by hook")]
    JobScheduleSkipped,

    /// Job scheduling failed due to a before_job_schedule hook
    #[error("Job scheduling failed: {0}")]
    JobScheduleFailed(String),
}

/// A Result type alias for GraphileWorkerError.
///
/// This type alias simplifies the return types for functions that can
/// return a GraphileWorkerError.
pub type Result<T> = core::result::Result<T, GraphileWorkerError>;
