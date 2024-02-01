use graphile_worker::JobSpec;
use serde_json::json;

mod common;

#[tokio::test]
async fn it_supports_the_flags_api() {
    common::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_raw_job("job3" as &str, |_, _: serde_json::Value| async move {
                Ok(()) as Result<(), ()>
            })
            .init()
            .await
            .expect("Failed to create worker");
        let utils = worker.create_utils();

        utils
            .add_raw_job(
                "job3",
                json!({ "a": 1 }),
                Some(JobSpec {
                    flags: Some(vec!["a".to_string(), "b".to_string()]),
                    ..Default::default()
                }),
            )
            .await
            .expect("Failed to add job");
        drop(utils);

        let jobs = test_db.get_jobs().await;

        assert_eq!(jobs.len(), 1);
        assert_eq!(
            jobs[0].flags,
            Some(json!({
                "a": true,
                "b": true,
            }))
        );

        worker.run_once().await.expect("Failed to run worker");
    })
    .await;
}
