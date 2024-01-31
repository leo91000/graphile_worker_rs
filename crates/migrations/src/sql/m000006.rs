use indoc::indoc;

use super::GraphileWorkerMigration;

pub const M000006_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000006",
    is_breaking: false,
    stmts: &[
        indoc! {r#"
            create index jobs_priority_run_at_id_locked_at_without_failures_idx
                on :GRAPHILE_WORKER_SCHEMA.jobs (priority, run_at, id, locked_at)
                where attempts < max_attempts;
        "#},
        indoc! {r#"
            drop index :GRAPHILE_WORKER_SCHEMA.jobs_priority_run_at_id_idx;
        "#},
    ],
};
