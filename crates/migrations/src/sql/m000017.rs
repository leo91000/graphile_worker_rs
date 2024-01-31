use indoc::indoc;

use super::GraphileWorkerMigration;

pub const M000017_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000017",
    is_breaking: false,
    stmts: &[
        // Create a new view 'jobs'
        indoc! {r#"
            CREATE VIEW :GRAPHILE_WORKER_SCHEMA.jobs AS (
                SELECT
                    jobs.id,
                    job_queues.queue_name,
                    tasks.identifier AS task_identifier,
                    jobs.priority,
                    jobs.run_at,
                    jobs.attempts,
                    jobs.max_attempts,
                    jobs.last_error,
                    jobs.created_at,
                    jobs.updated_at,
                    jobs.key,
                    jobs.locked_at,
                    jobs.locked_by,
                    jobs.revision,
                    jobs.flags
                FROM :GRAPHILE_WORKER_SCHEMA._private_jobs AS jobs
                INNER JOIN :GRAPHILE_WORKER_SCHEMA._private_tasks AS tasks
                ON tasks.id = jobs.task_id
                LEFT JOIN :GRAPHILE_WORKER_SCHEMA._private_job_queues AS job_queues
                ON job_queues.id = jobs.job_queue_id
            );
        "#},
    ],
};
