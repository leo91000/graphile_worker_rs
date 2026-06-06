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

#[tokio::test]
async fn test_after_job_run_hook() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = ExtendedHooksPlugin::new();
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
                    should_error: false,
                },
                JobSpec::default(),
            )
            .await
            .expect("Failed to add job");

        let c = counters.clone();
        wait_for_condition(
            || c.after_job_run.load(Ordering::SeqCst) >= 1,
            5,
            "after_job_run should have been called",
        )
        .await;

        assert_eq!(counters.before_job_run.load(Ordering::SeqCst), 1);
        assert_eq!(counters.after_job_run.load(Ordering::SeqCst), 1);

        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn test_after_job_run_terminal_results() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let skip_worker = test_db
            .create_worker_options()
            .define_job::<TestJob>()
            .add_plugin(AfterJobRunResultPlugin {
                result: HookResult::Skip,
            })
            .init()
            .await
            .expect("Failed to create skip worker");

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
            .expect("Failed to add skip job");

        skip_worker
            .run_once()
            .await
            .expect("Failed to run skip worker");

        assert!(test_db.get_jobs().await.is_empty());

        let fail_worker = test_db
            .create_worker_options()
            .define_job::<TestJob>()
            .add_plugin(AfterJobRunResultPlugin {
                result: HookResult::Fail("after hook failed".to_string()),
            })
            .init()
            .await
            .expect("Failed to create fail worker");

        utils
            .add_job(
                TestJob {
                    value: 2,
                    skip: false,
                    force_fail: false,
                    should_error: false,
                },
                JobSpec::builder().max_attempts(1).build(),
            )
            .await
            .expect("Failed to add fail job");

        fail_worker
            .run_once()
            .await
            .expect("Failed to run fail worker");

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].attempts, 1);
        assert!(jobs[0]
            .last_error
            .as_ref()
            .is_some_and(|error| error.contains("after hook failed")));
    })
    .await;
}

#[tokio::test]
async fn test_on_job_permanently_fail_hook() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = ExtendedHooksPlugin::new();
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
            .add_raw_job(
                "test_hooks_job",
                serde_json::json!({
                    "value": 1,
                    "should_error": true
                }),
                JobSpec::builder().max_attempts(1).build(),
            )
            .await
            .expect("Failed to add job");

        let c = counters.clone();
        wait_for_condition(
            || c.job_permanently_fail.load(Ordering::SeqCst) >= 1,
            5,
            "on_job_permanently_fail should have been called",
        )
        .await;

        assert_eq!(counters.job_permanently_fail.load(Ordering::SeqCst), 1);

        worker_fut.abort();
    })
    .await;
}
