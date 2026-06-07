use std::sync::Arc;

use graphile_worker_database::{Database, Schema};
use graphile_worker_extensions::ReadOnlyExtensions;
use graphile_worker_job::Job;
use serde_json::Value;

use crate::{SharedTaskDetails, WorkerContext};

#[derive(Clone, Default, Debug)]
pub struct WorkerContextBuilder {
    payload: Option<Value>,
    database: Option<Database>,
    schema: Option<Schema>,
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

    pub fn database(mut self, database: impl Into<Database>) -> Self {
        self.database = Some(database.into());
        self
    }

    #[cfg(feature = "driver-sqlx")]
    pub fn pg_pool(mut self, pg_pool: sqlx::PgPool) -> Self {
        self.database = Some(pg_pool.into());
        self
    }

    pub fn schema(mut self, schema: impl Into<Schema>) -> Self {
        self.schema = Some(schema.into());
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
            payload: self.payload,
            database: self.database.unwrap_or_else(|| missing_field("database")),
            schema: self.schema.unwrap_or_else(|| missing_field("schema")),
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
