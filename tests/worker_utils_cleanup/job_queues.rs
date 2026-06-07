use chrono::Utc;
use graphile_worker::{worker_utils::types::CleanupTask, JobSpec};
use indoc::formatdoc;

use crate::helpers::sql::safe_query;
use crate::helpers::with_test_db;

#[tokio::test]
async fn cleanup_with_gc_job_queues() {
    with_test_db(|test_db| async move {
        let worker_utils = test_db
            .create_worker_options()
            .init()
            .await
            .expect("Failed to create worker")
            .create_utils();

        let mut jobs = Vec::new();
        let specs = [
            ("worker1", "test", "test_job1"),
            ("worker2", "test2", "test_job2"),
            ("worker3", "test3", "test_job3"),
        ];

        let mut a = 0;
        let mut date = Utc::now();
        for (worker_id, queue_name, task_identifier) in specs.iter() {
            date -= chrono::Duration::minutes(1);
            a += 1;
            let job = worker_utils
                .add_raw_job(
                    task_identifier,
                    serde_json::json!({ "a": a }),
                    JobSpec {
                        queue_name: Some(queue_name.to_string()),
                        ..Default::default()
                    },
                )
                .await
                .expect("Failed to add job");
            jobs.push(job.clone());

            let sql = formatdoc!(
                r#"
                    WITH j AS (
                        UPDATE {schema}._private_jobs
                        SET locked_at = $1, locked_by = $2
                        WHERE id = $3
                        RETURNING *
                    ), q AS (
                        UPDATE {schema}._private_job_queues
                        SET locked_by = $2, locked_at = $1
                        FROM j
                        WHERE {schema}._private_job_queues.id = j.job_queue_id
                    )
                    SELECT * FROM j;
                "#,
                schema = "graphile_worker"
            );
            safe_query(sql)
                .bind(date)
                .bind(worker_id)
                .bind(job.id())
                .execute(&test_db.test_pool)
                .await
                .expect("Failed to lock job");
        }

        worker_utils
            .force_unlock_workers(&["worker3"])
            .await
            .expect("Failed to unlock worker3");

        let last_job = jobs.pop().expect("There should be at least one job");
        worker_utils
            .complete_jobs(&[*last_job.id()])
            .await
            .expect("Failed to complete the last job");

        worker_utils
            .cleanup(&[CleanupTask::GcJobQueues])
            .await
            .expect("Failed to cleanup job queues");

        let remaining_queues = test_db.get_job_queues().await;
        let remaining_queue_names: Vec<_> = remaining_queues
            .iter()
            .map(|q| q.queue_name.clone())
            .collect();

        let expected_queues = vec!["test".to_string(), "test2".to_string()];
        assert_eq!(
            remaining_queue_names, expected_queues,
            "Only non-empty job queues should remain after cleanup"
        );
    })
    .await;
}
