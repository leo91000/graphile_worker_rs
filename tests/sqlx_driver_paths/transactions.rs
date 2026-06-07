use super::*;

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
            &graphile_worker::Schema::default(),
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
            &graphile_worker::Schema::default(),
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
            &graphile_worker::Schema::default(),
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
