use graphile_worker::JobSpec;
use serde_json::json;

mod helpers;

#[tokio::test]
async fn it_supports_the_flags_api() {
    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_raw_job("job3" as &str, |_, _: serde_json::Value| async move {
                Ok(()) as Result<(), ()>
            })
            .init()
            .await
            .expect("Failed to create worker");

        {
            let utils = worker.create_utils();

            utils
                .add_raw_job(
                    "job3",
                    json!({ "a": 1 }),
                    JobSpec {
                        flags: Some(vec!["a".to_string(), "b".to_string()]),
                        ..Default::default()
                    },
                )
                .await
                .expect("Failed to add job");
        }

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

// TODO: parametrized tests
#[tokio::test]
async fn it_should_skip_jobs_with_forbidden_flags() {
    helpers::with_test_db(|test_db| async move {
        let bad_flag = "d";

        let worker = test_db
            .create_worker_options()
            .add_forbidden_flag(bad_flag)
            .define_raw_job("job3" as &str, |_, _: serde_json::Value| async move {
                Ok(()) as Result<(), ()>
            })
            .init()
            .await
            .expect("Failed to create worker");

        {
            let utils = worker.create_utils();

            utils
                .add_raw_job(
                    "job3",
                    json!({ "a": 1 }),
                    JobSpec {
                        flags: Some(vec!["a".to_string(), "b".to_string()]),
                        ..Default::default()
                    },
                )
                .await
                .expect("Failed to add job");

            utils
                .add_raw_job(
                    "job3",
                    json!({ "a": 1 }),
                    JobSpec {
                        flags: Some(vec!["c".to_string(), bad_flag.to_string()]),
                        ..Default::default()
                    },
                )
                .await
                .expect("Failed to add job");
        }

        worker.run_once().await.expect("Failed to run worker");

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(
            jobs[0].flags,
            Some(json!({
                "c": true,
                "d": true,
            }))
        );
    })
    .await;
}

// TODO: parametrized tests
#[tokio::test]
async fn it_should_run_all_jobs_if_no_forbidden_flags() {
    helpers::with_test_db(|test_db| async move {
        let bad_flag = "z";

        let worker = test_db
            .create_worker_options()
            .add_forbidden_flag(bad_flag)
            .define_raw_job("job3" as &str, |_, _: serde_json::Value| async move {
                Ok(()) as Result<(), ()>
            })
            .init()
            .await
            .expect("Failed to create worker");

        {
            let utils = worker.create_utils();

            utils
                .add_raw_job(
                    "job3",
                    json!({ "a": 1 }),
                    JobSpec {
                        flags: Some(vec!["a".to_string(), "b".to_string()]),
                        ..Default::default()
                    },
                )
                .await
                .expect("Failed to add job");

            utils
                .add_raw_job(
                    "job3",
                    json!({ "a": 1 }),
                    JobSpec {
                        flags: Some(vec!["c".to_string(), "d".to_string()]),
                        ..Default::default()
                    },
                )
                .await
                .expect("Failed to add job");
        }

        worker.run_once().await.expect("Failed to run worker");

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 0);
    })
    .await;
}
