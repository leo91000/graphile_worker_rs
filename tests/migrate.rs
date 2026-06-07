#![allow(unused_imports)]
use graphile_worker::{DbExecutor, DbParams, DbValue};
use graphile_worker_migrations::{
    migrate,
    sql::{
        m000001::M000001_MIGRATION, m000002::M000002_MIGRATION, m000003::M000003_MIGRATION,
        m000004::M000004_MIGRATION, m000005::M000005_MIGRATION, m000006::M000006_MIGRATION,
        m000007::M000007_MIGRATION, m000008::M000008_MIGRATION, m000009::M000009_MIGRATION,
        m000010::M000010_MIGRATION, GraphileWorkerMigration, MigrationStatements,
    },
    MigrateError,
};
use helpers::with_test_db;
use serde_json::json;
use sqlx::query;

mod helpers;

#[path = "migrate/future_revision.rs"]
mod future_revision;
#[path = "migrate/install_schema.rs"]
mod install_schema;
#[path = "migrate/locked_jobs_migration_11.rs"]
mod locked_jobs_migration_11;
#[path = "migrate/sqlx_transaction.rs"]
mod sqlx_transaction;
#[path = "migrate/take_over_existing.rs"]
mod take_over_existing;
