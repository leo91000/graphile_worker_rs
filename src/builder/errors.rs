use crate::local_queue::LocalQueueConfigError;
use graphile_worker_database::DbError;
use thiserror::Error;

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

    /// Local queue configuration is invalid
    #[error("Invalid local queue configuration: {0}")]
    LocalQueueConfig(#[from] LocalQueueConfigError),
}
