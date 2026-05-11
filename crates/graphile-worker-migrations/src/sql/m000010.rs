use super::GraphileWorkerMigration;

pub const M000010_MIGRATION: GraphileWorkerMigration = GraphileWorkerMigration {
    name: "m000010",
    is_breaking: false,
    stmts: &["alter table :GRAPHILE_WORKER_SCHEMA.jobs alter column queue_name drop default;"],
};
