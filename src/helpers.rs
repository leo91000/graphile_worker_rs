use archimedes_task_handler::TaskDefinition;
use serde::Serialize;
use sqlx::PgPool;

use crate::{errors::ArchimedesError, sql::add_job::add_job, JobSpec, WorkerContext};

/// The WorkerHelpers struct provides a set of methods to add jobs to the queue
pub struct WorkerHelpers {
    pg_pool: PgPool,
    escaped_schema: String,
}

impl WorkerHelpers {
    /// Create a new instance of WorkerHelpers
    pub fn new(pg_pool: PgPool, escaped_schema: String) -> Self {
        Self {
            pg_pool,
            escaped_schema,
        }
    }

    /// Add a job to the queue
    /// The payload must be exactly the same type as the one defined in the task definition
    pub async fn add_job<T: TaskDefinition<WorkerContext>>(
        &self,
        payload: T::Payload,
        spec: Option<JobSpec>,
    ) -> Result<(), ArchimedesError> {
        let identifier = T::identifier();
        let payload = serde_json::to_value(payload)?;
        add_job(
            &self.pg_pool,
            &self.escaped_schema,
            identifier,
            payload,
            spec.unwrap_or_default(),
        )
        .await
    }

    /// Add a job to the queue
    /// Contrary to add_job, this method does not require the task definition to be known
    pub async fn add_raw_job<P>(
        &self,
        identifier: &str,
        payload: P,
        spec: Option<JobSpec>,
    ) -> Result<(), ArchimedesError>
    where
        P: Serialize,
    {
        let payload = serde_json::to_value(payload)?;
        add_job(
            &self.pg_pool,
            &self.escaped_schema,
            identifier,
            payload,
            spec.unwrap_or_default(),
        )
        .await
    }
}
