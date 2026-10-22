#[path = "helpers/mod.rs"]
mod helpers;

use graphile_worker::{sql::get_queue_names::get_queue_names, JobSpec};

/// Names retain input order and duplicate/missing IDs in a quoted custom schema.
#[tokio::test]
async fn resolves_queue_names_without_stale_results() {
    helpers::with_test_db(|db| async move {
        let worker = db
            .create_worker_options()
            .schema("queue \"lookup")
            .init()
            .await
            .unwrap();
        let utils = worker.create_utils();
        let first = utils
            .add_raw_job(
                "names",
                serde_json::json!({}),
                JobSpec {
                    queue_name: Some("first".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let second = utils
            .add_raw_job(
                "names",
                serde_json::json!({}),
                JobSpec {
                    queue_name: Some("second".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let a = first.job_queue_id().unwrap();
        let b = second.job_queue_id().unwrap();
        assert_eq!(
            get_queue_names(&db.database, worker.schema(), &[b, -1, a, b])
                .await
                .unwrap(),
            [
                Some("second".into()),
                None,
                Some("first".into()),
                Some("second".into())
            ]
        );
        assert!(get_queue_names(&db.database, "missing_schema", &[a])
            .await
            .is_err());
        assert!(get_queue_names(&db.database, "missing_schema", &[])
            .await
            .unwrap()
            .is_empty());
        let transaction = db.database.begin().await.unwrap();
        assert_eq!(
            get_queue_names(&transaction, worker.schema(), &[a])
                .await
                .unwrap(),
            [Some("first".into())]
        );
        transaction.commit().await.unwrap();
        utils
            .complete_jobs(&[*first.id(), *second.id()])
            .await
            .unwrap();
        utils
            .cleanup(&[graphile_worker::worker_utils::types::CleanupTask::GcJobQueues])
            .await
            .unwrap();
        assert_eq!(
            get_queue_names(&db.database, worker.schema(), &[a, b])
                .await
                .unwrap(),
            [None, None]
        );
    })
    .await;
}
