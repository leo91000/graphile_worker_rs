use std::fmt::{self, Debug, Display};

use crate::errors::GraphileWorkerError;
use graphile_worker_crontab_runner::ScheduleCronJobError;
use graphile_worker_runtime as runtime;
use thiserror::Error;

/// Errors that can occur during worker runtime.
///
/// These errors represent various failure scenarios that can happen
/// while the worker is running and processing jobs.
#[derive(Error, Debug)]
pub enum WorkerRuntimeError {
    /// An error occurred while processing or releasing a job
    #[error("Unexpected error occurred while processing job : '{0}'")]
    ProcessJob(#[from] ProcessJobError),
    #[error("Worker task failed : '{0}'")]
    WorkerTask(#[from] runtime::JoinError),
    /// Failed to listen to PostgreSQL notifications for new jobs
    #[error("Failed to listen to postgres notifications : '{0}'")]
    PgListen(#[from] GraphileWorkerError),
    /// An error occurred while scheduling or executing a cron job
    #[error("Error occurred while trying to schedule cron job : {0}")]
    Crontab(#[from] ScheduleCronJobError),
}

/// Errors that can occur while processing a job.
#[derive(Error, Debug)]
pub enum ProcessJobError {
    /// Error occurred when trying to complete or fail a job after processing
    #[error("An error occurred while releasing a job : '{0}'")]
    ReleaseJobError(#[from] ReleaseJobError),
    /// Error occurred when trying to fetch a job from the database
    #[error("An error occurred while fetching a job to run : '{0}'")]
    GetJobError(#[from] GraphileWorkerError),
}

/// Errors that can occur during the execution of a job's task handler.
#[derive(Error, Debug)]
pub(super) enum RunJobError {
    /// No task identifier was found for the given task ID
    #[error("Cannot find any task identifier for given task id '{0}'. This is probably a bug !")]
    IdentifierNotFound(i32),
    /// No task handler function was registered for the given task identifier
    #[error("Cannot find any task fn for given task identifier '{0}'. This is probably a bug !")]
    FnNotFound(String),
    /// The task handler panicked during execution
    #[error("Task failed execution to complete : {0}")]
    TaskPanic(String),
    /// The task handler returned an error string
    #[error("Task returned the following error : {0}")]
    TaskError(String),
    /// The batch task handler returned partial failures with a replacement payload
    #[error("Task returned the following error : {message}")]
    TaskErrorWithReplacement {
        message: String,
        replacement_payload: Redacted<serde_json::Value>,
    },
    /// The task was aborted due to a shutdown signal
    #[error("Task was aborted by shutdown signal")]
    TaskAborted,
}

pub(super) struct Redacted<T>(T);

impl<T> Redacted<T> {
    pub(super) fn new(value: T) -> Self {
        Self(value)
    }

    fn get(&self) -> &T {
        &self.0
    }
}

impl<T> Debug for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

impl<T> Display for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

impl RunJobError {
    pub(super) fn persisted_error(&self) -> String {
        match self {
            RunJobError::TaskErrorWithReplacement { message, .. } => {
                format!("TaskError({message:?})")
            }
            _ => format!("{self:?}"),
        }
    }

    pub(super) fn replacement_payload(&self) -> Option<&serde_json::Value> {
        match self {
            RunJobError::TaskErrorWithReplacement {
                replacement_payload,
                ..
            } => Some(replacement_payload.get()),
            _ => None,
        }
    }
}

/// Error that occurs when trying to mark a job as completed or failed.
#[derive(Error, Debug)]
#[error("Failed to release job '{job_id}'. {source}")]
pub struct ReleaseJobError {
    /// The ID of the job that could not be released
    pub(super) job_id: i64,
    /// The underlying error that caused the release operation to fail
    #[source]
    pub(super) source: GraphileWorkerError,
}
