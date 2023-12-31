pub const M000015_MIGRATION: &[&str] = &[
    // Drop existing functions
    r#"
        DROP FUNCTION :ARCHIMEDES_SCHEMA.jobs__increase_job_queue_count();
    "#,
    r#"
        DROP FUNCTION :ARCHIMEDES_SCHEMA.jobs__decrease_job_queue_count();
    "#,
    r#"
        DROP FUNCTION :ARCHIMEDES_SCHEMA.tg__update_timestamp();
    "#,

    // Create a new function to unlock jobs and job queues
    r#"
        CREATE FUNCTION :ARCHIMEDES_SCHEMA.force_unlock_workers(worker_ids text[]) RETURNS void AS $$
        UPDATE :ARCHIMEDES_SCHEMA.jobs
        SET locked_at = null, locked_by = null
        WHERE locked_by = ANY(worker_ids);
        UPDATE :ARCHIMEDES_SCHEMA.job_queues
        SET locked_at = null, locked_by = null
        WHERE locked_by = ANY(worker_ids);
        $$ LANGUAGE sql VOLATILE;
    "#,
];
