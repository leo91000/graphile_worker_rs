use archimedes_task_handler::TaskDefinition;
use sqlx::PgPool;

use crate::{errors::ArchimedesError, sql::add_job::add_job, JobSpec, WorkerContext};

pub struct WorkerHelpers {
    pg_pool: PgPool,
    escaped_schema: String,
}

impl WorkerHelpers {
    pub fn new(pg_pool: PgPool, escaped_schema: String) -> Self {
        Self {
            pg_pool,
            escaped_schema,
        }
    }

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
}
