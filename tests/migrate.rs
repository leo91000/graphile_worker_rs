#![allow(unused_imports)]
use graphile_worker::{DbExecutor, DbParams, DbValue};
use graphile_worker_migrations::{
    migrate,
    sql::{GraphileWorkerMigration, MigrationExecuteExt, GRAPHILE_WORKER_MIGRATIONS},
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
