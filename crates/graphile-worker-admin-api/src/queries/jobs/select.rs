use graphile_worker_database::Schema;
use indoc::formatdoc;

use crate::sql::AdminTable;

pub(super) fn jobs_select_sql(schema: &Schema) -> String {
    let jobs = AdminTable::Jobs.qualified(schema);
    let tasks = AdminTable::Tasks.qualified(schema);
    let job_queues = AdminTable::JobQueues.qualified(schema);

    formatdoc!(
        r#"
            select
                jobs.id,
                tasks.identifier as task_identifier,
                job_queues.queue_name,
                jobs.payload,
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
                jobs.flags,
                jobs.is_available
            from {jobs} as jobs
            inner join {tasks} as tasks on tasks.id = jobs.task_id
            left join {job_queues} as job_queues on job_queues.id = jobs.job_queue_id
        "#
    )
}
