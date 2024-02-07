use chrono::Utc;
use graphile_worker::JobSpec;
use indoc::formatdoc;

use crate::helpers::with_test_db;

mod helpers;

#[tokio::test]
async fn unlocks_jobs_for_given_workers_leaves_others_unaffected() {
    with_test_db(|test_db| async move {
        let worker_utils = test_db
            .create_worker_options()
            .init()
            .await
            .expect("Failed to create worker")
            .create_utils();

        let specs = vec![
            (Some("worker1"), None),
            (Some("worker1"), Some("test")),
            (Some("worker2"), None),
            (Some("worker2"), Some("test2")),
            (Some("worker2"), Some("test3")),
            (Some("worker3"), None),
            (None, None),
            (None, Some("test")),
            (None, Some("test2")),
            (None, Some("test3")),
        ];

        let mut jobs = Vec::new();
        let mut date = Utc::now();

        for (worker_id, queue_name) in specs.iter() {
            date -= chrono::Duration::minutes(1);
            let job = worker_utils
                .add_raw_job(
                    "job3",
                    serde_json::json!({ "a": jobs.len() + 1 }),
                    Some(JobSpec {
                        queue_name: queue_name.map(|qn| qn.to_string()),
                        ..Default::default()
                    }),
                )
                .await
                .expect("Failed to add job");

            // Lock job
            let lock_job_sql = format!(
                "UPDATE {schema}._private_jobs SET locked_at = $1, locked_by = $2 WHERE id = $3",
                schema = "graphile_worker"
            );
            sqlx::query(&lock_job_sql)
                .bind(worker_id.map(|_| date))
                .bind(worker_id)
                .bind(job.id())
                .execute(&test_db.test_pool)
                .await
                .expect("Failed to lock job");

            jobs.push(job);
        }

        // Update job queues based on the locked jobs
        let update_queues_sql = formatdoc!(
            r#"
                UPDATE {schema}._private_job_queues AS job_queues
                    SET locked_at = jobs.locked_at, locked_by = jobs.locked_by
                    FROM {schema}._private_jobs AS jobs
                    WHERE jobs.job_queue_id = job_queues.id;"#,
            schema = "graphile_worker"
        );
        sqlx::query(&update_queues_sql)
            .execute(&test_db.test_pool)
            .await
            .expect("Failed to update job queues based on locked jobs");

        // Unlock jobs for worker2 and worker3
        worker_utils
            .force_unlock_workers(&["worker2", "worker3"])
            .await
            .expect("Failed to unlock workers");

        // Fetch remaining jobs and verify their locked states
        let remaining_jobs = test_db.get_jobs().await;

        for (i, job) in remaining_jobs.iter().enumerate() {
            let (expected_worker_id, _) = specs[i];
            if expected_worker_id == Some("worker2") || expected_worker_id == Some("worker3") {
                assert!(job.locked_by.is_none(), "Job should be unlocked");
                assert!(job.locked_at.is_none(), "Job should have no locked_at");
            } else if expected_worker_id.is_some() {
                assert_eq!(
                    job.locked_by,
                    expected_worker_id.map(String::from),
                    "Job should remain locked by the original worker"
                );
                assert!(
                    job.locked_at.is_some(),
                    "Job should have a locked_at timestamp"
                );
            } else {
                assert!(job.locked_by.is_none(), "Job should not be locked");
                assert!(job.locked_at.is_none(), "Job should have no locked_at");
            }
        }

        // Verify locked queues
        let locked_queues = test_db
            .get_job_queues()
            .await
            .into_iter()
            .filter(|q| q.locked_at.is_some())
            .collect::<Vec<_>>();
        assert_eq!(
            locked_queues.len(),
            1,
            "Only one job queue should remain locked"
        );
        assert_eq!(
            locked_queues[0].queue_name, "test",
            "The locked queue should be 'test'"
        );
        assert_eq!(
            locked_queues[0].locked_by,
            Some(String::from("worker1")),
            "The locked queue should be locked by 'worker1'"
        );
    })
    .await;
}
