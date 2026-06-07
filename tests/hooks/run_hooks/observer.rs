use super::*;

#[tokio::test]
async fn test_observer_hooks_are_called() {
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

        let c = counters.clone();
        wait_for_condition(
            || c.worker_start.load(Ordering::SeqCst) >= 1,
            5,
            "Worker should have started",
        )
        .await;
        assert_eq!(counters.worker_start.load(Ordering::SeqCst), 1);

        utils
            .add_job(
                TestJob {
                    value: 1,
                    skip: false,
                    force_fail: false,
                    should_error: false,
                },
                JobSpec::default(),
            )
            .await
            .expect("Failed to add job");

        let c = counters.clone();
        wait_for_condition(
            || c.job_complete.load(Ordering::SeqCst) >= 1,
            5,
            "Job should have completed",
        )
        .await;

        assert_eq!(counters.job_fetch.load(Ordering::SeqCst), 1);
        assert_eq!(counters.before_job_run.load(Ordering::SeqCst), 1);
        assert_eq!(counters.job_start.load(Ordering::SeqCst), 1);
        assert_eq!(counters.job_complete.load(Ordering::SeqCst), 1);
        assert_eq!(counters.job_fail.load(Ordering::SeqCst), 0);

        worker_fut.abort();
    })
    .await;
}
#[tokio::test]
async fn test_multiple_plugins() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin1 = TestHooksPlugin::new();
        let plugin2 = TestHooksPlugin::new();
        let counters1 = plugin1.counters();
        let counters2 = plugin2.counters();

        let worker_fut = spawn_local({
            let database = test_db.database.clone();
            async move {
                Worker::options()
                    .database(database)
                    .concurrency(2)
                    .define_job::<TestJob>()
                    .add_plugin(plugin1)
                    .add_plugin(plugin2)
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
                    should_error: false,
                },
                JobSpec::default(),
            )
            .await
            .expect("Failed to add job");

        let c1 = counters1.clone();
        let c2 = counters2.clone();
        wait_for_condition(
            || {
                c1.job_complete.load(Ordering::SeqCst) >= 1
                    && c2.job_complete.load(Ordering::SeqCst) >= 1
            },
            5,
            "Both plugins should have seen job complete",
        )
        .await;

        assert_eq!(counters1.job_start.load(Ordering::SeqCst), 1);
        assert_eq!(counters2.job_start.load(Ordering::SeqCst), 1);
        assert_eq!(counters1.job_complete.load(Ordering::SeqCst), 1);
        assert_eq!(counters2.job_complete.load(Ordering::SeqCst), 1);

        worker_fut.abort();
    })
    .await;
}
