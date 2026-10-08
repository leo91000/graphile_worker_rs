use graphile_worker::{worker_utils::types::CleanupTask, JobSpec};
use graphile_worker_task_details::SharedTaskDetails;
use serde_json::json;

use crate::helpers::{sql::safe_query, with_test_db};

#[tokio::test]
async fn cleanup_uses_distinct_task_and_queue_ids_and_preserves_all_references() {
    with_test_db(|test_db| async move {
        let details = SharedTaskDetails::default();
        let utils = test_db.worker_utils().with_task_details(details.clone());
        utils.migrate().await.unwrap();
        safe_query(
            "INSERT INTO graphile_worker._private_tasks (id, identifier) OVERRIDING SYSTEM VALUE VALUES
             (101, 'queued'), (102, 'unqueued'), (103, 'failed'), (104, 'locked'),
             (201, 'unused'), (202, 'registered');",
        )
        .execute(&test_db.test_pool)
        .await
        .unwrap();
        safe_query(
            "INSERT INTO graphile_worker._private_job_queues (id, queue_name, locked_at, locked_by)
             OVERRIDING SYSTEM VALUE VALUES (201, 'live', NULL, NULL), (101, 'unused', NULL, NULL),
             (301, 'locked_unused', now(), 'worker'), (401, 'empty', NULL, NULL);",
        )
        .execute(&test_db.test_pool)
        .await
        .unwrap();
        details.insert(202, "registered".into()).await;

        for identifier in ["queued", "queued", "unqueued", "failed", "locked"] {
            utils
                .add_raw_job(
                    identifier,
                    json!({}),
                    JobSpec {
                        queue_name: (identifier != "unqueued").then(|| "live".into()),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
        }
        safe_query(
            "UPDATE graphile_worker._private_jobs SET attempts = max_attempts WHERE task_id = 103",
        )
        .execute(&test_db.test_pool)
        .await
        .unwrap();
        safe_query(
            "UPDATE graphile_worker._private_jobs SET locked_at = now(), locked_by = 'worker'
             WHERE task_id = 104",
        )
        .execute(&test_db.test_pool)
        .await
        .unwrap();

        // Queue 201 is live but task 201 is unused; the reverse is true for ID
        // 101. Correlating on the wrong column must fail these assertions.
        utils.cleanup(&[CleanupTask::GcJobQueues]).await.unwrap();
        let queues: Vec<i32> = sqlx::query_scalar(
            "SELECT id FROM graphile_worker._private_job_queues ORDER BY id",
        )
        .fetch_all(&test_db.test_pool)
        .await
        .unwrap();
        assert_eq!(queues, [201, 301]);

        utils.cleanup(&[CleanupTask::GcTaskIdentifiers]).await.unwrap();
        let tasks: Vec<i32> =
            sqlx::query_scalar("SELECT id FROM graphile_worker._private_tasks ORDER BY id")
                .fetch_all(&test_db.test_pool)
                .await
                .unwrap();
        assert_eq!(tasks, [101, 102, 103, 104, 202]);
        assert_eq!(test_db.get_jobs().await.len(), 5);
        let dangling: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM graphile_worker._private_jobs jobs
             LEFT JOIN graphile_worker._private_tasks tasks ON tasks.id = jobs.task_id
             LEFT JOIN graphile_worker._private_job_queues queues ON queues.id = jobs.job_queue_id
             WHERE tasks.id IS NULL OR (jobs.job_queue_id IS NOT NULL AND queues.id IS NULL)",
        )
        .fetch_one(&test_db.test_pool)
        .await
        .unwrap();
        assert_eq!(dangling, 0, "cleanup must preserve every existing job reference");
    })
    .await;
}

#[tokio::test]
async fn cleanup_of_empty_jobs_preserves_registered_tasks_and_locked_queues() {
    with_test_db(|test_db| async move {
        let details = SharedTaskDetails::default();
        let utils = test_db.worker_utils().with_task_details(details.clone());
        utils.migrate().await.unwrap();
        safe_query(
            "INSERT INTO graphile_worker._private_tasks (id, identifier)
             OVERRIDING SYSTEM VALUE VALUES (51, 'registered'), (52, 'unused')",
        )
        .execute(&test_db.test_pool)
        .await
        .unwrap();
        safe_query(
            "INSERT INTO graphile_worker._private_job_queues (queue_name, locked_at, locked_by)
             VALUES ('unused', NULL, NULL), ('locked', now(), 'worker')",
        )
        .execute(&test_db.test_pool)
        .await
        .unwrap();
        details.insert(51, "registered".into()).await;

        utils
            .cleanup(&[CleanupTask::GcTaskIdentifiers, CleanupTask::GcJobQueues])
            .await
            .unwrap();
        assert_eq!(test_db.get_tasks().await, ["registered"]);
        let queues = test_db.get_job_queues().await;
        assert_eq!(queues.len(), 1);
        assert_eq!(queues[0].queue_name, "locked");
        assert_eq!(queues[0].locked_by.as_deref(), Some("worker"));
        assert!(test_db.get_jobs().await.is_empty());
    })
    .await;
}
