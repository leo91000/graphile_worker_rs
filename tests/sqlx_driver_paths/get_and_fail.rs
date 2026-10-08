use super::*;

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
            &graphile_worker::Schema::default(),
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
            &graphile_worker::Schema::default(),
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
            &graphile_worker::Schema::default(),
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
            &graphile_worker::Schema::default(),
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
            &graphile_worker::Schema::default(),
            "sqlx-worker-one",
        )
        .await
        .expect("SQLx fail_jobs should handle jobs without queues");

        let batch = batch_get_jobs(
            &test_db.test_pool,
            &task_details,
            &graphile_worker::Schema::default(),
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
            &graphile_worker::Schema::default(),
            "sqlx-worker-two",
        )
        .await
        .expect("SQLx fail_jobs should handle queued jobs");
    })
    .await;
}
