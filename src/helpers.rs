use graphile_worker_task_handler::TaskDefinition;
use serde::Serialize;
use sqlx::PgPool;

use crate::{errors::GraphileWorkerError, sql::add_job::add_job, JobSpec, WorkerContext};

/// The WorkerHelpers struct provides a set of methods to add jobs to the queue
pub struct WorkerUtils {
    pg_pool: PgPool,
    escaped_schema: String,
}

impl WorkerUtils {
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
    ) -> Result<(), GraphileWorkerError> {
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
    ) -> Result<(), GraphileWorkerError>
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

    pub async fn remove_job(&self, job_key: &str) -> Result<(), GraphileWorkerError> {
        let sql = format!(
            r#"
            select * from {escaped_schema}.remove_job($1::text);
        "#,
            escaped_schema = self.escaped_schema
        );

        sqlx::query(&sql)
            .bind(job_key)
            .execute(&self.pg_pool)
            .await?;

        Ok(())
    }
}
