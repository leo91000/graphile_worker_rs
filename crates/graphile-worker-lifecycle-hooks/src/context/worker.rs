use graphile_worker_database::{Database, Schema};
use graphile_worker_extensions::ReadOnlyExtensions;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownReason {
    Signal,
    Error,
    Graceful,
}

#[derive(Clone)]
pub struct WorkerInitContext {
    pub database: Database,
    pub schema: Schema,
    pub concurrency: usize,
}

#[derive(Clone)]
pub struct WorkerStartContext {
    pub database: Database,
    pub worker_id: String,
    pub extensions: ReadOnlyExtensions,
}

#[derive(Clone)]
pub struct WorkerShutdownContext {
    pub database: Database,
    pub worker_id: String,
    pub reason: ShutdownReason,
}

#[derive(Clone)]
pub struct WorkerRecoveredContext {
    pub worker_id: String,
    pub dead_worker_ids: Vec<String>,
    pub jobs_recovered: usize,
}
