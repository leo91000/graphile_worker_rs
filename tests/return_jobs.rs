use graphile_worker::{sql::return_jobs::batch::return_jobs, JobSpec, JobSpecBuilder};

mod helpers;

#[tokio::test]
async fn return_jobs_unlocks_mixed_queue_jobs() {
    helpers::with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let queued = utils
            .add_raw_job(
                "return_queued_job",
                serde_json::json!({}),
                JobSpecBuilder::new()
                    .queue_name("return_queue")
                    .build(),
            )
            .await
            .expect("Failed to add queued job");
        let regular = utils
            .add_raw_job(
                "return_regular_job",
                serde_json::json!({}),
                JobSpec::default(),
            )
            .await
            .expect("Failed to add regular job");

        let queue_id = queued
            .job_queue_id()
            .expect("Queued job should have queue id");
        let job_ids = vec![*queued.id(), *regular.id()];

        sqlx::query(
            "update graphile_worker._private_jobs set attempts = 1, locked_by = 'return-worker', locked_at = now() where id = any($1::bigint[])",
        )
        .bind(&job_ids)
        .execute(&test_db.test_pool)
        .await
        .expect("Failed to lock jobs");
        sqlx::query(
            "update graphile_worker._private_job_queues set locked_by = 'return-worker', locked_at = now() where id = $1",
        )
        .bind(queue_id)
        .execute(&test_db.test_pool)
        .await
        .expect("Failed to lock queue");

        return_jobs(
            &test_db.database,
            &[queued, regular],
            &graphile_worker::Schema::default(),
            "return-worker",
        )
        .await
        .expect("Failed to return jobs");

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 2);
        assert!(jobs.iter().all(|job| job.locked_by.is_none()));
        assert!(jobs.iter().all(|job| job.locked_at.is_none()));
        assert!(jobs.iter().all(|job| job.attempts == 0));

        let queues = test_db.get_job_queues().await;
        let queue = queues
            .iter()
            .find(|queue| queue.queue_name == "return_queue")
            .expect("Queue should exist");
        assert!(queue.locked_by.is_none());
        assert!(queue.locked_at.is_none());
    })
    .await;
}
