use super::*;

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
            &graphile_worker::Schema::default(),
            vec![
                "sqlx_batch_small".to_string(),
                "sqlx_batch_large".to_string(),
            ],
        )
        .await
        .expect("Failed to get task details");

        let empty = add_jobs(
            &test_db.test_pool,
            &graphile_worker::Schema::default(),
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
            &graphile_worker::Schema::default(),
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
            &graphile_worker::Schema::default(),
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
            &graphile_worker::Schema::default(),
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
