#![cfg(feature = "driver-sqlx")]

use chrono::Utc;
use graphile_worker::sql::add_job::{add_job, add_jobs, JobToAdd};
use graphile_worker::sql::batch_get_jobs::batch_get_jobs;
use graphile_worker::sql::fail_job::{fail_jobs, FailedJob};
use graphile_worker::sql::get_job::get_job;
use graphile_worker::sql::task_identifiers::get_tasks_details;
use graphile_worker::{JobKeyMode, JobSpec, JobSpecBuilder};
use serde_json::json;

use helpers::with_test_db;

mod helpers;

#[tokio::test]
async fn sqlx_pool_exercises_batch_add_jobs_fast_path() {
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
        .expect("SQLx add_jobs fast path should insert jobs");
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
        .expect("SQLx add_jobs fast path should insert large batches");
        assert_eq!(large_added.len(), 512);
    })
    .await;
}

#[tokio::test]
async fn sqlx_pool_exercises_get_and_fail_fast_paths() {
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
        .expect("SQLx get_job fast path should succeed")
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
        .expect("SQLx fail_jobs fast path should handle jobs without queues");

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
        .expect("SQLx batch_get_jobs fast path should succeed");
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
        .expect("SQLx fail_jobs fast path should handle queued jobs");
    })
    .await;
}
