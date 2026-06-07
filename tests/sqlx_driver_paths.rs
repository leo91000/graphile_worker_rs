#![cfg(feature = "driver-sqlx")]

use chrono::Utc;
use graphile_worker::sql::add_job::batch::add_jobs;
use graphile_worker::sql::add_job::single::add_job;
use graphile_worker::sql::add_job::types::JobToAdd;
use graphile_worker::sql::batch_get_jobs::batch_get_jobs;
use graphile_worker::sql::fail_job::batch::{fail_jobs, FailedJob};
use graphile_worker::sql::get_job::get_job;
use graphile_worker::sql::task_identifiers::get_tasks_details;
use graphile_worker::{DbExecutorArg, DbParams, DbValue, JobKeyMode, JobSpec, JobSpecBuilder};
use serde_json::json;

use helpers::sql::safe_query_scalar;
use helpers::with_test_db;

mod helpers;

async fn count_jobs(test_db: &helpers::TestDatabase, identifier: &str) -> i64 {
    let query = indoc::formatdoc! {"
        SELECT count(*)
        FROM graphile_worker._private_jobs jobs
        JOIN graphile_worker._private_tasks tasks ON tasks.id = jobs.task_id
        WHERE tasks.identifier = $1
    "};
    safe_query_scalar(query)
        .bind(identifier)
        .fetch_one(&test_db.test_pool)
        .await
        .expect("Failed to count jobs")
}

#[path = "sqlx_driver_paths/batch_add.rs"]
mod batch_add;
#[path = "sqlx_driver_paths/get_and_fail.rs"]
mod get_and_fail;
#[path = "sqlx_driver_paths/native_args.rs"]
mod native_args;
#[path = "sqlx_driver_paths/transactions.rs"]
mod transactions;
