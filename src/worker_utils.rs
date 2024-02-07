use graphile_worker_task_handler::TaskDefinition;
use indoc::formatdoc;
use serde::Serialize;
use sqlx::{PgExecutor, PgPool};

use crate::{
    errors::GraphileWorkerError, sql::add_job::add_job, DbJob, Job, JobSpec, WorkerContext,
};

pub enum CleanupTask {
    GcTaskIdentifiers,
    GcJobQueues,
    DeletePermenantlyFailedJobs,
}

impl CleanupTask {
    pub async fn execute<'e>(
        &self,
        executor: impl PgExecutor<'e>,
        escaped_schema: &str,
    ) -> Result<(), GraphileWorkerError> {
        match self {
            CleanupTask::DeletePermenantlyFailedJobs => {
                let sql = formatdoc!(
                    r#"
                        delete from {escaped_schema}._private_jobs jobs
                            where attempts = max_attempts
                            and locked_at is null;
                    "#,
                    escaped_schema = escaped_schema
                );

                sqlx::query(&sql).execute(executor).await?;
            }
            CleanupTask::GcTaskIdentifiers => {
                let sql = formatdoc!(
                    r#"
                        delete from {escaped_schema}._private_tasks tasks
                        where tasks.id not in (
                            select jobs.task_id from {escaped_schema}._private_jobs jobs
                        );
                    "#,
                    escaped_schema = escaped_schema
                );

                sqlx::query(&sql).execute(executor).await?;
            }
            CleanupTask::GcJobQueues => {
                let sql = formatdoc!(
                    r#"
                        delete from {escaped_schema}._private_job_queues job_queues
                        where job_queues.id not in (
                            select jobs.job_queue_id from {escaped_schema}._private_jobs jobs
                        );
                    "#,
                    escaped_schema = escaped_schema
                );

                sqlx::query(&sql)
                    .execute(executor)
                    .await
                    .expect("Failed to run gc_job_queues");
            }
        }

        Ok(())
    }
}

#[derive(Default, Debug)]
pub struct RescheduleJobOptions {
    pub run_at: Option<chrono::DateTime<chrono::Utc>>,
    pub priority: Option<i16>,
    pub attempts: Option<i16>,
    pub max_attempts: Option<i16>,
}

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
    ) -> Result<Job, GraphileWorkerError> {
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
    ) -> Result<Job, GraphileWorkerError>
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
        let sql = formatdoc!(
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

    pub async fn complete_jobs(&self, ids: &[i64]) -> Result<Vec<DbJob>, GraphileWorkerError> {
        let sql = formatdoc!(
            r#"
                select * from {escaped_schema}.complete_jobs($1::bigint[]);
            "#,
            escaped_schema = self.escaped_schema
        );

        let jobs = sqlx::query_as(&sql)
            .bind(ids)
            .fetch_all(&self.pg_pool)
            .await?;

        Ok(jobs)
    }

    pub async fn permanently_fail_jobs(
        &self,
        ids: &[i64],
        reason: &str,
    ) -> Result<Vec<DbJob>, GraphileWorkerError> {
        let sql = formatdoc!(
            r#"
                select * from {escaped_schema}.permanently_fail_jobs($1::bigint[], $2::text);
            "#,
            escaped_schema = self.escaped_schema
        );

        let jobs = sqlx::query_as(&sql)
            .bind(ids)
            .bind(reason)
            .fetch_all(&self.pg_pool)
            .await?;

        Ok(jobs)
    }

    pub async fn reschedule_jobs(
        &self,
        ids: &[i64],
        options: RescheduleJobOptions,
    ) -> Result<Vec<DbJob>, GraphileWorkerError> {
        let sql = formatdoc!(
            r#"
                select * from {escaped_schema}.reschedule_jobs(
                    $1::bigint[],
                    run_at := $2::timestamptz,
                    priority := $3::int,
                    attempts := $4::int,
                    max_attempts := $5::int
                );
            "#,
            escaped_schema = self.escaped_schema
        );

        let jobs = sqlx::query_as(&sql)
            .bind(ids)
            .bind(options.run_at)
            .bind(options.priority)
            .bind(options.attempts)
            .bind(options.max_attempts)
            .fetch_all(&self.pg_pool)
            .await?;

        Ok(jobs)
    }

    pub async fn force_unlock_workers(
        &self,
        worker_ids: &[&str],
    ) -> Result<(), GraphileWorkerError> {
        let sql = formatdoc!(
            r#"
                select * from {escaped_schema}.force_unlock_workers($1::text[]);
            "#,
            escaped_schema = self.escaped_schema
        );

        sqlx::query(&sql)
            .bind(worker_ids)
            .execute(&self.pg_pool)
            .await?;

        Ok(())
    }

    pub async fn cleanup(&self, tasks: &[CleanupTask]) -> Result<(), GraphileWorkerError> {
        for task in tasks {
            task.execute(&self.pg_pool, &self.escaped_schema).await?;
        }
        Ok(())
    }
}
