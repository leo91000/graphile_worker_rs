use graphile_worker::sql::add_job::single::add_job;
use graphile_worker::{Job, JobSpec, Schema, WorkerOptions, WorkerUtils};
use graphile_worker_crontab_runner::KnownCrontab;
use sqlx::Row;

use super::super::sql::known_crontab_from_sqlx_row;
use super::types::{JobQueue, JobWithQueueName, Migration, TestDatabase};

impl TestDatabase {
    pub fn create_worker_options(&self) -> WorkerOptions {
        WorkerOptions::default()
            .database(self.database.clone())
            .schema(Schema::default())
            .concurrency(4)
    }

    pub async fn get_jobs(&self) -> Vec<JobWithQueueName> {
        sqlx::query_as(
            r#"
                select jobs.*, identifier as task_identifier, job_queues.queue_name as queue_name
                    from graphile_worker._private_jobs as jobs
                    left join graphile_worker._private_tasks as tasks on jobs.task_id = tasks.id
                    left join graphile_worker._private_job_queues as job_queues on jobs.job_queue_id = job_queues.id
                    order by jobs.id asc
            "#,
        )
        .fetch_all(&self.test_pool)
        .await
        .expect("Failed to get jobs")
    }

    pub async fn get_job_queues(&self) -> Vec<JobQueue> {
        sqlx::query_as(
            r#"
                select job_queues.*, count(jobs.*)::int as job_count
                    from graphile_worker._private_job_queues as job_queues
                    left join graphile_worker._private_jobs as jobs on (
                        jobs.job_queue_id = job_queues.id
                    )
                    group by job_queues.id
                    order by job_queues.queue_name asc
            "#,
        )
        .fetch_all(&self.test_pool)
        .await
        .expect("Failed to get job queues")
    }

    pub async fn get_tasks(&self) -> Vec<String> {
        let rows = sqlx::query("select identifier from graphile_worker._private_tasks")
            .fetch_all(&self.test_pool)
            .await
            .expect("Failed to get tasks");

        rows.into_iter().map(|row| row.get("identifier")).collect()
    }

    pub async fn get_task_id(&self, identifier: &str) -> i32 {
        let row =
            sqlx::query("select id from graphile_worker._private_tasks where identifier = $1")
                .bind(identifier)
                .fetch_one(&self.test_pool)
                .await
                .expect("Failed to get task id");

        row.get("id")
    }

    pub async fn make_jobs_run_now(&self, task_identifier: &str) {
        sqlx::query(
            r#"
                update graphile_worker._private_jobs
                    set run_at = now()
                    where task_id = (
                        select id
                            from graphile_worker._private_tasks
                            where identifier = $1
                    )
            "#,
        )
        .bind(task_identifier)
        .execute(&self.test_pool)
        .await
        .expect("Failed to update jobs");
    }

    pub async fn get_migrations(&self) -> Vec<Migration> {
        sqlx::query_as("select * from graphile_worker.migrations")
            .fetch_all(&self.test_pool)
            .await
            .expect("Failed to get migrations")
    }

    pub async fn add_job(
        &self,
        identifier: &str,
        payload: impl serde::Serialize,
        spec: JobSpec,
    ) -> Job {
        add_job(
            &self.database,
            &Schema::default(),
            identifier,
            serde_json::to_value(payload).unwrap(),
            spec,
            true,
        )
        .await
        .expect("Failed to add job")
    }

    pub fn worker_utils(&self) -> WorkerUtils {
        WorkerUtils::new(self.database.clone(), Schema::default())
    }

    pub async fn get_known_crontabs(&self) -> Vec<KnownCrontab> {
        sqlx::query("select * from graphile_worker._private_known_crontabs")
            .fetch_all(&self.test_pool)
            .await
            .expect("Failed to get known crontabs")
            .into_iter()
            .map(known_crontab_from_sqlx_row)
            .collect()
    }
}
