#![cfg(feature = "driver-sqlx")]

use chrono::Utc;
use graphile_worker::sql::add_job::{add_job, add_jobs, JobToAdd};
use graphile_worker::sql::batch_get_jobs::batch_get_jobs;
use graphile_worker::sql::fail_job::{fail_jobs, FailedJob};
use graphile_worker::sql::get_job::get_job;
use graphile_worker::sql::task_identifiers::get_tasks_details;
use graphile_worker::{DbExecutorArg, DbParams, DbValue, JobKeyMode, JobSpec, JobSpecBuilder};
use serde_json::json;

use helpers::with_test_db;

mod helpers;

async fn count_jobs(test_db: &helpers::TestDatabase, identifier: &str) -> i64 {
    let query = indoc::formatdoc! {"
        SELECT count(*)
        FROM graphile_worker._private_jobs jobs
        JOIN graphile_worker._private_tasks tasks ON tasks.id = jobs.task_id
        WHERE tasks.identifier = $1
    "};
    sqlx::query_scalar(sqlx::AssertSqlSafe(query.as_str()))
        .bind(identifier)
        .fetch_one(&test_db.test_pool)
        .await
        .expect("Failed to count jobs")
}

#[tokio::test]
async fn sqlx_pool_exercises_batch_add_jobs() {
    with_test_db(|test_db| async move {
        test_db
            .worker_utils()
            .migrate()
            .await
            .expect("Failed to migrate");

        let task_details = get_tasks_details(
            &test_db.test_pool,
            "graphile_worker",
            vec![
                "sqlx_batch_small".to_string(),
                "sqlx_batch_large".to_string(),
            ],
        )
        .await
        .expect("Failed to get task details");

        let empty = add_jobs(
            &test_db.test_pool,
            "graphile_worker",
            &[],
            &task_details,
            false,
            true,
        )
        .await
        .expect("Empty add_jobs should succeed");
        assert!(empty.is_empty());

        let unsafe_spec = JobSpecBuilder::new()
            .job_key("unsafe")
            .job_key_mode(JobKeyMode::UnsafeDedupe)
            .build();
        let unsafe_job = [JobToAdd {
            identifier: "sqlx_batch_small",
            payload: json!({ "unsafe": true }),
            spec: &unsafe_spec,
        }];
        assert!(add_jobs(
            &test_db.test_pool,
            "graphile_worker",
            &unsafe_job,
            &task_details,
            false,
            true,
        )
        .await
        .is_err());

        let small_spec = JobSpecBuilder::new()
            .queue_name("sqlx_batch_queue")
            .priority(3)
            .flags(vec!["sqlx".to_string()])
            .build();
        let small_jobs = vec![
            JobToAdd {
                identifier: "sqlx_batch_small",
                payload: json!({ "value": 1 }),
                spec: &small_spec,
            },
            JobToAdd {
                identifier: "sqlx_batch_small",
                payload: json!({ "value": 2 }),
                spec: &small_spec,
            },
        ];

        let added = add_jobs(
            &test_db.test_pool,
            "graphile_worker",
            &small_jobs,
            &task_details,
            false,
            true,
        )
        .await
        .expect("SQLx add_jobs should insert jobs");
        assert_eq!(added.len(), 2);

        let large_spec = JobSpec::default();
        let large_jobs: Vec<_> = (0..512)
            .map(|value| JobToAdd {
                identifier: "sqlx_batch_large",
                payload: json!({ "value": value }),
                spec: &large_spec,
            })
            .collect();

        let large_added = add_jobs(
            &test_db.test_pool,
            "graphile_worker",
            &large_jobs,
            &task_details,
            false,
            true,
        )
        .await
        .expect("SQLx add_jobs should insert large batches");
        assert_eq!(large_added.len(), 512);
    })
    .await;
}

#[tokio::test]
async fn sqlx_transaction_executor_participates_in_caller_transaction() {
    with_test_db(|test_db| async move {
        test_db
            .worker_utils()
            .migrate()
            .await
            .expect("Failed to migrate");

        let mut tx = test_db
            .test_pool
            .begin()
            .await
            .expect("Failed to begin transaction");

        add_job(
            &mut tx,
            "graphile_worker",
            "sqlx_transaction_job",
            json!({ "transactional": true }),
            JobSpec::default(),
            false,
        )
        .await
        .expect("Failed to add job in SQLx transaction");

        tx.rollback()
            .await
            .expect("Failed to roll back transaction");

        assert_eq!(count_jobs(&test_db, "sqlx_transaction_job").await, 0);
    })
    .await;
}

#[tokio::test]
async fn sqlx_transaction_executor_handles_batch_and_multistep_helpers() {
    with_test_db(|test_db| async move {
        test_db
            .worker_utils()
            .migrate()
            .await
            .expect("Failed to migrate");

        let mut tx = test_db
            .test_pool
            .begin()
            .await
            .expect("Failed to begin transaction");

        let task_details = get_tasks_details(
            &mut tx,
            "graphile_worker",
            vec!["sqlx_transaction_batch".to_string()],
        )
        .await
        .expect("Failed to get task details in transaction");

        let spec = JobSpec::default();
        let jobs = [
            JobToAdd {
                identifier: "sqlx_transaction_batch",
                payload: json!({ "value": 1 }),
                spec: &spec,
            },
            JobToAdd {
                identifier: "sqlx_transaction_batch",
                payload: json!({ "value": 2 }),
                spec: &spec,
            },
        ];

        let added = add_jobs(
            &mut tx,
            "graphile_worker",
            &jobs,
            &task_details,
            false,
            false,
        )
        .await
        .expect("Failed to add batch jobs in SQLx transaction");
        assert_eq!(added.len(), 2);

        tx.rollback()
            .await
            .expect("Failed to roll back transaction");

        assert_eq!(count_jobs(&test_db, "sqlx_transaction_batch").await, 0);
    })
    .await;
}

#[tokio::test]
async fn sqlx_connection_executor_adds_job() {
    with_test_db(|test_db| async move {
        test_db
            .worker_utils()
            .migrate()
            .await
            .expect("Failed to migrate");

        let mut connection = test_db
            .test_pool
            .acquire()
            .await
            .expect("Failed to acquire SQLx connection");

        add_job(
            &mut *connection,
            "graphile_worker",
            "sqlx_connection_job",
            json!({ "connection": true }),
            JobSpec::default(),
            false,
        )
        .await
        .expect("Failed to add job with SQLx connection");

        assert_eq!(count_jobs(&test_db, "sqlx_connection_job").await, 1);
    })
    .await;
}

#[tokio::test]
async fn sqlx_native_executor_args_support_direct_queries() {
    with_test_db(|test_db| async move {
        let mut tx = test_db
            .test_pool
            .begin()
            .await
            .expect("Failed to begin transaction");
        let mut tx_executor = &mut tx;

        DbExecutorArg::execute(
            &mut tx_executor,
            "SELECT $1::int",
            vec![DbValue::I32(1)].into(),
        )
        .await
        .expect("SQLx transaction execute should succeed");

        let tx_rows = DbExecutorArg::fetch_all(
            &mut tx_executor,
            "SELECT $1::int AS value",
            vec![DbValue::I32(2)].into(),
        )
        .await
        .expect("SQLx transaction fetch_all should succeed");
        assert_eq!(tx_rows[0].try_get::<i32>("value").unwrap(), 2);

        let empty_error = DbExecutorArg::fetch_one(
            &mut tx_executor,
            "SELECT $1::int AS value WHERE false",
            vec![DbValue::I32(3)].into(),
        )
        .await
        .expect_err("SQLx transaction fetch_one should reject empty results");
        assert!(empty_error.to_string().contains("query returned no rows"));

        tx.rollback()
            .await
            .expect("Failed to roll back transaction");

        let mut connection = test_db
            .test_pool
            .acquire()
            .await
            .expect("Failed to acquire SQLx connection");
        let mut connection_executor = &mut *connection;

        DbExecutorArg::execute(
            &mut connection_executor,
            "SELECT $1::int",
            vec![DbValue::I32(4)].into(),
        )
        .await
        .expect("SQLx connection execute should succeed");

        let connection_rows = DbExecutorArg::fetch_all(
            &mut connection_executor,
            "SELECT $1::int AS value",
            vec![DbValue::I32(5)].into(),
        )
        .await
        .expect("SQLx connection fetch_all should succeed");
        assert_eq!(connection_rows[0].try_get::<i32>("value").unwrap(), 5);

        let mut pool_executor = &test_db.test_pool;
        let pool_rows = DbExecutorArg::fetch_all(
            &mut pool_executor,
            "SELECT $1::int AS value",
            vec![DbValue::I32(6)].into(),
        )
        .await
        .expect("SQLx pool DbExecutorArg fetch_all should succeed");
        assert_eq!(pool_rows[0].try_get::<i32>("value").unwrap(), 6);

        let params = DbParams::new();
        DbExecutorArg::execute(&mut pool_executor, "SELECT 1", params)
            .await
            .expect("SQLx pool DbExecutorArg execute should succeed");
    })
    .await;
}

#[tokio::test]
async fn sqlx_pool_exercises_get_and_fail_helpers() {
    with_test_db(|test_db| async move {
        test_db
            .worker_utils()
            .migrate()
            .await
            .expect("Failed to migrate");

        let task_details = get_tasks_details(
            &test_db.test_pool,
            "graphile_worker",
            vec![
                "sqlx_fetch_no_queue".to_string(),
                "sqlx_fetch_queue".to_string(),
            ],
        )
        .await
        .expect("Failed to get task details");

        let now = Utc::now();
        let no_queue_spec = JobSpecBuilder::new().priority(1).run_at(now).build();
        add_job(
            &test_db.test_pool,
            "graphile_worker",
            "sqlx_fetch_no_queue",
            json!({ "kind": "no_queue" }),
            no_queue_spec,
            false,
        )
        .await
        .expect("Failed to add no-queue job");

        let queue_spec = JobSpecBuilder::new()
            .priority(5)
            .run_at(now)
            .queue_name("sqlx_fetch_queue")
            .flags(vec!["allowed".to_string()])
            .build();
        add_job(
            &test_db.test_pool,
            "graphile_worker",
            "sqlx_fetch_queue",
            json!({ "kind": "queue" }),
            queue_spec,
            false,
        )
        .await
        .expect("Failed to add queued job");

        let skip_flags = vec!["blocked".to_string()];
        let first_job = get_job(
            &test_db.test_pool,
            &task_details,
            "graphile_worker",
            "sqlx-worker-one",
            &skip_flags,
            Some(now + chrono::Duration::seconds(1)),
        )
        .await
        .expect("SQLx get_job should succeed")
        .expect("A job should be fetched");
        assert_eq!(first_job.task_identifier(), "sqlx_fetch_no_queue");

        fail_jobs(
            &test_db.test_pool,
            &[FailedJob {
                job: &first_job,
                error: "retry no queue",
            }],
            "graphile_worker",
            "sqlx-worker-one",
        )
        .await
        .expect("SQLx fail_jobs should handle jobs without queues");

        let batch = batch_get_jobs(
            &test_db.test_pool,
            &task_details,
            "graphile_worker",
            "sqlx-worker-two",
            &skip_flags,
            10,
            Some(now + chrono::Duration::seconds(1)),
        )
        .await
        .expect("SQLx batch_get_jobs should succeed");
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].task_identifier(), "sqlx_fetch_queue");

        fail_jobs(
            &test_db.test_pool,
            &[FailedJob {
                job: &batch[0],
                error: "retry queue",
            }],
            "graphile_worker",
            "sqlx-worker-two",
        )
        .await
        .expect("SQLx fail_jobs should handle queued jobs");
    })
    .await;
}
