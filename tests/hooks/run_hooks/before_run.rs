use super::*;

#[tokio::test]
async fn test_before_job_run_skip() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = TestHooksPlugin::new();
        let counters = plugin.counters();

        let worker_fut = spawn_local({
            let database = test_db.database.clone();
            async move {
                Worker::options()
                    .database(database)
                    .concurrency(2)
                    .define_job::<TestJob>()
                    .add_plugin(plugin)
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        utils
            .add_job(
                TestJob {
                    value: 1,
                    skip: true,
                    force_fail: false,
                    should_error: false,
                },
                JobSpec::default(),
            )
            .await
            .expect("Failed to add job");

        let c = counters.clone();
        wait_for_condition(
            || c.skipped.load(Ordering::SeqCst) >= 1,
            5,
            "Job should have been skipped",
        )
        .await;

        let c = counters.clone();
        wait_for_condition(
            || c.job_complete.load(Ordering::SeqCst) >= 1,
            5,
            "Skipped job should have completed",
        )
        .await;

        assert_eq!(counters.before_job_run.load(Ordering::SeqCst), 1);
        assert_eq!(counters.skipped.load(Ordering::SeqCst), 1);
        assert_eq!(counters.job_start.load(Ordering::SeqCst), 0);
        assert_eq!(counters.job_complete.load(Ordering::SeqCst), 1);

        worker_fut.abort();
    })
    .await;
}
#[tokio::test]
async fn test_before_job_run_fail() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = TestHooksPlugin::new();
        let counters = plugin.counters();

        let worker_fut = spawn_local({
            let database = test_db.database.clone();
            async move {
                Worker::options()
                    .database(database)
                    .concurrency(2)
                    .define_job::<TestJob>()
                    .add_plugin(plugin)
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        utils
            .add_job(
                TestJob {
                    value: 1,
                    skip: false,
                    force_fail: true,
                    should_error: false,
                },
                JobSpec::default(),
            )
            .await
            .expect("Failed to add job");

        let c = counters.clone();
        wait_for_condition(
            || c.failed_by_hook.load(Ordering::SeqCst) >= 1,
            5,
            "Job should have been failed by hook",
        )
        .await;

        let c = counters.clone();
        wait_for_condition(
            || c.job_fail.load(Ordering::SeqCst) >= 1,
            5,
            "Job fail hook should have been called",
        )
        .await;

        assert_eq!(counters.before_job_run.load(Ordering::SeqCst), 1);
        assert_eq!(counters.failed_by_hook.load(Ordering::SeqCst), 1);
        assert_eq!(counters.job_start.load(Ordering::SeqCst), 0);
        assert_eq!(counters.job_fail.load(Ordering::SeqCst), 1);

        worker_fut.abort();
    })
    .await;
}
#[tokio::test]
async fn test_job_fail_hook_on_task_error() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = TestHooksPlugin::new();
        let counters = plugin.counters();

        let worker_fut = spawn_local({
            let database = test_db.database.clone();
            async move {
                Worker::options()
                    .database(database)
                    .concurrency(2)
                    .define_job::<TestJob>()
                    .add_plugin(plugin)
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        utils
            .add_job(
                TestJob {
                    value: 1,
                    skip: false,
                    force_fail: false,
                    should_error: true,
                },
                JobSpec::default(),
            )
            .await
            .expect("Failed to add job");

        let c = counters.clone();
        wait_for_condition(
            || c.job_fail.load(Ordering::SeqCst) >= 1,
            5,
            "Job should have failed",
        )
        .await;

        assert_eq!(counters.before_job_run.load(Ordering::SeqCst), 1);
        assert_eq!(counters.job_start.load(Ordering::SeqCst), 1);
        assert_eq!(counters.job_complete.load(Ordering::SeqCst), 0);
        assert_eq!(counters.job_fail.load(Ordering::SeqCst), 1);

        worker_fut.abort();
    })
    .await;
}
