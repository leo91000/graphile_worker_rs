use indoc::indoc;

use super::ArchimedesMigration;

pub const M000015_MIGRATION: ArchimedesMigration = ArchimedesMigration {
    name: "m000015",
    is_breaking: false,
    stmts: &[
        // Drop existing functions
        indoc! {r#"
            DROP FUNCTION :ARCHIMEDES_SCHEMA.jobs__increase_job_queue_count();
        "#},
        indoc! {r#"
            DROP FUNCTION :ARCHIMEDES_SCHEMA.jobs__decrease_job_queue_count();
        "#},
        indoc! {r#"
            DROP FUNCTION :ARCHIMEDES_SCHEMA.tg__update_timestamp();
        "#},
        // Create a new function to unlock jobs and job queues
        indoc! {r#"
            CREATE FUNCTION :ARCHIMEDES_SCHEMA.force_unlock_workers(worker_ids text[]) RETURNS void AS $$
            UPDATE :ARCHIMEDES_SCHEMA.jobs
            SET locked_at = null, locked_by = null
            WHERE locked_by = ANY(worker_ids);
            UPDATE :ARCHIMEDES_SCHEMA.job_queues
            SET locked_at = null, locked_by = null
            WHERE locked_by = ANY(worker_ids);
            $$ LANGUAGE sql VOLATILE;
        "#},
    ],
};
