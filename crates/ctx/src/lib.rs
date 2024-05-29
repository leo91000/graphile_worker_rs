use std::fmt::Debug;

use getset::Getters;
use graphile_worker_extensions::ReadOnlyExtensions;
use graphile_worker_job::Job;
use serde_json::Value;
use sqlx::PgPool;

#[derive(Getters, Clone, Debug)]
#[getset(get = "pub")]
pub struct WorkerContext {
    /// The payload of the job.
    payload: Value,
    /// The postgres pool.
    pg_pool: PgPool,
    /// The job.
    job: Job,
    /// The worker id.
    worker_id: String,
    /// Extensions (for providing custom app state)
    extensions: ReadOnlyExtensions,
}

impl WorkerContext {
    /// Creates a new WorkerContext.
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

    /// Returns the extension value associated with the specified type.
    pub fn get_ext<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.extensions.get()
    }
}
